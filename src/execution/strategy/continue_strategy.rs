mod always_continue;
mod limit_abort;
mod limit_discard;

pub use always_continue::*;
pub use limit_abort::*;
pub use limit_discard::*;

use crate::context::ActiveExecutorContext;
use crate::execution::phase::UncheckedActiveExecutor;
use crate::modeling::model::Model;

/// [ContinueStrategy]実行結果の型エイリアス
pub type ContinuousStrategyResult<E, M, Err> =
    Result<ActiveExecutorContext<E, M>, (ActiveExecutorContext<E, M>, Err)>;

/// [Runner](crate::execution::runner::Runner)の継続戦略
pub trait ContinueStrategy<E, M: Model<E>, RunnerError> {
    type Err;

    #[allow(clippy::result_large_err)]
    fn handle_micro_step_continue(
        &mut self,
        model: &M,
        unchecked_executor: UncheckedActiveExecutor<E, M>,
    ) -> ContinuousStrategyResult<E, M, Self::Err>;
}
