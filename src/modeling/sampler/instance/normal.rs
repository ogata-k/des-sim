use crate::modeling::sampler::DurationSampler;
use crate::primitive::time::{Duration, SimTime};
use rand::Rng;
use rand::distr::Distribution;
use rand_distr::num_traits::{Float, ToPrimitive};
use rand_distr::{Normal, NormalError, StandardNormal};

/// 指定された平均値と標準偏差をもとに正規分布からサンプリングを行う。
/// ただし、正の値がでるまでサンプリングするので指定されるパラメータによっては実行時間に注意。
pub struct NormalSampler<F>
where
    F: Float + ToPrimitive,
    StandardNormal: Distribution<F>,
{
    dist: Normal<F>,
}

impl<F> DurationSampler for NormalSampler<F>
where
    F: Float + ToPrimitive,
    StandardNormal: Distribution<F>,
{
    fn try_sample(
        &mut self,
        rng: &mut dyn Rng,
        _current_tick: SimTime,
        try_count: u8,
    ) -> Option<Duration> {
        for _ in 0..try_count {
            let v = self.dist.sample(rng);
            if v.is_sign_negative() || v.is_infinite() {
                continue;
            }

            // 単にto_usize()すると小数点以下が単純に切り捨てられるのでいったん四捨五入してからusizeにする。
            let u = v.round().to_usize();
            match u {
                None => {
                    continue;
                }
                Some(u) => {
                    return Some(Duration::ticks(u));
                }
            }
        }

        None
    }

    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> Duration {
        // デフォルトの成功率が低いと終わらないので限界まで試してダメなら次の時刻を返す
        self.try_sample(rng, current_tick, 255).unwrap_or_else(|| {
            log::warn!(
                "NormalSampler sampling failed after 255 attempts, returning Duration::one()"
            );
            Duration::one()
        })
    }
}

impl<F> NormalSampler<F>
where
    F: Float + ToPrimitive,
    StandardNormal: Distribution<F>,
{
    pub fn new(mean: F, std_dev: F) -> Result<Self, NormalError> {
        Normal::new(mean, std_dev).map(|dist| NormalSampler { dist })
    }
}
