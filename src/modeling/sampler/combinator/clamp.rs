use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;

/// A sampler that clamps the sampled value within the specified [min, max] range.
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
    /// Creates a new `ClampSampler` that constrains the output of `sampler`
    /// between `min` and `max`.
    pub fn new(sampler: S, min: f64, max: f64) -> Self {
        ClampSampler { sampler, min, max }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modeling::sampler::instance::ConstantSampler;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    #[test]
    fn test_clamp_sampler_within_bounds() {
        let mut rng = SmallRng::seed_from_u64(2);
        let s = ConstantSampler::new(50.0);
        let mut sampler = ClampSampler::new(s, 10.0, 100.0);

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 50.0);
    }

    #[test]
    fn test_clamp_sampler_below_min() {
        let mut rng = SmallRng::seed_from_u64(2);
        let s = ConstantSampler::new(5.0);
        let mut sampler = ClampSampler::new(s, 10.0, 100.0);

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 10.0);
    }

    #[test]
    fn test_clamp_sampler_above_max() {
        let mut rng = SmallRng::seed_from_u64(2);
        let s = ConstantSampler::new(150.0);
        let mut sampler = ClampSampler::new(s, 10.0, 100.0);

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 100.0);
    }
}
