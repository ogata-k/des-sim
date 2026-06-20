use crate::execution::engine::ActiveExecutorContext;
use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::world::model::Model;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct TickStatus {
    current_tick: SimTime,
    skipped: Duration,
}
impl TickStatus {
    pub(crate) fn new(current_tick: SimTime, skipped: Duration) -> Self {
        TickStatus {
            current_tick,
            skipped,
        }
    }

    pub(crate) fn initialize() -> Self {
        TickStatus {
            current_tick: SimTime::zero(),
            skipped: Duration::zero(),
        }
    }

    pub fn current(&self) -> SimTime {
        self.current_tick
    }

    pub fn skipped(&self) -> Duration {
        self.skipped
    }

    pub fn previous(&self) -> SimTime {
        self.current_tick - self.skipped
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct MicroStepStatus {
    current_micro_step: MicroStep,
}

impl MicroStepStatus {
    pub(crate) fn new(current_micro_step: MicroStep) -> Self {
        MicroStepStatus { current_micro_step }
    }

    pub(crate) fn initialize() -> Self {
        MicroStepStatus {
            current_micro_step: MicroStep::zero(),
        }
    }

    pub fn current(&self) -> MicroStep {
        self.current_micro_step
    }
}

pub enum MicroStepResult<E, M: Model<E>> {
    /// シミュレーションを継続可能
    /// 第二引数の[MicroStep]は次の状態
    Continue(ActiveExecutorContext<E, M>, MicroStep),
    /// シミュレーションが完了（停止条件に到達）
    /// 第二引数の[MicroStepStatus]は最後の状態
    Complete(ActiveExecutorContext<E, M>, MicroStepStatus),
}

/// シミュレーションの最終的な出力結果
#[derive(Debug)]
pub struct SimulationOutput<M> {
    /// 終了時点のシミュレーション時刻
    time: SimTime,
    /// 終了時点のモデルの状態
    model: M,
}

impl<M> SimulationOutput<M> {
    pub(crate) fn new(time: SimTime, model: M) -> Self {
        Self { time, model }
    }

    pub fn last_tick(&self) -> SimTime {
        self.time
    }

    pub fn model(&self) -> &M {
        &self.model
    }

    pub fn model_mut(&mut self) -> &mut M {
        &mut self.model
    }
}

/// シミュレーションの最終的な出力結果
#[derive(Debug)]
pub struct SimulationError<M, Err> {
    /// 終了時点のシミュレーション時刻
    time: SimTime,
    /// 終了時点のモデルの状態
    model: M,
    /// 終了時点のエラー内容
    error: Err,
}

impl<M, Err> SimulationError<M, Err> {
    pub(crate) fn new(time: SimTime, model: M, error: Err) -> Self {
        Self { time, model, error }
    }

    pub fn last_tick(&self) -> SimTime {
        self.time
    }

    pub fn model(&self) -> &M {
        &self.model
    }

    pub fn model_mut(&mut self) -> &mut M {
        &mut self.model
    }

    pub fn error(&self) -> &Err {
        &self.error
    }

    pub fn error_mut(&mut self) -> &mut Err {
        &mut self.error
    }
}

/// シミュレーション実行結果の型エイリアス
pub type SimulationResult<M, Err> = Result<SimulationOutput<M>, SimulationError<M, Err>>;
