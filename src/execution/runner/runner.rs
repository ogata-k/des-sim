use crate::execution::engine::{Engine, ExecutorStatus};
use crate::execution::utility::{SimulationResult, TickStatus};
use crate::world::model::Model;

pub trait Runner<E, M: Model<E>> {
    type Err: std::fmt::Debug;

    fn run<F>(
        &mut self,
        engine: Engine<E, M>,
        model: M,
        should_stop: F,
    ) -> SimulationResult<M, Self::Err>
    where
        // 内部で散開エラーが起きたら終了するみたいなことをやるためにFnMutとしている。
        F: FnMut(&M, ExecutorStatus, TickStatus) -> bool;
}
