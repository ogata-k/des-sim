use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;

/// 定数値をサンプリングするサンプラー。
#[derive(Debug, Clone)]
pub struct ConstantSampler {
    value: f64,
}

impl DurationSampler for ConstantSampler {
    fn sample(&mut self, _rng: &mut dyn Rng, _current_tick: SimTime) -> PendingDuration {
        // 乱数生成器を使わずに常に固定値を返す
        PendingDuration::new(self.value)
    }
}

impl ConstantSampler {
    pub fn new(value: f64) -> Self {
        Self { value }
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    /// [Rng]や[SimTime]を用意するのは面倒だけど[PendingDuration]は欲しいというときは[sample()](DurationSampler::sample)の代わりにこちらを利用する。
    pub fn constant_sample(&self) -> PendingDuration {
        PendingDuration::new(self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    #[test]
    fn test_constant_sampler() {
        let mut rng = SmallRng::seed_from_u64(2);
        let mut sampler = ConstantSampler::new(100.0);

        let sample = sampler.sample(&mut rng, SimTime::new(0));
        assert_eq!(sample.raw_value(), 100.0);
    }
}
