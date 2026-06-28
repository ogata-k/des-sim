use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;

/// ベースとなったサンプラーで得られた値に対して、遅延用のサンプラーで得られた値だけ増加させて遅らせる。
/// ただし、遅延用のサンプラーが負の値を返したときは加算されない。
/// 加算してほしいときは[WithJitterSampler](crate::modeling::sampler::JitterSampler)を使うこと。
pub struct DelaySampler<S>
where
    S: DurationSampler,
{
    sampler: S,
    delay_sampler: Box<dyn DurationSampler>,
}

impl<S> DurationSampler for DelaySampler<S>
where
    S: DurationSampler,
{
    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> PendingDuration {
        let sampled = self.sampler.sample(rng, current_tick);
        let delay = self.delay_sampler.sample(rng, current_tick);

        // 遅延が負になっているということをできるだけ早く発火させろということで解釈する。
        PendingDuration::new(sampled.raw_value() + delay.raw_value().max(0.0))
    }
}

impl<S> DelaySampler<S>
where
    S: DurationSampler,
{
    pub fn new(sampler: S, delay_sampler: Box<dyn DurationSampler>) -> Self {
        DelaySampler {
            sampler,
            delay_sampler,
        }
    }
}
