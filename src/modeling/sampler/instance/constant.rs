use crate::modeling::sampler::DurationSampler;
use crate::primitive::time::{Duration, SimTime, TimeTick};
use rand::Rng;

/// 定数値をサンプリングするサンプラー。
/// 直接[Duration]を
pub struct ConstantSampler {
    value: TimeTick,
}

impl DurationSampler for ConstantSampler {
    fn try_sample(
        &mut self,
        rng: &mut dyn Rng,
        current_tick: SimTime,
        _try_count: u8,
    ) -> Option<Duration> {
        Some(self.sample(rng, current_tick))
    }

    fn sample(&mut self, _rng: &mut dyn Rng, _current_tick: SimTime) -> Duration {
        // 乱数生成器を使わずに常に固定値を返す
        Duration::ticks(self.value)
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
