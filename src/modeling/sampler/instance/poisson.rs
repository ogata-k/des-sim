use crate::modeling::sampler::DurationSampler;
use crate::primitive::time::{Duration, SimTime};
use rand::Rng;
use rand::distr::{Distribution, StandardUniform};
use rand_distr::num_traits::{Float, FloatConst, ToPrimitive};
use rand_distr::{Exp1, Poisson, PoissonError, StandardNormal};

/// 指定されたポアソン分布からサンプリングを行う。
pub struct PoissonSampler<F>
where
    F: Float + FloatConst + ToPrimitive,
    StandardUniform: Distribution<F>,
    StandardNormal: Distribution<F>,
    Exp1: Distribution<F>,
{
    dist: Poisson<F>,
}

impl<F> DurationSampler for PoissonSampler<F>
where
    F: Float + FloatConst + ToPrimitive,
    StandardUniform: Distribution<F>,
    StandardNormal: Distribution<F>,
    Exp1: Distribution<F>,
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
                "PoissonSampler sampling failed after 255 attempts, returning Duration::one()"
            );
            Duration::one()
        })
    }
}

impl<F> PoissonSampler<F>
where
    F: Float + FloatConst + ToPrimitive,
    StandardUniform: Distribution<F>,
    StandardNormal: Distribution<F>,
    Exp1: Distribution<F>,
{
    pub fn new(lambda: F) -> Result<Self, PoissonError> {
        Poisson::new(lambda).map(|dist| PoissonSampler { dist })
    }
}
