use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::{Duration, SimTime};
use rand::Rng;

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

            if raw_value < 0.0 {
                continue;
            }

            return PendingDuration::new(raw_value);
        }

        PendingDuration::from_duration((self.fallback)(rng, current_tick))
    }
}

impl<S, F> EnsureNonNegativeSampler<S, F>
where
    S: DurationSampler,
    F: FnMut(&mut dyn Rng, SimTime) -> Duration,
{
    pub fn new(sampler: S, limit_try_count: u8, fallback: F) -> Self {
        Self {
            sampler,
            limit_try_count,
            fallback,
        }
    }
}
