use crate::execution::phase::UncheckedActiveExecutor;
use crate::execution::strategy::{ContinueStrategy, ContinuousStrategyResult};
use crate::modeling::model::Model;

/// 指定されたマイクロステップ上限に達したらそのtick内で処理すべきイベントなどがまだ残っている場合は破棄して継続する継続戦略
pub struct LimitDiscardStrategy {
    limit_micro_step_count: u64,
}

impl<E, M: Model<E>, RunnerError> ContinueStrategy<E, M, RunnerError> for LimitDiscardStrategy {
    type Err = RunnerError;

    fn handle_micro_step_continue(
        &mut self,
        model: &M,
        unchecked: UncheckedActiveExecutor<E, M>,
    ) -> ContinuousStrategyResult<E, M, Self::Err> {
        let current_micro_step = unchecked.current_micro_step();

        // 上限未達
        if current_micro_step.value() < self.limit_micro_step_count {
            return Ok(unchecked.into_active_executor());
        }

        let mut next_active = unchecked.into_active_executor();
        // まだ継続するので現在のtickで残っている処理すべきイベントをすべて破棄
        next_active.discard_remain_micro_step(model);
        Ok(next_active)
    }
}

impl LimitDiscardStrategy {
    pub fn new(limit_micro_step_count: u64) -> Self {
        LimitDiscardStrategy {
            limit_micro_step_count,
        }
    }
}
