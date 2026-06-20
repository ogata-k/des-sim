mod event;
mod handler;
mod source;

use crate::context::ActiveExecutorContext;
use crate::modeling::model::Model;
use crate::primitive::time::MicroStepStatus;
use crate::primitive::time::{MicroStep, SimTime};
pub use event::*;
pub use handler::*;
pub use source::*;

/// ポリシーが処理する前の「未判定状態」のExecutorラッパー
pub struct UncheckedActiveExecutor<E, M: Model<E>> {
    active_executor: ActiveExecutorContext<E, M>,
    current_micro_step: MicroStep,
}

impl<E, M: Model<E>> UncheckedActiveExecutor<E, M> {
    pub(crate) fn new(
        executor: ActiveExecutorContext<E, M>,
        current_micro_step: MicroStep,
    ) -> Self {
        Self {
            active_executor: executor,
            current_micro_step,
        }
    }

    pub fn current_tick(&self) -> SimTime {
        self.active_executor.current_tick_status.current()
    }

    pub fn current_micro_step(&self) -> MicroStep {
        self.current_micro_step
    }

    pub fn into_active_executor(self) -> ActiveExecutorContext<E, M> {
        self.active_executor
    }
}

pub enum MicroStepResult<E, M: Model<E>> {
    /// 1 micro step分のシミュレーションを継続可能
    Continue(UncheckedActiveExecutor<E, M>),
    /// 1 micro step分のシミュレーションが完了（停止条件に到達）
    /// 第二引数の[MicroStepStatus]は最後の状態
    Complete(ActiveExecutorContext<E, M>, MicroStepStatus),
}
