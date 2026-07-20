//! The `uniform` module provides the `UniformSampler`, a `DurationSampler`
//! that generates durations following a continuous uniform distribution.
//!
//! This sampler is useful for modeling scenarios where any value within a given
//! range is equally likely, such as random delays or resource allocation times
//! with no particular bias.

use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;
use rand::distr::{Distribution, Uniform, uniform};

/// A sampler that draws from a continuous uniform distribution within [lower_bound, higher_bound).
#[derive(Debug, Clone)]
pub struct UniformSampler {
    dist: Uniform<f64>,
}

impl DurationSampler for UniformSampler {
    fn sample(&mut self, rng: &mut dyn Rng, _current_tick: SimTime) -> PendingDuration {
        PendingDuration::new(self.dist.sample(rng))
    }
}

impl UniformSampler {
    /// Creates a new `UniformSampler` with the given range.
    /// Returns an error if the range is empty or if values are non-finite.
    pub fn new(lower_bound: f64, higher_bound: f64) -> Result<UniformSampler, uniform::Error> {
        let uniform = Uniform::new(lower_bound, higher_bound)?;

        Ok(UniformSampler { dist: uniform })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    #[test]
    fn test_uniform_sampler() {
        let mut rng = SmallRng::seed_from_u64(2);
        let lower_bound = 5.0;
        let higher_bound = 15.0;
        let mut sampler = UniformSampler::new(lower_bound, higher_bound).unwrap();

        let mut samples = Vec::new();
        for _ in 0..10000 {
            samples.push(sampler.sample(&mut rng, SimTime::from_ticks(0)).raw_value());
        }

        // Check if all samples are within the specified range
        assert!(
            samples
                .iter()
                .all(|&x| x >= lower_bound && x < higher_bound)
        );

        let sample_mean: f64 = samples.iter().sum::<f64>() / samples.len() as f64;
        let expected_mean = (lower_bound + higher_bound) / 2.0;

        // Check if the sampled mean is close to the expected mean
        assert!(
            (sample_mean - expected_mean).abs() < 0.1,
            "Mean: {}, Expected: {}",
            sample_mean,
            expected_mean
        );
    }

    #[test]
    fn test_uniform_sampler_invalid_bounds() {
        // Lower bound must be less than higher bound
        let sampler_empty_range = UniformSampler::new(10.0, 5.0);
        assert_eq!(sampler_empty_range.err(), Some(uniform::Error::EmptyRange));
    }

    #[test]
    fn test_uniform_sampler_invalid_infinite_bounds() {
        // Bounds must be finite
        let sampler_infinite_range = UniformSampler::new(10.0, f64::INFINITY);
        assert_eq!(
            sampler_infinite_range.err(),
            Some(uniform::Error::NonFinite)
        );
    }
}
