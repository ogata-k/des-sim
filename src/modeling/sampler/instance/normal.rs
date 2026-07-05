use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;
use rand::distr::Distribution;
use rand_distr::{Normal, NormalError};

/// 指定された平均値と標準偏差をもとに正規分布からサンプリングを行う。
/// ただし、正の値がでるまでサンプリングするので指定されるパラメータによっては実行時間に注意。
#[derive(Debug, Clone)]
pub struct NormalSampler {
    dist: Normal<f64>,
}

impl DurationSampler for NormalSampler {
    fn sample(&mut self, rng: &mut dyn Rng, _current_tick: SimTime) -> PendingDuration {
        PendingDuration::new(self.dist.sample(rng))
    }
}

impl NormalSampler {
    pub fn new(mean: f64, std_dev: f64) -> Result<Self, NormalError> {
        Normal::new(mean, std_dev).map(|dist| NormalSampler { dist })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    #[test]
    fn test_normal_sampler() {
        let mut rng = SmallRng::seed_from_u64(2);
        let mean = 10.0;
        let std_dev = 2.0;
        let mut sampler = NormalSampler::new(mean, std_dev).unwrap();

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
        let sample_std_dev = variance.sqrt();

        // Check if the sampled mean and std_dev are close to the expected values
        assert!(
            (sample_mean - mean).abs() < 0.1,
            "Mean: {}, Expected: {}",
            sample_mean,
            mean
        );
        assert!(
            (sample_std_dev - std_dev).abs() < 0.1,
            "Std Dev: {}, Expected: {}",
            sample_std_dev,
            std_dev
        );
    }

    #[test]
    fn test_normal_sampler_invalid_std_dev() {
        let sampler = NormalSampler::new(0.0, f64::INFINITY);
        assert_eq!(sampler.err(), Some(NormalError::BadVariance));
    }
}
