use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;
use rand::distr::{Distribution, Uniform, uniform};

/// 指定された最大値と最小値をもとに一様分布からサンプリングを行う。
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
    pub fn new(lower_bound: f64, higher_bound: f64) -> Result<UniformSampler, uniform::Error> {
        let uniform = Uniform::new(lower_bound, higher_bound)?;

        Ok(UniformSampler { dist: uniform })
    }
}
