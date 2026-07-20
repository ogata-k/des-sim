//! The `map` module provides the `MapSampler`, a combinator that transforms
//! the output of a base `DurationSampler` using a user-defined closure.
//!
//! This allows for arbitrary mathematical or logical operations to be applied
//! to the sampled duration, enabling flexible customization of sampling behavior.

use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;

/// A sampler that maps the output of a base sampler to a new value using a closure.
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
    /// Creates a new `MapSampler`.
    pub fn new(sampler: S, f: F) -> Self {
        MapSampler { sampler, f }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modeling::sampler::instance::ConstantSampler;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    #[test]
    fn test_map_sampler_double_value() {
        let mut rng = SmallRng::seed_from_u64(2);
        let base_sampler = ConstantSampler::new(10.0);
        let mut sampler = MapSampler::new(base_sampler, |_, _, value| value * 2.0);

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 20.0);
    }

    #[test]
    fn test_map_sampler_add_offset() {
        let mut rng = SmallRng::seed_from_u64(2);
        let base_sampler = ConstantSampler::new(10.0);
        let mut sampler = MapSampler::new(base_sampler, |_, _, value| value + 5.0);

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 15.0);
    }

    #[test]
    fn test_map_sampler_negate_value() {
        let mut rng = SmallRng::seed_from_u64(2);
        let base_sampler = ConstantSampler::new(10.0);
        let mut sampler = MapSampler::new(base_sampler, |_, _, value| -value);

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), -10.0);
    }
}
