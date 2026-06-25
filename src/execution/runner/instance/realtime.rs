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
/// ※ [Model]と[Source]と[Hook]が決定論的に動くとき決定論的に処理を行うことができる。
pub struct RealtimeRunner<CS> {
    continue_strategy: CS,
    /// 1 Tickの処理に必要な実時間（例: `Duration::from_millis(100)` で 1秒間に10Ticks）。
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
            let next_tick_value = tick_status.current().as_tick_value();

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
