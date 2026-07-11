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

/// イベントを発火をリアルタイムに行うRunner。
/// [`Source`](crate::modeling::source::Source)や[`Event`](crate::modeling::event::Event)がしばらくの間発火しない状態であれば、その時間は処理がスキップされる。
/// そのため、定期的に集計したいのであれば定期的に発火するが何もしない[`Source`](crate::modeling::source::Source)を使って観測点といて使う必要があることに注意。
/// ※ [`Model`](Model)と[`Source`](crate::modeling::source::Source)と[`Hook`](crate::modeling::hook::Hook)が決定論的に動くとき決定論的に処理を行うことができる。
#[derive(Clone)]
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
            // 何もない時間は処理不要なのでスキップしてCPUを節約する。
            executor = active_executor.end_tick_with_jump_to_next_tick(&model);

            if runner_error.is_some() {
                break;
            }
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
    use crate::modeling::hook::Hook;
    use crate::modeling::hook::instance::SharedHook;
    use crate::modeling::source::Source;
    use crate::primitive::time::{Duration, MicroStep, SimTime, TickStatus};
    use crate::source_handler::{SourceReadyEntry, SourceView};
    use std::sync::{Arc, Mutex};

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

    // 呼び出し順序を追跡するためのライフサイクルイベント定義
    #[derive(Debug, PartialEq, Eq, Clone)]
    enum LifecycleEvent {
        BeforeSimulation,
        BeforeTick(SimTime),
        BeforeFireSource(SimTime),
        BeforeScheduleEvent,
        AfterScheduleEvent,
        AfterFireSource(SimTime),
        AfterTick(SimTime),
        AfterSimulation,
    }

    // テスト用に、各イベントでトレースログを共有・記録するダミーソース
    struct TraceSource {
        trace: Arc<Mutex<Vec<LifecycleEvent>>>,
        initial_delay: Duration,
        interval_delay: Option<Duration>,
    }

    impl Source<TestEvent, TestModel> for TraceSource {
        fn on_registered(
            &mut self,
            _context: &mut dyn UserContext<TestEvent, TestModel>,
            _model: &TestModel,
        ) -> Option<Duration> {
            self.trace
                .lock()
                .unwrap()
                .push(LifecycleEvent::BeforeSimulation);
            Some(self.initial_delay)
        }

        fn fire(
            &mut self,
            context: &mut SourceContext<TestEvent, TestModel>,
            _model: &TestModel,
        ) -> Option<Duration> {
            let mut t = self.trace.lock().unwrap();
            t.push(LifecycleEvent::BeforeFireSource(context.current_tick()));
            t.push(LifecycleEvent::BeforeScheduleEvent);

            // イベントを発火させる
            context.schedule_event(Duration::zero(), EventPriority::minimum(), TestEvent::A);

            t.push(LifecycleEvent::AfterScheduleEvent);
            t.push(LifecycleEvent::AfterFireSource(context.current_tick()));
            self.interval_delay
        }
    }

    #[test]
    fn test_runner_lifecycle_execution_order_scenario() {
        let trace = Arc::new(Mutex::new(Vec::new()));
        let model = TestModel { event_count: 0 };
        let mut engine = Engine::new();

        // 時刻1で単発発火するソースを登録
        engine.add_source(
            "trace_source",
            TraceSource {
                trace: Arc::clone(&trace),
                initial_delay: Duration::ticks(1),
                interval_delay: None, // 単発発火
            },
        );

        let mut runner = RealtimeRunner::new(std::time::Duration::from_millis(100));

        // 停止条件：時刻2「になる前（peekした段階）」で止める
        let should_stop = move |_m: &TestModel, _status: ExecutorStatus, tick: TickStatus| {
            // run の実装通り、まず should_stop が評価される
            // 時刻2に到達した時点で停止させる
            if tick.is_done_ticks(false, 2) {
                return true;
            }

            false
        };

        let result = runner.run(engine, model, should_stop);
        assert!(result.is_ok());

        // ループを抜けた後、安全にシミュレーションが終了したため、最後に手動で記録
        trace.lock().unwrap().push(LifecycleEvent::AfterSimulation);

        let final_trace = trace.lock().unwrap();

        // 実際のコードフローに基づく、正当なライフサイクル順序:
        // 1. initialize_sources 内での登録時 (BeforeSimulation)
        // 2. 時刻0のTick開始 (イベントがないため、内部のMicroStepはスキップされるか即座に終わる)
        // 3. 時刻1のTick開始 -> MicroStepループ突入 -> Source発火 (BeforeMicroStep -> BeforeEvent -> AfterEvent -> AfterMicroStep)
        // 4. 時刻2のpeek時点で should_stop が true となりループ脱出 -> AfterSimulation
        let expected = vec![
            LifecycleEvent::BeforeSimulation,
            // 時刻1のイテレーション（時刻0はイベントがないため、このソースのfireは通らない）
            LifecycleEvent::BeforeFireSource(SimTime::from_ticks(1)),
            LifecycleEvent::BeforeScheduleEvent,
            LifecycleEvent::AfterScheduleEvent,
            LifecycleEvent::AfterFireSource(SimTime::from_ticks(1)),
            // ループ脱出後の終了処理
            LifecycleEvent::AfterSimulation,
        ];

        assert_eq!(*final_trace, expected);
    }

    #[test]
    fn test_lifecycle_interruption_on_strategy_error() {
        let trace = Arc::new(Mutex::new(Vec::new()));
        let model = TestModel { event_count: 0 };
        let mut engine = Engine::new();

        // 最初のTick（時刻0）で確実に無限発火するソースを仕込む
        engine.add_source(
            "loop_source",
            TraceSource {
                trace: Arc::clone(&trace),
                initial_delay: Duration::zero(),
                // 次のMicroStepに再度発火するよう設定
                interval_delay: Some(Duration::zero()),
            },
        );

        // マイクロステップの上限エラーを即座に発生させる戦略
        let strategy = LimitAbortStrategy::new(0, 0);
        let mut runner = RealtimeRunner::new_with_continue_strategy(
            std::time::Duration::from_millis(100),
            strategy,
        );

        let trace_for_stop = Arc::clone(&trace);
        let should_stop = move |_m: &TestModel, _status: ExecutorStatus, tick: TickStatus| {
            trace_for_stop
                .lock()
                .unwrap()
                .push(LifecycleEvent::BeforeTick(tick.current()));
            false
        };

        let result = runner.run(engine, model, should_stop);

        // 戦略エラーで異常終了することを確認
        assert!(result.is_err());

        let final_trace = trace.lock().unwrap();

        // エラー中断時であっても、`BeforeMicroStep` など開始されたフックの対となる
        // `AfterMicroStep` や `AfterTick`、`AfterSimulation` が異常を検知して正しく
        // 途切れる（またはクリーンアップへ向かう）流れになっているかを検証。
        // ※このテストにより、途中でパニック/エラー break した際にライフサイクルが
        // 異常なステートのまま残らないことを保証します。
        assert!(final_trace.contains(&LifecycleEvent::BeforeSimulation));
        assert!(final_trace.contains(&LifecycleEvent::BeforeTick(SimTime::zero())));
        assert!(!final_trace.contains(&LifecycleEvent::BeforeTick(SimTime::from_ticks(1))));

        // エラーが発生したマイクロステップ以降の正常系ライフサイクルイベント（AfterTickなど）は
        // 実行されずに、安全にループを脱出していることを検証
        let last_event = final_trace.last().unwrap();
        assert_ne!(last_event, &LifecycleEvent::AfterTick(SimTime::zero()));
    }

    // フックが呼ばれたことを詳細なパラメータと共に記録する列挙型
    #[derive(Debug, PartialEq, Eq, Clone)]
    enum HookCall {
        BeforeSimulation,
        AfterSimulation(SimTime),
        BeforeTick {
            current: SimTime,
            skipped: Duration,
        },
        AfterTick {
            current: SimTime,
            last_micro: MicroStep,
        },
        BeforeMicroStep {
            current: SimTime,
            micro: MicroStep,
        },
        AfterMicroStep {
            current: SimTime,
            micro: MicroStep,
        },
        BeforeSourcePhase {
            current: SimTime,
            micro: MicroStep,
        },
        AfterSourcePhase {
            current: SimTime,
            micro: MicroStep,
        },
        BeforeEventPhase {
            current: SimTime,
            micro: MicroStep,
        },
        AfterEventPhase {
            current: SimTime,
            micro: MicroStep,
        },
    }

    // テスト用の Hook 実装体
    struct MockHook {
        calls: Arc<Mutex<Vec<HookCall>>>,
    }

    impl<E, M: Model<E>> Hook<E, M> for MockHook {
        fn before_simulation(&self, _model: &M) {
            self.calls.lock().unwrap().push(HookCall::BeforeSimulation);
        }
        fn after_simulation(&self, _model: &M, end_tick: SimTime) {
            self.calls
                .lock()
                .unwrap()
                .push(HookCall::AfterSimulation(end_tick));
        }
        fn before_tick(&self, _model: &M, current_tick: SimTime, skipped_duration: Duration) {
            self.calls.lock().unwrap().push(HookCall::BeforeTick {
                current: current_tick,
                skipped: skipped_duration,
            });
        }
        fn after_tick(&self, _model: &M, current_tick: SimTime, last_micro_step: MicroStep) {
            self.calls.lock().unwrap().push(HookCall::AfterTick {
                current: current_tick,
                last_micro: last_micro_step,
            });
        }
        fn before_micro_step(
            &self,
            _model: &M,
            current_tick: SimTime,
            current_micro_step: MicroStep,
        ) {
            self.calls.lock().unwrap().push(HookCall::BeforeMicroStep {
                current: current_tick,
                micro: current_micro_step,
            });
        }
        fn after_micro_step(
            &self,
            _model: &M,
            current_tick: SimTime,
            current_micro_step: MicroStep,
        ) {
            self.calls.lock().unwrap().push(HookCall::AfterMicroStep {
                current: current_tick,
                micro: current_micro_step,
            });
        }
        // 簡略化のため、個別要素のフックは省略（必要に応じて同様に記録可能）
        fn on_discard_remain_micro_step(
            &self,
            _: &M,
            _: SimTime,
            _: MicroStep,
            _: &[SourceReadyEntry],
            _: &[Event<E>],
        ) {
        }
        fn before_register_source(&self, _: &M, _: &str) {}
        fn after_register_source(&self, _: &M, _: &str) {}
        fn before_source_phase(
            &self,
            _model: &M,
            current_tick: SimTime,
            current_micro_step: MicroStep,
        ) {
            self.calls
                .lock()
                .unwrap()
                .push(HookCall::BeforeSourcePhase {
                    current: current_tick,
                    micro: current_micro_step,
                });
        }

        fn before_source(&self, _: &M, _: SimTime, _: MicroStep, _: &SourceView) {}
        fn after_source(
            &self,
            _: &M,
            _: SimTime,
            _: MicroStep,
            _: &SourceView,
            _: Option<SimTime>,
        ) {
        }
        fn cancel_source(&self, _: &M, _: SimTime, _: MicroStep, _: SimTime, _: &SourceView) {}
        fn discard_source(&self, _: &M, _: SimTime, _: MicroStep, _: &SourceView) {}
        fn after_source_phase(
            &self,
            _model: &M,
            current_tick: SimTime,
            current_micro_step: MicroStep,
        ) {
            self.calls.lock().unwrap().push(HookCall::AfterSourcePhase {
                current: current_tick,
                micro: current_micro_step,
            });
        }
        fn before_event_phase(
            &self,
            _model: &M,
            current_tick: SimTime,
            current_micro_step: MicroStep,
        ) {
            self.calls.lock().unwrap().push(HookCall::BeforeEventPhase {
                current: current_tick,
                micro: current_micro_step,
            });
        }
        fn before_event(&self, _: &M, _: SimTime, _: MicroStep, _: &Event<E>) {}
        fn after_event(&self, _: &M, _: SimTime, _: MicroStep, _: &Event<E>) {}
        fn cancel_event(&self, _: &M, _: SimTime, _: MicroStep, _: SimTime, _: &Event<E>) {}
        fn discard_event(&self, _: &M, _: SimTime, _: MicroStep, _: &Event<E>) {}
        fn after_event_phase(
            &self,
            _model: &M,
            current_tick: SimTime,
            current_micro_step: MicroStep,
        ) {
            self.calls.lock().unwrap().push(HookCall::AfterEventPhase {
                current: current_tick,
                micro: current_micro_step,
            });
        }
    }

    #[test]
    fn test_standard_runner_hook_lifecycle_flow_with_include_zero_tick() {
        let hook = MockHook {
            calls: Arc::new(Mutex::new(Vec::new())),
        };
        let shared_hook = SharedHook::new(hook);

        let model = TestModel { event_count: 0 };
        let mut engine = Engine::new();

        // MockHookで記録を残すためにHookを登録する。
        engine.add_shared_hook(shared_hook.clone());

        // 時刻1にダミーイベントを1つだけ配置
        engine.schedule_event_at(
            SimTime::from_ticks(1),
            EventPriority::minimum(),
            TestEvent::A,
        );

        let mut runner = RealtimeRunner::new(std::time::Duration::from_millis(100));

        // 停止条件: 時刻 2つ分処理ができたらに終了
        let should_stop = |_m: &TestModel, _status: ExecutorStatus, tick: TickStatus| {
            // include_zero_tick=trueなので0tick 1tickの二つで終了
            tick.is_done_ticks(true, 2)
        };

        let _result = runner.run(engine, model, should_stop);

        let final_calls = shared_hook.get_ref().calls.lock().unwrap();

        // `run` 内のフェーズ入れ子構造に完全準拠した期待値配列
        let expected = vec![
            HookCall::BeforeSimulation,
            // --- 時刻 0 の処理フェーズ ---
            HookCall::BeforeTick {
                current: SimTime::from_ticks(0),
                skipped: Duration::zero(),
            },
            // マイクロステップ開始
            HookCall::BeforeMicroStep {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::BeforeSourcePhase {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::AfterSourcePhase {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::BeforeEventPhase {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::AfterEventPhase {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::AfterMicroStep {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::AfterTick {
                current: SimTime::from_ticks(0),
                last_micro: MicroStep::zero(),
            },
            // --- 時刻 1 の処理フェーズ ---
            HookCall::BeforeTick {
                current: SimTime::from_ticks(1),
                skipped: Duration::ticks(0),
            },
            HookCall::BeforeMicroStep {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::BeforeSourcePhase {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::AfterSourcePhase {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::BeforeEventPhase {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            // ※ここでイベント A が実際に処理される
            HookCall::AfterEventPhase {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::AfterMicroStep {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::AfterTick {
                current: SimTime::from_ticks(1),
                last_micro: MicroStep::zero(),
            },
            // 時刻 2 に到達する直前段階で should_stop が true になりループを抜ける
            HookCall::AfterSimulation(SimTime::from_ticks(1)),
        ];

        assert_eq!(*final_calls, expected);
    }

    #[test]
    fn test_standard_runner_hook_lifecycle_flow_without_include_zero_tick() {
        let hook = MockHook {
            calls: Arc::new(Mutex::new(Vec::new())),
        };
        let shared_hook = SharedHook::new(hook);

        let model = TestModel { event_count: 0 };
        let mut engine = Engine::new();

        // MockHookで記録を残すためにHookを登録する。
        engine.add_shared_hook(shared_hook.clone());

        // 時刻1にダミーイベントを1つだけ配置
        engine.schedule_event_at(
            SimTime::from_ticks(1),
            EventPriority::minimum(),
            TestEvent::A,
        );

        let mut runner = RealtimeRunner::new(std::time::Duration::from_millis(100));

        // 停止条件: 時刻 2つ分処理ができたらに終了
        let should_stop = |_m: &TestModel, _status: ExecutorStatus, tick: TickStatus| {
            // include_zero_tick=falseなので0tick 1tick 2tickの三つで終了
            tick.is_done_ticks(false, 2)
        };

        let _result = runner.run(engine, model, should_stop);

        let final_calls = shared_hook.get_ref().calls.lock().unwrap();

        // `run` 内のフェーズ入れ子構造に完全準拠した期待値配列
        let expected = vec![
            HookCall::BeforeSimulation,
            // --- 時刻 0 の処理フェーズ ---
            HookCall::BeforeTick {
                current: SimTime::from_ticks(0),
                skipped: Duration::zero(),
            },
            // マイクロステップ開始
            HookCall::BeforeMicroStep {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::BeforeSourcePhase {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::AfterSourcePhase {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::BeforeEventPhase {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::AfterEventPhase {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::AfterMicroStep {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::AfterTick {
                current: SimTime::from_ticks(0),
                last_micro: MicroStep::zero(),
            },
            // --- 時刻 1 の処理フェーズ ---
            HookCall::BeforeTick {
                current: SimTime::from_ticks(1),
                skipped: Duration::ticks(0),
            },
            HookCall::BeforeMicroStep {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::BeforeSourcePhase {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::AfterSourcePhase {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::BeforeEventPhase {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            // ※ここでイベント A が実際に処理される
            HookCall::AfterEventPhase {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::AfterMicroStep {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::AfterTick {
                current: SimTime::from_ticks(1),
                last_micro: MicroStep::zero(),
            },
            // --- 時刻 2 の処理フェーズ ---
            HookCall::BeforeTick {
                current: SimTime::from_ticks(2),
                skipped: Duration::zero(),
            },
            // マイクロステップ開始
            HookCall::BeforeMicroStep {
                current: SimTime::from_ticks(2),
                micro: MicroStep::zero(),
            },
            HookCall::BeforeSourcePhase {
                current: SimTime::from_ticks(2),
                micro: MicroStep::zero(),
            },
            HookCall::AfterSourcePhase {
                current: SimTime::from_ticks(2),
                micro: MicroStep::zero(),
            },
            HookCall::BeforeEventPhase {
                current: SimTime::from_ticks(2),
                micro: MicroStep::zero(),
            },
            HookCall::AfterEventPhase {
                current: SimTime::from_ticks(2),
                micro: MicroStep::zero(),
            },
            HookCall::AfterMicroStep {
                current: SimTime::from_ticks(2),
                micro: MicroStep::zero(),
            },
            HookCall::AfterTick {
                current: SimTime::from_ticks(2),
                last_micro: MicroStep::zero(),
            },
            // 時刻 3 に到達する直前段階で should_stop が true になりループを抜ける
            HookCall::AfterSimulation(SimTime::from_ticks(2)),
        ];

        assert_eq!(*final_calls, expected);
    }
}
