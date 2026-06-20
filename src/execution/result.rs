use crate::primitive::time::SimTime;

/// シミュレーション実行結果の型エイリアス
pub type SimulationResult<M, Err> = Result<SimulationOutput<M>, SimulationError<M, Err>>;

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
