mod event;
mod handler;
mod source;

use crate::context::ActiveExecutorContext;
use crate::modeling::model::Model;
use crate::primitive::time::MicroStep;
use crate::primitive::time::MicroStepStatus;
pub use event::*;
pub use handler::*;
pub use source::*;

pub enum MicroStepResult<E, M: Model<E>> {
    /// 1 micro step分のシミュレーションを継続可能
    /// 第二引数の[MicroStep]は次の状態
    Continue(ActiveExecutorContext<E, M>, MicroStep),
    /// 1 micro step分のシミュレーションが完了（停止条件に到達）
    /// 第二引数の[MicroStepStatus]は最後の状態
    Complete(ActiveExecutorContext<E, M>, MicroStepStatus),
}
