use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;

/// A sampler that delays the output of a base sampler by adding the value from a delay sampler.
///
/// If the delay sampler returns a negative value, it is treated as zero, meaning the delay
/// is not applied. If you need to support negative values (e.g., to accelerate the schedule),
/// use [JitterSampler](crate::modeling::sampler::JitterSampler) instead.
pub struct DelaySampler<S>
where
    S: DurationSampler,
{
    sampler: S,
    delay_sampler: Box<dyn DurationSampler>,
}

impl<S> DurationSampler for DelaySampler<S>
where
    S: DurationSampler,
{
    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> PendingDuration {
        let sampled = self.sampler.sample(rng, current_tick);
        let delay = self.delay_sampler.sample(rng, current_tick);

        // If the delay is negative, it is interpreted as no delay (max(0.0)).
        PendingDuration::new(sampled.raw_value() + delay.raw_value().max(0.0))
    }
}

impl<S> DelaySampler<S>
where
    S: DurationSampler,
{
    /// Creates a new `DelaySampler`.
    pub fn new(sampler: S, delay_sampler: Box<dyn DurationSampler>) -> Self {
        DelaySampler {
            sampler,
            delay_sampler,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modeling::sampler::CombinatorExt;
    use crate::modeling::sampler::instance::ConstantSampler;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    #[test]
    fn test_delay_sampler_positive_delay() {
        let mut rng = SmallRng::seed_from_u64(2);
        let base_sampler = ConstantSampler::new(10.0);
        let delay_sampler = ConstantSampler::new(5.0);
        let mut sampler = DelaySampler::new(base_sampler, delay_sampler.boxed());

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 15.0); // 10.0 + 5.0
    }

    #[test]
    fn test_delay_sampler_zero_delay() {
        let mut rng = SmallRng::seed_from_u64(2);
        let base_sampler = ConstantSampler::new(10.0);
        let delay_sampler = ConstantSampler::new(0.0);
        let mut sampler = DelaySampler::new(base_sampler, delay_sampler.boxed());

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 10.0); // 10.0 + 0.0
    }

    #[test]
    fn test_delay_sampler_negative_delay() {
        let mut rng = SmallRng::seed_from_u64(2);
        let base_sampler = ConstantSampler::new(10.0);
        let delay_sampler = ConstantSampler::new(-5.0);
        let mut sampler = DelaySampler::new(base_sampler, delay_sampler.boxed());

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 10.0); // 10.0 + max(-5.0, 0.0) = 10.0
    }
}
