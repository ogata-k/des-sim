use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;

/// ベースとなったサンプラーで得られた値に対して、遅延用のサンプラーで得られた値だけ増加させて遅らせる。
/// ただし、遅延用のサンプラーが負の値を返したときは加算されない。
/// 加算してほしいときは[WithJitterSampler]を使うこと。
#[derive(Debug)]
pub struct WithDelaySampler<S, D>
where
    S: DurationSampler,
    D: DurationSampler,
{
    sampler: S,
    delay_sampler: D,
}

impl<S, D> DurationSampler for WithDelaySampler<S, D>
where
    S: DurationSampler,
    D: DurationSampler,
{
    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> PendingDuration {
        let sampled = self.sampler.sample(rng, current_tick);
        let delay = self.delay_sampler.sample(rng, current_tick);

        // 遅延が負になっているということをできるだけ早く発火させろということで解釈する。
        PendingDuration::new(sampled.raw_value() + delay.raw_value().max(0.0))
    }
}

impl<S, D> WithDelaySampler<S, D>
where
    S: DurationSampler,
    D: DurationSampler,
{
    pub fn new(sampler: S, delay_sampler: D) -> Self {
        WithDelaySampler {
            sampler,
            delay_sampler,
        }
    }
}
