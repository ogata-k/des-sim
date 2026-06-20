pub mod instance;

use crate::context::ExecutorStatus;
use crate::execution::SimulationResult;
use crate::execution::engine::Engine;
use crate::modeling::model::Model;
use crate::primitive::time::TickStatus;

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
