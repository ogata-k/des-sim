use crate::context::ActiveExecutorContext;
use crate::execution::phase::UncheckedActiveExecutor;
use crate::execution::strategy::ContinueStrategy;
use crate::modeling::model::Model;
use std::marker::PhantomData;

/// [Runner]のデフォルトの挙動のままの継続戦略
pub struct AlwaysContinueStrategy<E, M>(PhantomData<(E, M)>);

impl<E, M: Model<E>, RunnerError> ContinueStrategy<E, M, RunnerError>
    for AlwaysContinueStrategy<E, M>
{
    type Err = RunnerError;

    fn handle_micro_step_continue(
        &mut self,
        _model: &M,
        unchecked: UncheckedActiveExecutor<E, M>,
    ) -> Result<ActiveExecutorContext<E, M>, (ActiveExecutorContext<E, M>, Self::Err)> {
        Ok(unchecked.into_active_executor())
    }
}

impl<E, M> AlwaysContinueStrategy<E, M> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}
