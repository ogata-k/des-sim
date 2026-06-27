mod always_continue;
mod limit_abort;
mod limit_discard;

pub use always_continue::*;
pub use limit_abort::*;
pub use limit_discard::*;

use crate::context::ActiveExecutorContext;
use crate::execution::phase::UncheckedActiveExecutor;
use crate::modeling::model::Model;

/// [Runner](crate::execution::runner::Runner)の継続戦略
pub trait ContinueStrategy<E, M: Model<E>, RunnerError> {
    type Err;

    fn handle_micro_step_continue(
        &mut self,
        model: &M,
        unchecked_executor: UncheckedActiveExecutor<E, M>,
    ) -> Result<ActiveExecutorContext<E, M>, (ActiveExecutorContext<E, M>, Self::Err)>;
}
