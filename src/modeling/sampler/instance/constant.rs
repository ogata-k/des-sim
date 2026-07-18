use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;

/// A sampler that consistently returns a constant value.
#[derive(Debug, Clone)]
pub struct ConstantSampler {
    value: f64,
}

impl DurationSampler for ConstantSampler {
    fn sample(&mut self, _rng: &mut dyn Rng, _current_tick: SimTime) -> PendingDuration {
        // Returns the fixed constant value regardless of RNG or simulation time.
        PendingDuration::new(self.value)
    }
}

impl ConstantSampler {
    /// Creates a new `ConstantSampler` with the given `value`.
    pub fn new(value: f64) -> Self {
        Self { value }
    }

    /// Returns the constant value held by this sampler.
    pub fn value(&self) -> f64 {
        self.value
    }

    /// A convenience method to retrieve the `PendingDuration` without needing an RNG or `SimTime`.
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

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 100.0);
    }
}
