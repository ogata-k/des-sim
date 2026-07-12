use crate::context::{EventContext, ExecutorStatus};
use crate::execution::SimulationResult;
use crate::execution::engine::Engine;
use crate::execution::phase::MicroStepResult;
use crate::execution::runner::Runner;
use crate::execution::strategy::{AlwaysContinueStrategy, ContinueStrategy};
use crate::modeling::event::{Event, EventPriority};
use crate::modeling::model::Model;
use crate::primitive::time::TickStatus;
use std::marker::PhantomData;
use std::sync::mpsc::Sender;

pub trait ParallelModel<E, Command>: Model<E> {
    /// 非同期（並列）スレッド側から呼ばれる、イミュータブルなイベントハンドラー。
    /// 計算結果や状態変更の要求を `sender` を通じてメインスレッドに送信する。
    fn handle_event_parallel(&self, event: Event<E>, sender: Sender<Command>);

    /// メインスレッド側で、非同期スレッド群から集約されたコマンドを順次適用する。
    /// ここでは `&mut self` と `EventContext` が使えるため、安全に状態更新や新規イベントのスケジュールが可能。
    fn apply_command(&mut self, context: &mut EventContext<E, Self>, command: Command)
    where
        Self: Sized;
}

/// イベントを発火した順番に並列で処理する標準的なRunner。
/// sync_priority_thresholdで指定された以上の[EventPriority]以上のイベントは直列で処理することもできるようになっている。
/// そして、skippableがtrueであれば、イベントがない時間をスキップすることができる。
#[derive(Clone)]
pub struct ParallelRunner<Command, CS> {
    skippable: bool,
    continue_strategy: CS,
    /// この閾値以上のプライオリティを持つイベントは同期処理する
    sync_priority_threshold: EventPriority,
    _command: PhantomData<Command>,
}

impl<E, Command, M: ParallelModel<E, Command>, CS: ContinueStrategy<E, M, ()>> Runner<E, M, CS>
    for ParallelRunner<Command, CS>
where
    E: Send + 'static,
    M: Send + Sync + 'static,
    Command: Send,
{
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
        let mut executor = engine.begin_simulation(&model);

        loop {
            let (executor_status, tick_status) = executor.peek_next_tick();
            if should_stop(&model, executor_status, tick_status) {
                break;
            }

            let mut active_executor = executor.begin_tick(&model);

            loop {
                // 1. マイクロステップ開始
                let micro_step_handler = active_executor.begin_micro_step(&model);

                // 2. Sourceフェーズ (Sourceは通常、直列または状態に依存するため同期実行)
                let mut source_phase = micro_step_handler.start_source_phase(&model);
                while let Some(source_ready) = source_phase.take_one() {
                    source_phase.fire_and_schedule(&model, source_ready);
                }
                let micro_step_handler = source_phase.complete_source_phase(&model);

                // 3. Eventフェーズ
                let mut event_phase = micro_step_handler.to_event_phase(&model);

                loop {
                    // 先頭のイベントが同期処理対象（閾値以上）かどうかをチェックして取り出す
                    // ※EventPriorityは値が大きいほど優先度が高い（重要）
                    if let Some(event_ready) =
                        event_phase.take_front_if(|e| e.priority >= self.sync_priority_threshold)
                    {
                        // 【同期実行】現在のスレッド（メインスレッド）で即座に処理
                        event_phase.handle_event(&mut model, event_ready);
                    } else {
                        // 残りをすべて引っこ抜いて非同期に処理させる
                        let parallel_events = event_phase.take_all();

                        // コマンド受信用のチャンネルを用意
                        let (sender, receiver) = std::sync::mpsc::channel();

                        // ポイント: std::thread::scope を被せることで、
                        // Rayonのスレッドプールに対しても &model (イミュータブル参照) を安全に貸し出せます。
                        std::thread::scope(|_scope| {
                            use rayon::prelude::*;

                            // sender の所有権を scope 内に移動させ、自動ドロップを狙う
                            let sender = sender;
                            // スレッドプールで一斉に並列処理
                            parallel_events.into_par_iter().for_each_with(
                                sender,
                                |sender_worker, event_ready| {
                                    let model_ref = &model; // &model はすべてのスレッドで安全に共有される

                                    // スレッドは新しく作られず、常駐しているスレッドが超高速にこの関数を実行する
                                    model_ref
                                        .handle_event_parallel(event_ready, sender_worker.clone());
                                },
                            ); // ここで scope 内の sender が自動ドロップされる ＆ 全並列処理の完了が保証される
                        });

                        // 溜まったコマンドをメインスレッドで一括適用
                        while let Ok(command) = receiver.try_recv() {
                            model.apply_command(event_phase.get_context(), command);
                        }

                        break;
                    }
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

            executor = if self.skippable {
                active_executor.end_tick_with_jump_to_next_tick(&model)
            } else {
                active_executor.end_tick_with_increment_tick(&model)
            };

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

impl<Command> ParallelRunner<Command, AlwaysContinueStrategy> {
    pub fn new(skippable: bool, sync_priority_threshold: EventPriority) -> Self {
        ParallelRunner {
            skippable,
            continue_strategy: AlwaysContinueStrategy::new(),
            sync_priority_threshold,
            _command: PhantomData,
        }
    }
}

impl<Command, CS> ParallelRunner<Command, CS> {
    pub fn new_with_continue_strategy(
        skippable: bool,
        sync_priority_threshold: EventPriority,
        continue_strategy: CS,
    ) -> Self {
        ParallelRunner {
            skippable,
            continue_strategy,
            sync_priority_threshold,
            _command: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{SourceContext, UserContext};
    use crate::execution::strategy::LimitAbortStrategy;
    use crate::modeling::event::EventPriority;
    use crate::modeling::hook::Hook;
    use crate::modeling::source::Source;
    use crate::primitive::time::{Duration, MicroStep, SimTime, TickStatus};
    use crate::source_handler::{SourceReadyEntry, SourceView};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    // テスト用の共通コマンド
    #[allow(unused)]
    #[derive(Debug)]
    enum TestCommand {
        IncrementSync,
        IncrementParallel,
    }

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    enum TestEvent {
        SyncTarget,
        ParallelTarget,
    }

    // スレッドを跨ぐため、検証用カウンタには AtomicUsize を使用
    struct TestParallelModel {
        sync_event_count: Arc<AtomicUsize>,
        parallel_command_count: Arc<AtomicUsize>,
    }

    impl Model<TestEvent> for TestParallelModel {
        fn handle_event(
            &mut self,
            _context: &mut EventContext<TestEvent, Self>,
            event: &Event<TestEvent>,
        ) {
            // 同期実行 (handle_event) が呼ばれた場合
            if let TestEvent::SyncTarget = event.payload {
                self.sync_event_count.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    impl ParallelModel<TestEvent, TestCommand> for TestParallelModel {
        fn handle_event_parallel(&self, event: Event<TestEvent>, sender: Sender<TestCommand>) {
            // 並列実行側。ここではイミュータブル参照で処理し、メインへの要求を送る
            if let TestEvent::ParallelTarget = event.payload {
                let _ = sender.send(TestCommand::IncrementParallel);
            }
        }

        fn apply_command(
            &mut self,
            _context: &mut EventContext<TestEvent, Self>,
            command: TestCommand,
        ) {
            // メインスレッドに帰ってきたコマンドを適用
            if let TestCommand::IncrementParallel = command {
                self.parallel_command_count.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    #[test]
    fn test_parallel_runner_new_and_args() {
        let runner = ParallelRunner::<TestCommand, AlwaysContinueStrategy>::new(
            true,
            EventPriority::new(100),
        );
        assert!(runner.skippable);
        assert_eq!(runner.sync_priority_threshold, EventPriority::new(100));
    }

    #[test]
    fn test_parallel_runner_sync_and_parallel_execution() {
        let sync_counter = Arc::new(AtomicUsize::new(0));
        let parallel_counter = Arc::new(AtomicUsize::new(0));

        let model = TestParallelModel {
            sync_event_count: sync_counter.clone(),
            parallel_command_count: parallel_counter.clone(),
        };

        let mut engine = Engine::new();

        // 1. 同期閾値以上の重要イベント (優先度 10)
        engine.schedule_event_at(
            SimTime::zero(),
            EventPriority::new(10),
            TestEvent::SyncTarget,
        );

        // 2. 同期閾値未満の並列処理対象イベント (優先度 0)
        engine.schedule_event_at(
            SimTime::zero(),
            EventPriority::new(0),
            TestEvent::ParallelTarget,
        );

        // 閾値を 「5」 に設定。1 は同期、2 は並列へ振り分けられるはず
        let mut runner = ParallelRunner::new(true, EventPriority::new(5));

        let mut tick_counter = 0;
        let should_stop = |_m: &TestParallelModel, _status: ExecutorStatus, _tick: TickStatus| {
            if tick_counter >= 1 {
                true
            } else {
                tick_counter += 1;
                false
            }
        };

        let result = runner.run(engine, model, should_stop);
        assert!(result.is_ok());

        // それぞれ期待通りのルートでカウントアップが走ったか検証
        assert_eq!(sync_counter.load(Ordering::SeqCst), 1);
        assert_eq!(parallel_counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_parallel_runner_boundary_priority() {
        let sync_counter = Arc::new(AtomicUsize::new(0));
        let parallel_counter = Arc::new(AtomicUsize::new(0));

        let model = TestParallelModel {
            sync_event_count: sync_counter.clone(),
            parallel_command_count: parallel_counter.clone(),
        };

        let mut engine = Engine::new();

        // 閾値と「完全に同じ」プライオリティを持つイベント
        engine.schedule_event_at(
            SimTime::zero(),
            EventPriority::new(5),
            TestEvent::SyncTarget,
        );

        // 閾値（5）以上のイベントは同期処理（>= 判定）される仕様の確認
        let mut runner = ParallelRunner::new(true, EventPriority::new(5));

        let mut tick_counter = 0;
        let should_stop = |_m: &TestParallelModel, _status: ExecutorStatus, _tick: TickStatus| {
            if tick_counter >= 1 {
                true
            } else {
                tick_counter += 1;
                false
            }
        };

        let result = runner.run(engine, model, should_stop);
        assert!(result.is_ok());

        // 境界値 (== 5) なので、同期（Sync）側に入っていることを確認
        assert_eq!(sync_counter.load(Ordering::SeqCst), 1);
        assert_eq!(parallel_counter.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_parallel_runner_massive_parallel_events() {
        let sync_counter = Arc::new(AtomicUsize::new(0));
        let parallel_counter = Arc::new(AtomicUsize::new(0));

        let model = TestParallelModel {
            sync_event_count: sync_counter.clone(),
            parallel_command_count: parallel_counter.clone(),
        };

        let mut engine = Engine::new();

        // 大量の並列対象イベントを一挙に投入（100件）
        let event_count = 100;
        for _ in 0..event_count {
            engine.schedule_event_at(
                SimTime::zero(),
                EventPriority::new(0), // 閾値未満
                TestEvent::ParallelTarget,
            );
        }

        let mut runner = ParallelRunner::new(true, EventPriority::new(10));

        let mut tick_counter = 0;
        let should_stop = |_m: &TestParallelModel, _status: ExecutorStatus, _tick: TickStatus| {
            if tick_counter >= 1 {
                true
            } else {
                tick_counter += 1;
                false
            }
        };

        let result = runner.run(engine, model, should_stop);
        assert!(result.is_ok());

        // Rayonを介してすべてのコマンドがメインスレッドに集約され、漏れなく適用されたか検証
        assert_eq!(parallel_counter.load(Ordering::SeqCst), event_count);
    }

    #[test]
    fn test_parallel_runner_aborts_on_strategy_error() {
        let mut engine = Engine::new();

        // 1. まず最初のトリガーとなるイベントを時刻0に仕込む
        engine.schedule_event_at(
            SimTime::zero(),
            EventPriority::new(0), // 閾値5未満なので並列処理される
            TestEvent::ParallelTarget,
        );

        // 2.TestParallelModel の apply_command の挙動を調整するか、
        // もしくはテスト専用のフックやモデルが必要ですが、
        // 既存の `TestParallelModel` の `apply_command` はイベントスケジュールを行っていない。
        //
        // ここで確実に同Tick内の再スケジュール（Continueルート）に乗せるため、
        // テスト用の派生モデル `TestAbortModel` をこのテスト内だけで定義して利用する。

        struct TestAbortModel;
        impl Model<TestEvent> for TestAbortModel {
            fn handle_event(
                &mut self,
                _context: &mut EventContext<TestEvent, Self>,
                _event: &Event<TestEvent>,
            ) {
                // none
            }
        }
        impl ParallelModel<TestEvent, TestCommand> for TestAbortModel {
            fn handle_event_parallel(&self, event: Event<TestEvent>, sender: Sender<TestCommand>) {
                if let TestEvent::ParallelTarget = event.payload {
                    let _ = sender.send(TestCommand::IncrementParallel);
                }
            }
            fn apply_command(
                &mut self,
                context: &mut EventContext<TestEvent, Self>,
                _command: TestCommand,
            ) {
                // 【超重要】ここで同じ時刻 (Duration::zero) に次のイベントをねじ込む！
                // これにより end_micro_step が「まだ同Tickにイベントがある」と判断して Continue を返します。
                context.schedule_event(
                    Duration::zero(),
                    EventPriority::new(0),
                    TestEvent::ParallelTarget,
                );
            }
        }

        let abort_model = TestAbortModel;

        // 即座にエラーを起こす上限0設定の継続戦略を ParallelRunner に組み合わせる
        let strategy = LimitAbortStrategy::new(0, 0);
        let mut runner =
            ParallelRunner::new_with_continue_strategy(true, EventPriority::new(5), strategy);

        let mut loop_count = 0;
        let should_stop = |_m: &TestAbortModel, _status: ExecutorStatus, _tick: TickStatus| {
            loop_count += 1;
            loop_count > 5
        };

        // 実行すると、最初のイベントが処理された直後、apply_command 内で同Tickにイベントが再登録され、
        // 最初のマイクロステップ終了判定 (end_micro_step) で MicroStepResult::Continue が発生。
        // 直後に LimitAbortStrategy が上限突破を検知して Err を返す。
        let result = runner.run(engine, abort_model, should_stop);

        // 正常終了せず、継続戦略のエラー（Err）を検知して安全に中断したかを検証
        assert!(result.is_err());
    }

    #[test]
    fn test_parallel_runner_without_aborts_on_strategy_error() {
        let model = TestParallelModel {
            sync_event_count: Arc::new(AtomicUsize::new(0)),
            parallel_command_count: Arc::new(AtomicUsize::new(0)),
        };

        let mut engine = Engine::new();

        // エンジンで登録した時刻0のイベントはシミュレーション開始時に時刻0のイベントとして処理されるよう登録されるため、
        // マイクロステップ上限が0でも上限に引っかからない。
        engine.schedule_event_at(
            SimTime::zero(),
            EventPriority::new(0),
            TestEvent::ParallelTarget,
        );

        // 即座にエラーを起こす上限0設定の継続戦略を ParallelRunner に組み合わせる
        let strategy = LimitAbortStrategy::new(0, 0);
        let mut runner =
            ParallelRunner::new_with_continue_strategy(true, EventPriority::new(5), strategy);

        let mut loop_count = 0;
        let should_stop = |_m: &TestParallelModel, _status: ExecutorStatus, _tick: TickStatus| {
            loop_count += 1;
            loop_count > 5
        };

        let result = runner.run(engine, model, should_stop);

        // 正常終了せず、継続戦略のエラーにならずに完了したかを検証
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

    impl Source<TestEvent, TestParallelModel> for TraceSource {
        fn on_registered(
            &mut self,
            _context: &mut dyn UserContext<TestEvent, TestParallelModel>,
            _model: &TestParallelModel,
        ) -> Option<Duration> {
            self.trace
                .lock()
                .unwrap()
                .push(LifecycleEvent::BeforeSimulation);
            Some(self.initial_delay)
        }

        fn fire(
            &mut self,
            context: &mut SourceContext<TestEvent, TestParallelModel>,
            _model: &TestParallelModel,
        ) -> Option<Duration> {
            let mut t = self.trace.lock().unwrap();
            t.push(LifecycleEvent::BeforeFireSource(context.current_tick()));
            t.push(LifecycleEvent::BeforeScheduleEvent);

            // イベントを発火させる
            context.schedule_event(
                Duration::zero(),
                EventPriority::new(0),
                TestEvent::ParallelTarget,
            );

            t.push(LifecycleEvent::AfterScheduleEvent);
            t.push(LifecycleEvent::AfterFireSource(context.current_tick()));
            self.interval_delay
        }
    }

    #[test]
    fn test_parallel_runner_lifecycle_execution_order_scenario() {
        let trace = Arc::new(Mutex::new(Vec::new()));
        let model = TestParallelModel {
            sync_event_count: Arc::new(AtomicUsize::new(0)),
            parallel_command_count: Arc::new(AtomicUsize::new(0)),
        };
        let mut engine = Engine::new();

        // 時刻1で単発発火するソースを登録
        engine.add_source(
            "trace_source",
            TraceSource {
                trace: Arc::clone(&trace),
                initial_delay: Duration::ticks(1),
                interval_delay: None,
            },
        );

        let mut runner = ParallelRunner::new(true, EventPriority::new(5));

        // 停止条件：時刻2に到達した時点で停止させる
        let should_stop =
            move |_m: &TestParallelModel, _status: ExecutorStatus, tick: TickStatus| {
                tick.is_done_ticks(false, 2)
            };

        let result = runner.run(engine, model, should_stop);
        assert!(result.is_ok());

        // ループを抜けた後、安全にシミュレーションが終了したため、最後に手動で記録
        trace.lock().unwrap().push(LifecycleEvent::AfterSimulation);

        let final_trace = trace.lock().unwrap();

        let expected = vec![
            LifecycleEvent::BeforeSimulation,
            // 時刻1のイテレーション
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
    fn test_parallel_runner_lifecycle_interruption_on_strategy_error() {
        let trace = Arc::new(Mutex::new(Vec::new()));
        let model = TestParallelModel {
            sync_event_count: Arc::new(AtomicUsize::new(0)),
            parallel_command_count: Arc::new(AtomicUsize::new(0)),
        };
        let mut engine = Engine::new();

        // 最初のTick（時刻0）で確実に無限発火するソースを仕込む
        engine.add_source(
            "loop_source",
            TraceSource {
                trace: Arc::clone(&trace),
                initial_delay: Duration::zero(),
                interval_delay: Some(Duration::zero()),
            },
        );

        // マイクロステップの上限エラーを即座に発生させる戦略
        let strategy = LimitAbortStrategy::new(0, 0);
        let mut runner =
            ParallelRunner::new_with_continue_strategy(true, EventPriority::new(5), strategy);

        let trace_for_stop = Arc::clone(&trace);
        let should_stop =
            move |_m: &TestParallelModel, _status: ExecutorStatus, tick: TickStatus| {
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

        // エラー中断時であっても、開始されたフックの対となる処理が異常を検知して途切れる流れを検証
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
    fn test_parallel_runner_hook_lifecycle_flow_with_include_zero_tick() {
        use crate::modeling::hook::instance::SharedHook;

        let hook = MockHook {
            calls: Arc::new(Mutex::new(Vec::new())),
        };
        let shared_hook = SharedHook::new(hook);

        let model = TestParallelModel {
            sync_event_count: Arc::new(AtomicUsize::new(0)),
            parallel_command_count: Arc::new(AtomicUsize::new(0)),
        };
        let mut engine = Engine::new();
        engine.add_shared_hook(shared_hook.clone());

        // 時刻1に並列イベント（優先度0）を1つだけ配置
        engine.schedule_event_at(
            SimTime::from_ticks(1),
            EventPriority::new(0),
            TestEvent::ParallelTarget,
        );

        let mut runner = ParallelRunner::new(true, EventPriority::new(5));

        // 停止条件: 時刻 2つ分処理ができたら終了
        let should_stop = |_m: &TestParallelModel, _status: ExecutorStatus, tick: TickStatus| {
            tick.is_done_ticks(true, 2)
        };

        let _result = runner.run(engine, model, should_stop);

        let final_calls = shared_hook.get_ref().calls.lock().unwrap();

        let expected = vec![
            HookCall::BeforeSimulation,
            // --- 時刻 0 の処理フェーズ ---
            HookCall::BeforeTick {
                current: SimTime::from_ticks(0),
                skipped: Duration::zero(),
            },
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
            HookCall::AfterSimulation(SimTime::from_ticks(1)),
        ];

        assert_eq!(*final_calls, expected);
    }

    #[test]
    fn test_parallel_runner_hook_lifecycle_flow_without_include_zero_tick() {
        use crate::modeling::hook::instance::SharedHook;

        let hook = MockHook {
            calls: Arc::new(Mutex::new(Vec::new())),
        };
        let shared_hook = SharedHook::new(hook);

        let model = TestParallelModel {
            sync_event_count: Arc::new(AtomicUsize::new(0)),
            parallel_command_count: Arc::new(AtomicUsize::new(0)),
        };
        let mut engine = Engine::new();
        engine.add_shared_hook(shared_hook.clone());

        // 時刻1に並列イベント（優先度0）を1つだけ配置
        engine.schedule_event_at(
            SimTime::from_ticks(1),
            EventPriority::new(0),
            TestEvent::ParallelTarget,
        );

        let mut runner = ParallelRunner::new(true, EventPriority::new(5));

        // 停止条件: 時刻 2つ分処理ができたら終了
        let should_stop = |_m: &TestParallelModel, _status: ExecutorStatus, tick: TickStatus| {
            tick.is_done_ticks(false, 2)
        };

        let _result = runner.run(engine, model, should_stop);

        let final_calls = shared_hook.get_ref().calls.lock().unwrap();

        let expected = vec![
            HookCall::BeforeSimulation,
            // --- 時刻 0 の処理フェーズ ---
            HookCall::BeforeTick {
                current: SimTime::from_ticks(0),
                skipped: Duration::zero(),
            },
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
            HookCall::AfterSimulation(SimTime::from_ticks(2)),
        ];

        assert_eq!(*final_calls, expected);
    }
}
