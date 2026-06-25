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

pub trait AsyncModel<E, Command>: Model<E> {
    /// 非同期（並列）スレッド側から呼ばれる、イミュータブルなイベントハンドラー。
    /// 計算結果や状態変更の要求を `tx` を通じてメインスレッドに送信する。
    fn handle_event_async(&self, event: Event<E>, tx: Sender<Command>);

    /// メインスレッド側で、非同期スレッド群から集約されたコマンドを順次適用する。
    /// ここでは `&mut self` と `EventContext` が使えるため、安全に状態更新や新規イベントのスケジュールが可能。
    fn apply_command(&mut self, context: &mut EventContext<E, Self>, command: Command)
    where
        Self: Sized;
}

/// イベントを発火した順番に並列で処理する標準的なRunner。
/// sync_priority_thresholdで指定された以上の[EventPriority]以上のイベントは直列で処理することもできるようになっている。
/// そして、skippableがtrueであれば、イベントがない時間をスキップすることができる。
pub struct AsyncRunner<Command, CS> {
    skippable: bool,
    continue_strategy: CS,
    /// この閾値以上のプライオリティを持つイベントは同期処理する
    sync_priority_threshold: EventPriority,
    _command: PhantomData<Command>,
}

impl<E, Command, M: AsyncModel<E, Command>, CS: ContinueStrategy<E, M, ()>> Runner<E, M, CS>
    for AsyncRunner<Command, CS>
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
                        let async_events = event_phase.take_all();

                        // コマンド受信用のチャンネルを用意
                        let (tx, rx) = std::sync::mpsc::channel();

                        // 💡 技ありポイント: std::thread::scope を被せることで、
                        // Rayonのスレッドプールに対しても &model (イミュータブル参照) を安全に貸し出せます。
                        std::thread::scope(|_scope| {
                            use rayon::prelude::*;

                            // tx の所有権を scope 内に移動させ、自動ドロップを狙う
                            let tx = tx;
                            // スレッドプールで一斉に並列処理
                            async_events.into_par_iter().for_each_with(
                                tx,
                                |tx_worker, event_ready| {
                                    let model_ref = &model; // &model はすべてのスレッドで安全に共有される

                                    // スレッドは新しく作られず、常駐しているスレッドが超高速にこの関数を実行する
                                    model_ref.handle_event_async(event_ready, tx_worker.clone());
                                },
                            ); // ここで scope 内の tx が自動ドロップされる ＆ 全並列処理の完了が保証される
                        });

                        // 溜まったコマンドをメインスレッドで一括適用
                        while let Ok(command) = rx.try_recv() {
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
        }

        if let Some(error) = runner_error.take() {
            executor.end_simulation_as_error(model, error)
        } else {
            executor.end_simulation_as_ok(model)
        }
    }
}

impl<E, Command, M: AsyncModel<E, Command>> AsyncRunner<Command, AlwaysContinueStrategy<E, M>> {
    pub fn new(skippable: bool, sync_priority_threshold: EventPriority) -> Self {
        AsyncRunner {
            skippable,
            continue_strategy: AlwaysContinueStrategy::new(),
            sync_priority_threshold,
            _command: PhantomData,
        }
    }
}

impl<Command, CS> AsyncRunner<Command, CS> {
    pub fn new_with_continue_strategy(
        skippable: bool,
        sync_priority_threshold: EventPriority,
        continue_strategy: CS,
    ) -> Self {
        AsyncRunner {
            skippable,
            continue_strategy,
            sync_priority_threshold,
            _command: PhantomData,
        }
    }
}
