use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;
use rand::distr::Distribution;
use rand_distr::{Poisson, PoissonError};

/// 指定されたポアソン分布からサンプリングを行う。
#[derive(Debug, Clone)]
pub struct PoissonSampler {
    dist: Poisson<f64>,
}

impl DurationSampler for PoissonSampler {
    fn sample(&mut self, rng: &mut dyn Rng, _current_tick: SimTime) -> PendingDuration {
        PendingDuration::new(self.dist.sample(rng))
    }
}

impl PoissonSampler {
    pub fn new(lambda: f64) -> Result<Self, PoissonError> {
        Poisson::new(lambda).map(|dist| PoissonSampler { dist })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    #[test]
    fn test_poisson_sampler() {
        let mut rng = SmallRng::seed_from_u64(2);
        let lambda = 5.0; // Expected mean and variance
        let mut sampler = PoissonSampler::new(lambda).unwrap();

        let mut samples = Vec::new();
        for _ in 0..10000 {
            samples.push(sampler.sample(&mut rng, SimTime::new(0)).raw_value());
        }

        let sample_mean: f64 = samples.iter().sum::<f64>() / samples.len() as f64;
        let variance: f64 = samples
            .iter()
            .map(|x| (x - sample_mean).powi(2))
            .sum::<f64>()
            / samples.len() as f64;

        // Check if the sampled mean and variance are close to the expected values (lambda)
        assert!(
            (sample_mean - lambda).abs() < 0.1,
            "Mean: {}, Expected: {}",
            sample_mean,
            lambda
        );
        assert!(
            (variance - lambda).abs() < 0.1,
            "Variance: {}, Expected: {}",
            variance,
            lambda
        );

        // Check if all samples are non-negative integers (as f64)
        assert!(samples.iter().all(|&x| x >= 0.0 && x.fract() == 0.0));
    }
    #[test]
    fn test_poisson_sampler_invalid_zero_lambda() {
        let sampler = PoissonSampler::new(0.0);
        assert_eq!(sampler.err(), Some(PoissonError::ShapeTooSmall));
        let sampler = PoissonSampler::new(-0.01);
        assert_eq!(sampler.err(), Some(PoissonError::ShapeTooSmall));
    }

    #[test]
    fn test_poisson_sampler_invalid_infinity_lambda() {
        let sampler = PoissonSampler::new(f64::INFINITY);
        assert_eq!(sampler.err(), Some(PoissonError::NonFinite));
        let sampler = PoissonSampler::new(f64::NAN);
        assert_eq!(sampler.err(), Some(PoissonError::NonFinite));
    }

    #[test]
    fn test_poisson_sampler_invalid_lambda() {
        let sampler = PoissonSampler::new(Poisson::<f64>::MAX_LAMBDA * 2.0);
        assert_eq!(sampler.err(), Some(PoissonError::ShapeTooLarge));
    }
}
