use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;

/// ベースとなったサンプラーで得られた値に対して、揺らぎ用のサンプラーで得られた値だけ増加させて遅らせる。
/// ただし、揺らぎ用のサンプラーが負の値を返しても加算される。
/// 加算してほしくないときは[WithDelaySampler]を使うこと。
pub struct WithJitterSampler<S, J>
where
    S: DurationSampler,
    J: DurationSampler,
{
    sampler: S,
    jitter_sampler: J,
}

impl<S, J> DurationSampler for WithJitterSampler<S, J>
where
    S: DurationSampler,
    J: DurationSampler,
{
    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> PendingDuration {
        let sampled = self.sampler.sample(rng, current_tick);
        let jitter = self.jitter_sampler.sample(rng, current_tick);

        PendingDuration::new(sampled.raw_value() + jitter.raw_value())
    }
}

impl<S, J> WithJitterSampler<S, J>
where
    S: DurationSampler,
    J: DurationSampler,
{
    pub fn new(sampler: S, jitter_sampler: J) -> Self {
        Self {
            sampler,
            jitter_sampler,
        }
    }
}
