use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;

#[derive(Debug, Clone)]
pub struct ChainSampler<S1, S2, F>
where
    S1: DurationSampler,
    S2: DurationSampler,
    F: FnMut(&mut dyn Rng, SimTime, f64, f64) -> f64,
{
    sampler_1: S1,
    sampler_2: S2,
    f: F,
}

impl<S1, S2, F> DurationSampler for ChainSampler<S1, S2, F>
where
    S1: DurationSampler,
    S2: DurationSampler,
    F: FnMut(&mut dyn Rng, SimTime, f64, f64) -> f64,
{
    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> PendingDuration {
        let sampled_1 = self.sampler_1.sample(rng, current_tick);
        let sampled_2 = self.sampler_2.sample(rng, current_tick);
        PendingDuration::new((self.f)(rng, current_tick, sampled_1.0, sampled_2.0))
    }
}

impl<S1, S2, F> ChainSampler<S1, S2, F>
where
    S1: DurationSampler,
    S2: DurationSampler,
    F: FnMut(&mut dyn Rng, SimTime, f64, f64) -> f64,
{
    pub fn new(sampler_1: S1, sampler_2: S2, f: F) -> Self {
        ChainSampler {
            sampler_1,
            sampler_2,
            f,
        }
    }
}
