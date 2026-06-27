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
