use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::{Duration, SimTime};
use rand::Rng;

/// A sampler that ensures the sampled duration is non-negative.
///
/// It attempts to sample from the base `sampler` up to `limit_try_count` times.
/// If all attempts return a negative value, it falls back to the result of the
/// `fallback` closure.
#[derive(Debug, Clone)]
pub struct EnsureNonNegativeSampler<S, F>
where
    S: DurationSampler,
    F: FnMut(&mut dyn Rng, SimTime) -> Duration,
{
    sampler: S,
    limit_try_count: u8,
    fallback: F,
}

impl<S, F> DurationSampler for EnsureNonNegativeSampler<S, F>
where
    S: DurationSampler,
    F: FnMut(&mut dyn Rng, SimTime) -> Duration,
{
    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> PendingDuration {
        for _ in 0..self.limit_try_count {
            let sampled = self.sampler.sample(rng, current_tick);
            let raw_value = sampled.raw_value();

            if raw_value >= 0.0 {
                return PendingDuration::new(raw_value);
            }
        }

        PendingDuration::from_duration((self.fallback)(rng, current_tick))
    }
}

impl<S, F> EnsureNonNegativeSampler<S, F>
where
    S: DurationSampler,
    F: FnMut(&mut dyn Rng, SimTime) -> Duration,
{
    /// Creates a new `EnsureNonNegativeSampler`.
    pub fn new(sampler: S, limit_try_count: u8, fallback: F) -> Self {
        Self {
            sampler,
            limit_try_count,
            fallback,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modeling::sampler::instance::ConstantSampler;
    use crate::primitive::time::Duration;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    #[test]
    fn test_ensure_non_negative_sampler_positive_value() {
        let mut rng = SmallRng::seed_from_u64(2);
        let base_sampler = ConstantSampler::new(10.0);
        let mut sampler = EnsureNonNegativeSampler::new(base_sampler, 3, |_, _| Duration::ticks(0));

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 10.0);
    }

    #[test]
    fn test_ensure_non_negative_sampler_negative_then_fallback() {
        let mut rng = SmallRng::seed_from_u64(2);
        // This mock sampler will always return -1.0
        struct NegativeSampler;
        impl DurationSampler for NegativeSampler {
            fn sample(&mut self, _rng: &mut dyn Rng, _current_tick: SimTime) -> PendingDuration {
                PendingDuration::new(-1.0)
            }
        }

        let base_sampler = NegativeSampler;
        let mut sampler = EnsureNonNegativeSampler::new(base_sampler, 1, |_, _| Duration::ticks(5));

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 5.0); // Should fall back to 5.0
    }

    #[test]
    fn test_ensure_non_negative_sampler_multiple_tries_then_positive() {
        let mut rng = SmallRng::seed_from_u64(2);
        struct MixedSampler {
            call_count: usize,
        }
        impl DurationSampler for MixedSampler {
            fn sample(&mut self, _rng: &mut dyn Rng, _current_tick: SimTime) -> PendingDuration {
                self.call_count += 1;
                if self.call_count < 3 {
                    PendingDuration::new(-1.0)
                } else {
                    PendingDuration::new(10.0)
                }
            }
        }

        let base_sampler = MixedSampler { call_count: 0 };
        let mut sampler = EnsureNonNegativeSampler::new(base_sampler, 5, |_, _| Duration::ticks(0));

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 10.0);
        assert_eq!(sampler.sampler.call_count, 3); // Should have tried 3 times
    }
}
