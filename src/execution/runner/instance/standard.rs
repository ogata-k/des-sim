use crate::context::ExecutorStatus;
use crate::execution::SimulationResult;
use crate::execution::engine::Engine;
use crate::execution::phase::MicroStepResult;
use crate::execution::runner::Runner;
use crate::execution::strategy::{AlwaysContinueStrategy, ContinueStrategy};
use crate::modeling::model::Model;
use crate::primitive::time::TickStatus;

/// イベントを発火した順番に直列で処理する標準的なRunner。
/// skippableがtrueであれば、イベントがない時間をスキップすることができる。
///
/// ※ [`Model`](Model)と[`Source`](crate::modeling::source::Source)と[`Hook`](crate::modeling::hook::Hook)が決定論的に動くとき決定論的に処理を行うことができる。
pub struct StandardRunner<CS> {
    skippable: bool,
    continue_strategy: CS,
}

impl<E, M: Model<E>, CS: ContinueStrategy<E, M, ()>> Runner<E, M, CS> for StandardRunner<CS> {
    type Err = ();

    fn run<F>(
        &mut self,
        engine: Engine<E, M>,
        mut model: M,
        mut should_stop: F,
    ) -> SimulationResult<M, CS::Err>
    where
        F: FnMut(&M, ExecutorStatus, TickStatus) -> bool,
    {
        let mut runner_error: Option<CS::Err> = None;

        // 最初に生成されるのは「待機状態」の executor
        let mut executor = engine.begin_simulation(&model);

        loop {
            let (executor_status, tick_status) = executor.peek_next_tick();
            if should_stop(&model, executor_status, tick_status) {
                // ここで抜ける時は、手元に executor の所有権があるので
                // ループ外のend_simulation(model)に安全に渡せる
                break;
            }

            // これを呼ばないと、次の行の `active_executor` が作れないため、
            // 下のMicroStepループやフェーズ処理（シミュレータの本体）が1文字も書けない。
            let mut active_executor = executor.begin_tick(&model);

            loop {
                // 1. マイクロステップ開始
                let micro_step_handler = active_executor.begin_micro_step(&model);

                // 2. Sourceフェーズ
                let mut source_phase = micro_step_handler.start_source_phase(&model);
                while let Some(source_ready) = source_phase.take_one() {
                    source_phase.fire_and_schedule(&model, source_ready);
                }
                let micro_step_handler = source_phase.complete_source_phase(&model);

                // 3. Eventフェーズ
                let mut event_phase = micro_step_handler.to_event_phase(&model);
                while let Some(event_ready) = event_phase.take_one() {
                    event_phase.handle_event(&mut model, event_ready);
                }
                let micro_step_handler = event_phase.complete_event_phase(&model);

                // 4. マイクロステップ終了
                match micro_step_handler.end_micro_step(&model) {
                    MicroStepResult::Continue(unchecked) => {
                        match self
                            .continue_strategy
                            .handle_micro_step_continue(&model, unchecked)
                        {
                            Ok(new_active_executor) => {
                                // 次のループでactive_executorを呼ぶためにしっかり所有権を回収してからcontinueする。
                                active_executor = new_active_executor;
                                continue;
                            }
                            Err((new_active_executor, error)) => {
                                // エラー、つまりこのマイクロステップで終了とする場合は、所有権とエラーを回収してからbreakする。
                                active_executor = new_active_executor;
                                runner_error = Some(error);
                                break;
                            }
                        }
                    }
                    MicroStepResult::Complete(new_active_executor, _) => {
                        // 外側のループでend_tick()を呼ぶために、しっかり所有権を回収してからbreakする
                        active_executor = new_active_executor;
                        break;
                    }
                }
            }

            // active_executorをend_tick()して元の「executor」型に戻して再代入しないと、
            // ループの先頭に戻ったときに `executor.peek_next_tick()` が実行できず、コンパイルエラーになる。
            executor = if self.skippable {
                active_executor.end_tick_with_jump_to_next_tick(&model)
            } else {
                active_executor.end_tick_with_increment_tick(&model)
            };
        }

        if let Some(error) = runner_error.take() {
            executor.end_simulation_as_error(model, error)
        } else {
            // 綺麗にすべてのTickが閉じた executorで終了
            executor.end_simulation_as_ok(model)
        }
    }
}

impl<E, M: Model<E>> StandardRunner<AlwaysContinueStrategy<E, M>> {
    pub fn new(skippable: bool) -> Self {
        StandardRunner {
            skippable,
            continue_strategy: AlwaysContinueStrategy::new(),
        }
    }
}

impl<CS> StandardRunner<CS> {
    pub fn new_with_continue_strategy(skippable: bool, continue_strategy: CS) -> Self {
        StandardRunner {
            skippable,
            continue_strategy,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{EventContext, SourceContext, UserContext};
    use crate::execution::strategy::LimitAbortStrategy;
    use crate::modeling::event::{Event, EventPriority};
    use crate::modeling::source::Source;
    use crate::primitive::time::{Duration, SimTime, TickStatus};

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    enum TestEvent {
        A,
    }

    #[derive(Debug)]
    struct TestModel {
        event_count: usize,
    }

    impl Model<TestEvent> for TestModel {
        fn handle_event(
            &mut self,
            _context: &mut EventContext<TestEvent, Self>,
            _event: &Event<TestEvent>,
        ) {
            self.event_count += 1;
        }
    }

    #[test]
    fn test_standard_runner_new_and_skippable() {
        let runner_always =
            StandardRunner::<AlwaysContinueStrategy<TestEvent, TestModel>>::new(true);
        assert!(runner_always.skippable);

        let strategy = AlwaysContinueStrategy::<TestEvent, TestModel>::new();
        let runner_custom = StandardRunner::new_with_continue_strategy(false, strategy);
        assert!(!runner_custom.skippable);
    }

    #[test]
    fn test_standard_runner_run_success() {
        let model = TestModel { event_count: 0 };
        let mut engine = Engine::new();

        // 微妙なタイミングのTickで処理されるイベントを仕込む
        engine.schedule_event_at(
            SimTime::from_ticks(5),
            EventPriority::minimum(),
            TestEvent::A,
        );

        let mut runner = StandardRunner::new(true);

        // 10Tick分だけ回し終わったところで、シミュレーションを終了する停止条件
        let should_stop = |_m: &TestModel, _status: ExecutorStatus, tick: TickStatus| {
            tick.is_done_ticks(false, 10)
        };

        let result = runner.run(engine, model, should_stop);

        // シミュレーションが正常終了し、イベントが処理されたことを検証
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.model().event_count, 1);
    }

    #[test]
    fn test_standard_runner_run_with_strategy_error() {
        let model = TestModel { event_count: 0 };
        let mut engine = Engine::new();

        struct TestSource;

        impl Source<TestEvent, TestModel> for TestSource {
            fn on_registered(
                &mut self,
                _context: &mut dyn UserContext<TestEvent, TestModel>,
                _model: &TestModel,
            ) -> Option<Duration> {
                // 最初のマイクロステップ内で確実にループ/継続が発生するようイベントを登録
                Some(Duration::zero())
            }

            fn fire(
                &mut self,
                context: &mut SourceContext<TestEvent, TestModel>,
                _model: &TestModel,
            ) -> Option<Duration> {
                // 同一のマイクロステップ内で確実にループ/継続が発生するようイベントを登録
                context.schedule_event(Duration::zero(), EventPriority::minimum(), TestEvent::A);
                Some(Duration::one())
            }
        }
        engine.add_source("test source", TestSource);

        // マイクロステップ上限を「0」に、許容回数を「0」に設定した LimitAbortStrategy を投入
        // これにより、最初の Continue 判定で即座にエラーに落とす
        let strategy = LimitAbortStrategy::new(0, 0);
        let mut runner = StandardRunner::new_with_continue_strategy(true, strategy);

        // 無限ループを防ぐためのセーフティ付き停止条件（通常は戦略エラーで先に抜ける）
        let mut loop_count = 0;
        let should_stop = |_m: &TestModel, _status: ExecutorStatus, _tick: TickStatus| {
            loop_count += 1;
            loop_count > 10
        };

        let result = runner.run(engine, model, should_stop);

        // 戦略によってシミュレーションがエラー中断したことを検証
        assert!(result.is_err());
    }

    #[test]
    fn test_standard_runner_run_without_strategy_error() {
        let model = TestModel { event_count: 0 };
        let mut engine = Engine::new();

        // エンジンで登録した時刻0のイベントはシミュレーション開始時に時刻0のイベントとして処理されるよう登録されるため、
        // マイクロステップ上限が0でも上限に引っかからない。
        engine.schedule_event_at(SimTime::zero(), EventPriority::minimum(), TestEvent::A);

        // マイクロステップ上限を「0」に、許容回数を「0」に設定した LimitAbortStrategy を投入
        // これにより、最初の Continue 判定で即座にエラーに落とす
        let strategy = LimitAbortStrategy::new(0, 0);
        let mut runner = StandardRunner::new_with_continue_strategy(true, strategy);

        // 無限ループを防ぐためのセーフティ付き停止条件（通常は戦略エラーで先に抜ける）
        let mut loop_count = 0;
        let should_stop = |_m: &TestModel, _status: ExecutorStatus, _tick: TickStatus| {
            loop_count += 1;
            loop_count > 10
        };

        let result = runner.run(engine, model, should_stop);

        // 戦略によってシミュレーションがエラー中断しなかったことを検証
        assert!(result.is_ok());
    }
}
