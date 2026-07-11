use crate::context::ExecutorStatus;
use crate::execution::SimulationResult;
use crate::execution::engine::Engine;
use crate::execution::phase::MicroStepResult;
use crate::execution::runner::Runner;
use crate::execution::strategy::{AlwaysContinueStrategy, ContinueStrategy};
use crate::modeling::model::Model;
use crate::primitive::time::TickStatus;
use std::thread;
use std::time::{Duration as StdDuration, Instant};

/// イベントを発火した順番に直列で処理する標準的なRunner。
/// リアルタイムに状況をシミュレートするので、スキップは不可。
///
/// ※ [`Model`](Model)と[`Source`](crate::modeling::source::Source)と[`Hook`](crate::modeling::hook::Hook)が決定論的に動くとき決定論的に処理を行うことができる。
pub struct RealtimeRunner<CS> {
    continue_strategy: CS,
    /// 1 Tickの処理に必要な実時間（例: [std::time::Duration::from_millis(100)] で 1秒間に10Ticks）。
    tick_unit_duration: StdDuration,
}

impl<E, M: Model<E>, CS: ContinueStrategy<E, M, ()>> Runner<E, M, CS> for RealtimeRunner<CS> {
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

        // シミュレーション開始の実時間を記録（絶対時間同期のベースライン）
        let start_instant = Instant::now();

        // 最初に生成されるのは「待機状態」の executor
        let mut executor = engine.begin_simulation(&model);

        loop {
            let (executor_status, tick_status) = executor.peek_next_tick();
            if should_stop(&model, executor_status, tick_status) {
                break;
            }

            // --- [リアルタイム同期処理] ---
            // 次に処理すべきシミュレーション上のTick数を取得
            let next_tick_value = tick_status.current().as_time_tick();

            // シミュレーション開始時点から、本来「あるべき理想の実経過時間」を計算
            let target_elapsed = self.tick_unit_duration * next_tick_value as u32;

            // 現実の時間が理想の時間に追いつくまでスレッドをブロックして待機
            let now = Instant::now();
            let expected_instant = start_instant + target_elapsed;
            if now < expected_instant {
                thread::sleep(expected_instant - now);
            }
            // ------------------------------

            // 所有権を移行してTickを開始
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
                                active_executor = new_active_executor;
                                continue;
                            }
                            Err((new_active_executor, error)) => {
                                active_executor = new_active_executor;
                                runner_error = Some(error);
                                break;
                            }
                        }
                    }
                    MicroStepResult::Complete(new_active_executor, _) => {
                        active_executor = new_active_executor;
                        break;
                    }
                }
            }

            // 次のループの準備のため executor に戻す
            executor = active_executor.end_tick_with_increment_tick(&model);
        }

        if let Some(error) = runner_error.take() {
            executor.end_simulation_as_error(model, error)
        } else {
            executor.end_simulation_as_ok(model)
        }
    }
}

impl<E, M: Model<E>> RealtimeRunner<AlwaysContinueStrategy<E, M>> {
    /// 新しい `RealtimeRunner` を生成します。
    /// `tick_unit_duration` には 1 Tickの処理に必要な実時間を指定します（例: `Duration::from_millis(100)` で 1秒間に10Ticks）。
    pub fn new(tick_unit_duration: StdDuration) -> Self {
        RealtimeRunner {
            continue_strategy: AlwaysContinueStrategy::new(),
            tick_unit_duration,
        }
    }
}

impl<CS> RealtimeRunner<CS> {
    /// 新しい `RealtimeRunner` を生成します。
    /// `tick_unit_duration` には 1 Tickの処理に必要な実時間を指定します（例: `Duration::from_millis(100)` で 1秒間に10Ticks）。
    pub fn new_with_continue_strategy(
        tick_unit_duration: StdDuration,
        continue_strategy: CS,
    ) -> Self {
        RealtimeRunner {
            continue_strategy,
            tick_unit_duration,
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
    fn test_realtime_runner_new() {
        let runner_always = RealtimeRunner::<AlwaysContinueStrategy<TestEvent, TestModel>>::new(
            std::time::Duration::from_millis(100),
        );
        assert_eq!(
            runner_always.tick_unit_duration,
            std::time::Duration::from_millis(100)
        );

        let strategy = AlwaysContinueStrategy::<TestEvent, TestModel>::new();
        let runner_custom = RealtimeRunner::new_with_continue_strategy(
            std::time::Duration::from_millis(100),
            strategy,
        );
        assert_eq!(
            runner_custom.tick_unit_duration,
            std::time::Duration::from_millis(100)
        );
    }

    #[test]
    fn test_realtime_runner_run_success() {
        let model = TestModel { event_count: 0 };
        let mut engine = Engine::new();

        // 微妙なタイミングのTickで処理されるイベントを仕込む
        engine.schedule_event_at(
            SimTime::from_ticks(5),
            EventPriority::minimum(),
            TestEvent::A,
        );

        let mut runner = RealtimeRunner::new(std::time::Duration::from_millis(100));

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
    fn test_realtime_runner_run_with_strategy_error() {
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
        let mut runner = RealtimeRunner::new_with_continue_strategy(
            std::time::Duration::from_millis(100),
            strategy,
        );

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
    fn test_realtime_runner_run_without_strategy_error() {
        let model = TestModel { event_count: 0 };
        let mut engine = Engine::new();

        // エンジンで登録した時刻0のイベントはシミュレーション開始時に時刻0のイベントとして処理されるよう登録されるため、
        // マイクロステップ上限が0でも上限に引っかからない。
        engine.schedule_event_at(SimTime::zero(), EventPriority::minimum(), TestEvent::A);

        // マイクロステップ上限を「0」に、許容回数を「0」に設定した LimitAbortStrategy を投入
        // これにより、最初の Continue 判定で即座にエラーに落とす
        let strategy = LimitAbortStrategy::new(0, 0);
        let mut runner = RealtimeRunner::new_with_continue_strategy(
            std::time::Duration::from_millis(100),
            strategy,
        );

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
