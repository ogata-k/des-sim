use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::{Duration, SimTime, TimeTick};
use rand::Rng;

/// 定数値をサンプリングするサンプラー。
/// 直接[Duration]を取得したいときは[ConstantSampler::sample_constant]を使う。
#[derive(Debug, Clone)]
pub struct ConstantSampler {
    value: TimeTick,
}

impl DurationSampler for ConstantSampler {
    fn sample(&mut self, _rng: &mut dyn Rng, _current_tick: SimTime) -> PendingDuration {
        // 乱数生成器を使わずに常に固定値を返す
        Duration::ticks(self.value).into()
    }
}

impl ConstantSampler {
    pub fn new<T: Into<TimeTick>>(value: T) -> Self {
        Self {
            value: value.into(),
        }
    }

    pub fn sample_constant(&self) -> Duration {
        // 乱数生成器を使わずに常に固定値を返す
        Duration::ticks(self.value)
    }
}
