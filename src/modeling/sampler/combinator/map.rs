use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;

#[derive(Debug, Clone)]
pub struct MapSampler<S, F>
where
    S: DurationSampler,
    F: FnMut(&mut dyn Rng, SimTime, f64) -> f64,
{
    sampler: S,
    f: F,
}

impl<S, F> DurationSampler for MapSampler<S, F>
where
    S: DurationSampler,
    F: FnMut(&mut dyn Rng, SimTime, f64) -> f64,
{
    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> PendingDuration {
        let sampled = self.sampler.sample(rng, current_tick);

        PendingDuration::new((self.f)(rng, current_tick, sampled.raw_value()))
    }
}

impl<S, F> MapSampler<S, F>
where
    S: DurationSampler,
    F: FnMut(&mut dyn Rng, SimTime, f64) -> f64,
{
    pub fn new(sampler: S, f: F) -> Self {
        MapSampler { sampler, f }
    }
}
