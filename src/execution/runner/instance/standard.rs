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
