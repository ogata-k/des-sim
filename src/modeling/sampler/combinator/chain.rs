//! The `chain` module provides the `ChainSampler`, a combinator that links two
//! `DurationSampler` instances.
//!
//! It takes the output of both samplers and combines them using a user-defined
//! closure, allowing for sequential or interdependent sampling logic.

use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;

/// A sampler that chains two sub-samplers together, combining their results
/// using a closure.
pub struct ChainSampler<S1, F>
where
    S1: DurationSampler,
    F: FnMut(&mut dyn Rng, SimTime, f64, f64) -> f64,
{
    sampler_1: S1,
    sampler_2: Box<dyn DurationSampler>,
    f: F,
}

impl<S1, F> DurationSampler for ChainSampler<S1, F>
where
    S1: DurationSampler,
    F: FnMut(&mut dyn Rng, SimTime, f64, f64) -> f64,
{
    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> PendingDuration {
        let sampled_1 = self.sampler_1.sample(rng, current_tick);
        let sampled_2 = self.sampler_2.sample(rng, current_tick);

        // Pass the raw f64 values to the closure
        PendingDuration::new((self.f)(
            rng,
            current_tick,
            sampled_1.raw_value(),
            sampled_2.raw_value(),
        ))
    }
}

impl<S1, F> ChainSampler<S1, F>
where
    S1: DurationSampler,
    F: FnMut(&mut dyn Rng, SimTime, f64, f64) -> f64,
{
    /// Creates a new `ChainSampler`.
    pub fn new(sampler_1: S1, sampler_2: Box<dyn DurationSampler>, f: F) -> Self {
        ChainSampler {
            sampler_1,
            sampler_2,
            f,
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
    fn test_chain_sampler_add() {
        let mut rng = SmallRng::seed_from_u64(2);
        let s1 = ConstantSampler::new(10.0);
        let s2 = ConstantSampler::new(20.0);

        let mut sampler = ChainSampler::new(s1, s2.boxed(), |_, _, val1, val2| val1 + val2);

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 30.0);
    }

    #[test]
    fn test_chain_sampler_multiply() {
        let mut rng = SmallRng::seed_from_u64(2);
        let s1 = ConstantSampler::new(5.0);
        let s2 = ConstantSampler::new(4.0);

        let mut sampler = ChainSampler::new(s1, s2.boxed(), |_, _, val1, val2| val1 * val2);

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 20.0);
    }
}
