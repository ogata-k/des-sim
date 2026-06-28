use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;

/// ベースとなったサンプラーで得られた値に対して、揺らぎ用のサンプラーで得られた値だけ増加させて遅らせる。
/// ただし、揺らぎ用のサンプラーが負の値を返しても加算される。
/// 加算してほしくないときは[WithDelaySampler](crate::modeling::sampler::DelaySampler)を使うこと。
pub struct JitterSampler<S>
where
    S: DurationSampler,
{
    sampler: S,
    jitter_sampler: Box<dyn DurationSampler>,
}

impl<S> DurationSampler for JitterSampler<S>
where
    S: DurationSampler,
{
    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> PendingDuration {
        let sampled = self.sampler.sample(rng, current_tick);
        let jitter = self.jitter_sampler.sample(rng, current_tick);

        PendingDuration::new(sampled.raw_value() + jitter.raw_value())
    }
}

impl<S> JitterSampler<S>
where
    S: DurationSampler,
{
    pub fn new(sampler: S, jitter_sampler: Box<dyn DurationSampler>) -> Self {
        Self {
            sampler,
            jitter_sampler,
        }
    }
}
