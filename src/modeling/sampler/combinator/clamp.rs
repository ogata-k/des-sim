use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;

/// 最小最大を指定してその範囲で切り取るサンプラー
#[derive(Debug, Clone)]
pub struct ClampSampler<S: DurationSampler> {
    sampler: S,
    min: f64,
    max: f64,
}

impl<S: DurationSampler> DurationSampler for ClampSampler<S> {
    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> PendingDuration {
        self.sampler
            .sample(rng, current_tick)
            .apply(|v| v.clamp(self.min, self.max))
    }
}

impl<S: DurationSampler> ClampSampler<S> {
    pub fn new(sampler: S, min: f64, max: f64) -> Self {
        ClampSampler { sampler, min, max }
    }
}
