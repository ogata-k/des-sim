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
