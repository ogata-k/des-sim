//! The `jitter` module provides the `JitterSampler`, a combinator that adds
//! a random or pseudo-random offset (jitter) to the output of a base
//! `DurationSampler`.
//!
//! Unlike `DelaySampler`, `JitterSampler` allows for both positive and negative
//! offsets, enabling the sampled duration to be either increased or decreased.

use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;

/// A sampler that applies "jitter" (noise) to the output of a base sampler.
///
/// The jitter value is added directly to the base sampler's output. Negative jitter
/// values will decrease the total duration (unlike [DelaySampler](crate::modeling::sampler::DelaySampler)).
pub struct JitterSampler<S>
where
    S: DurationSampler,
{
    sampler: S,
    jitter_sampler: Box<dyn DurationSampler>,
}

impl<S> DurationSampler for JitterSampler<S>
where
    S: DurationSampler,
{
    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> PendingDuration {
        let sampled = self.sampler.sample(rng, current_tick);
        let jitter = self.jitter_sampler.sample(rng, current_tick);

        PendingDuration::new(sampled.raw_value() + jitter.raw_value())
    }
}

impl<S> JitterSampler<S>
where
    S: DurationSampler,
{
    /// Creates a new `JitterSampler`.
    pub fn new(sampler: S, jitter_sampler: Box<dyn DurationSampler>) -> Self {
        Self {
            sampler,
            jitter_sampler,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modeling::sampler::instance::ConstantSampler;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    #[test]
    fn test_jitter_sampler_positive_jitter() {
        let mut rng = SmallRng::seed_from_u64(2);
        let base_sampler = ConstantSampler::new(10.0);
        let jitter_sampler = Box::new(ConstantSampler::new(5.0));
        let mut sampler = JitterSampler::new(base_sampler, jitter_sampler);

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 15.0); // 10.0 + 5.0
    }

    #[test]
    fn test_jitter_sampler_zero_jitter() {
        let mut rng = SmallRng::seed_from_u64(2);
        let base_sampler = ConstantSampler::new(10.0);
        let jitter_sampler = Box::new(ConstantSampler::new(0.0));
        let mut sampler = JitterSampler::new(base_sampler, jitter_sampler);

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 10.0); // 10.0 + 0.0
    }

    #[test]
    fn test_jitter_sampler_negative_jitter() {
        let mut rng = SmallRng::seed_from_u64(2);
        let base_sampler = ConstantSampler::new(10.0);
        let jitter_sampler = Box::new(ConstantSampler::new(-5.0));
        let mut sampler = JitterSampler::new(base_sampler, jitter_sampler);

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 5.0); // 10.0 + (-5.0) = 5.0
    }
}
