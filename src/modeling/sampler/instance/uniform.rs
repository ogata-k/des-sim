use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::{Duration, SimTime};
use rand::Rng;
use rand::distr::uniform::{SampleBorrow, UniformUsize};
use rand::distr::{Distribution, Uniform, uniform};

/// 指定された最大値と最小値をもとに一様分布からサンプリングを行う。
#[derive(Debug, Clone)]
pub struct UniformSampler {
    dist: Uniform<usize>,
}

impl DurationSampler for UniformSampler {
    fn sample(&mut self, rng: &mut dyn Rng, _current_tick: SimTime) -> PendingDuration {
        let v = self.dist.sample(rng);
        Duration::ticks(v).into()
    }
}

impl UniformSampler {
    pub fn new<B1, B2>(low_b: B1, high_b: B2) -> Result<UniformSampler, uniform::Error>
    where
        B1: SampleBorrow<<UniformUsize as uniform::UniformSampler>::X>,
        B2: SampleBorrow<<UniformUsize as uniform::UniformSampler>::X>,
    {
        let uniform = Uniform::new(low_b, high_b)?;

        Ok(UniformSampler { dist: uniform })
    }
}
