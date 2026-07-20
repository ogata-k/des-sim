//! The `exponential` module provides the `ExponentialSampler`, a `DurationSampler`
//! that generates durations following an exponential distribution.
//!
//! This is commonly used to model the time between events in a Poisson process,
//! such as arrival times in queuing systems, where events occur continuously
//! and independently at a constant average rate.

use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;
use rand::distr::Distribution;
use rand_distr::{Exp, ExpError};

/// A sampler that draws from an exponential distribution with a specified lambda (rate).
#[derive(Debug, Clone)]
pub struct ExponentialSampler {
    dist: Exp<f64>,
}

impl DurationSampler for ExponentialSampler {
    fn sample(&mut self, rng: &mut dyn Rng, _current_tick: SimTime) -> PendingDuration {
        PendingDuration::new(self.dist.sample(rng))
    }
}

impl ExponentialSampler {
    /// Creates a new `ExponentialSampler` with the given lambda (rate parameter).
    /// Returns an error if the lambda is invalid (e.g., negative or NaN).
    pub fn new(lambda: f64) -> Result<Self, ExpError> {
        Exp::new(lambda).map(|dist| ExponentialSampler { dist })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    #[test]
    fn test_new_valid_lambda() {
        let lambda = 0.5;
        let sampler = ExponentialSampler::new(lambda);
        assert!(sampler.is_ok());
    }

    #[test]
    fn test_new_invalid_lambda() {
        let lambda = -0.5;
        let sampler = ExponentialSampler::new(lambda);
        assert_eq!(sampler.err(), Some(ExpError::LambdaTooSmall));

        let lambda = f64::NAN;
        let sampler = ExponentialSampler::new(lambda);
        assert_eq!(sampler.err(), Some(ExpError::LambdaTooSmall));
    }

    #[test]
    fn test_sample_produces_positive_duration() {
        let lambda = 1.0;
        let mut sampler = ExponentialSampler::new(lambda).unwrap();
        let mut rng = SmallRng::seed_from_u64(2);
        let current_tick = SimTime::from_ticks(0);

        for _ in 0..1000 {
            let duration = sampler.sample(&mut rng, current_tick);
            assert!(duration.raw_value() > 0.0);
        }
    }

    #[test]
    fn test_sample_with_different_lambda() {
        let lambda_high = 10.0; // Mean = 1/lambda = 0.1
        let mut sampler_high = ExponentialSampler::new(lambda_high).unwrap();
        let mut rng_high = SmallRng::seed_from_u64(2);
        let current_tick = SimTime::from_ticks(0);

        let mut sum_high = 0.0;
        for _ in 0..1000 {
            sum_high += sampler_high.sample(&mut rng_high, current_tick).raw_value();
        }
        let avg_high = sum_high / 1000.0;
        // For Exp(lambda), mean is 1/lambda. So for lambda=10, mean should be 0.1
        assert!(avg_high > 0.05 && avg_high < 0.2); // Check if it's in a reasonable range

        let lambda_low = 0.1; // Mean = 1/lambda = 10.0
        let mut sampler_low = ExponentialSampler::new(lambda_low).unwrap();
        let mut rng_low = SmallRng::seed_from_u64(2);

        let mut sum_low = 0.0;
        for _ in 0..1000 {
            sum_low += sampler_low.sample(&mut rng_low, current_tick).raw_value();
        }
        let avg_low = sum_low / 1000.0;
        // For Exp(lambda), mean is 1/lambda. So for lambda=0.1, mean should be 10.0
        assert!(avg_low > 5.0 && avg_low < 15.0); // Check if it's in a reasonable range

        // Ensure that higher lambda generally leads to smaller samples
        assert!(avg_high < avg_low);
    }
}
