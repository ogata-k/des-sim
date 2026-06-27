use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;
use rand::distr::Distribution;
use rand_distr::{Exp, ExpError};

/// 指定された指数分布からサンプリングを行う。
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
    pub fn new(lambda: f64) -> Result<Self, ExpError> {
        Exp::new(lambda).map(|dist| ExponentialSampler { dist })
    }
}
