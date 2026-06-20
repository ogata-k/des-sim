use crate::execution::engine::{Engine, ExecutorStatus};
use crate::execution::runner::Runner;
use crate::execution::utility::{MicroStepResult, SimulationResult, TickStatus};
use crate::world::model::Model;

pub struct StandardRunner {
    skippable: bool,
}

impl<E, M: Model<E>> Runner<E, M> for StandardRunner {
    type Err = ();

    fn run<F>(
        &mut self,
        engine: Engine<E, M>,
        mut model: M,
        mut should_stop: F,
    ) -> SimulationResult<M, Self::Err>
    where
        F: FnMut(&M, ExecutorStatus, TickStatus) -> bool,
    {
        // 最初に生成されるのは「待機状態」の executor
        let mut executor = engine.begin_simulation();

        loop {
            let (executor_status, tick_status) = executor.peek_next_tick();
            if should_stop(&model, executor_status, tick_status) {
                // ここで抜ける時は、手元に executor の所有権があるので
                // ループ外のend_simulation(model)に安全に渡せる
                break;
            }

            // これを呼ばないと、次の行の `active_executor` が作れないため、
            // 下のMicroStepループやフェーズ処理（シミュレータの本体）が1文字も書けない。
            let mut active_executor = executor.begin_tick();

            loop {
                // 1. マイクロステップ開始
                let micro_step_handler = active_executor.begin_micro_step();

                // 2. Sourceフェーズ
                let mut source_phase = micro_step_handler.start_source_phase();
                while let Some(source_ready) = source_phase.take_one() {
                    source_phase.fire_and_schedule(&model, source_ready);
                }
                let micro_step_handler = source_phase.finish_source_phase();

                // 3. Eventフェーズ
                let mut event_phase = micro_step_handler.to_event_phase();
                while let Some(event_ready) = event_phase.take_one() {
                    event_phase.handle_event(&mut model, event_ready);
                }
                let micro_step_handler = event_phase.finish_event_phase();

                // 4. マイクロステップ終了
                match micro_step_handler.end_micro_step() {
                    MicroStepResult::Continue(new_active_executor, _) => {
                        // 次のループでactive_executorを呼ぶためにしっかり所有権を回収してからcontinueする。
                        // もしここで次のマイクロステップを見て終了せずに抜ける場合は、マイクロステップ内の残りのソースやイベントを破棄する必要がある。
                        active_executor = new_active_executor;
                        continue;
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
                active_executor.end_tick_with_jump_to_next_tick()
            } else {
                active_executor.end_tick_with_increment_tick()
            };
        }

        // 綺麗にすべてのTickが閉じた executorで終了
        executor.end_simulation_as_ok(model)
    }
}

impl StandardRunner {
    pub fn new(skippable: bool) -> Self {
        StandardRunner { skippable }
    }
}
