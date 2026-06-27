pub mod combinator;
pub mod instance;

use std::ops::{Add, Sub};

use crate::primitive::time::{Duration, SimTime, TimeTick};
use combinator::*;
use rand::Rng;
use rand_distr::num_traits::ToPrimitive;

/// 丸目誤差を吸収するために内部で浮動小数点数で持つ[Duration]のラッパー
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct PendingDuration(f64);

impl From<Duration> for PendingDuration {
    fn from(value: Duration) -> Self {
        PendingDuration::from_duration(value)
    }
}

impl Add<Duration> for PendingDuration {
    type Output = PendingDuration;

    fn add(self, rhs: Duration) -> Self::Output {
        PendingDuration::new(self.0 + rhs.as_ticks() as f64)
    }
}

impl Add<PendingDuration> for PendingDuration {
    type Output = PendingDuration;

    fn add(self, rhs: PendingDuration) -> Self::Output {
        PendingDuration::new(self.0 + rhs.0)
    }
}

impl Sub<Duration> for PendingDuration {
    type Output = PendingDuration;

    fn sub(self, rhs: Duration) -> Self::Output {
        debug_assert!(self.0 >= rhs.as_ticks() as f64);
        PendingDuration::new(self.0 - rhs.as_ticks() as f64)
    }
}

impl Sub<PendingDuration> for PendingDuration {
    type Output = PendingDuration;

    fn sub(self, rhs: PendingDuration) -> Self::Output {
        debug_assert!(self.0 >= rhs.0);
        PendingDuration::new(self.0 - rhs.0)
    }
}

impl PendingDuration {
    // コンストラクタで正の値を強制する
    pub fn new(value: f64) -> Self {
        Self(value)
    }

    pub fn from_duration(duration: Duration) -> Self {
        Self::new(duration.as_ticks() as f64)
    }

    pub fn raw_value(&self) -> f64 {
        self.0
    }

    pub fn to_duration(&self) -> Duration {
        self.try_duration().expect(&format!(
            "Duration Not Support negative value: {}",
            self.raw_value()
        ))
    }

    pub fn to_duration_with_clamp(&self, max: TimeTick) -> Duration {
        let raw_value = self.raw_value();
        if raw_value >= max as f64 {
            Duration::ticks(max)
        } else {
            Duration::ticks(
                raw_value
                    .round()
                    // これで制限しているからto_usize()は常に成功するはず
                    .clamp(0.0, max as f64)
                    .to_usize()
                    .expect(&format!("Unexpected clamp handling value: {}", raw_value)),
            )
        }
    }

    pub fn to_duration_or_else<F>(&self, f: F) -> Duration
    where
        F: FnOnce() -> Duration,
    {
        self.try_duration().unwrap_or_else(f)
    }

    pub fn try_duration(&self) -> Option<Duration> {
        // 変換時に初めて整数化・丸めを行う
        Some(Duration::ticks(self.raw_value().round().to_usize()?))
    }

    pub fn apply<F>(&mut self, f: F) -> PendingDuration
    where
        F: Fn(f64) -> f64,
    {
        PendingDuration::new(f(self.0))
    }
}

/// [Duration]を取得するためのヘルパートレイト
// なお、Box<dyn DurationSampler>とかく必要があるところがあるので、各メソッドでジェネリクスは使えない。
pub trait DurationSampler {
    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> PendingDuration;
}

pub trait CombinatorExt: DurationSampler + Sized + 'static {
    /// コンビネータを作るたびに Box::new() を書く手間を省くだけのヘルパー
    fn boxed(self) -> Box<dyn DurationSampler> {
        Box::new(self)
    }

    fn map<F>(self, f: F) -> MapSampler<Self, F>
    where
        F: Fn(f64) -> f64,
    {
        MapSampler::new(self, f)
    }

    fn with_delay<D>(self, delay: D) -> WithDelaySampler<Self, D>
    where
        D: DurationSampler,
    {
        WithDelaySampler::new(self, delay)
    }

    fn with_jitter<J>(self, jitter: J) -> WithJitterSampler<Self, J>
    where
        J: DurationSampler,
    {
        WithJitterSampler::new(self, jitter)
    }

    fn chain<S, F>(self, sampler: S, f: F) -> ChainSampler<Self, S, F>
    where
        S: DurationSampler,
        F: Fn(f64, f64) -> f64,
    {
        ChainSampler::new(self, sampler, f)
    }

    fn aggregate_with<F>(
        self,
        others: impl IntoIterator<Item = Box<dyn DurationSampler>>,
        f: F,
    ) -> AggregateSampler<F>
    where
        F: Fn(Vec<f64>) -> f64,
    {
        let mut samplers = vec![self.boxed()];
        samplers.extend(others);

        AggregateSampler::new(samplers, f)
    }

    fn aggregate_builder(self) -> AggregateBuilder {
        AggregateBuilder::from_sampler(self)
    }

    fn clamp(self, min: f64, max: f64) -> ClampSampler<Self> {
        ClampSampler::new(self, min, max)
    }

    fn non_negative<F>(self, limit_try_count: u8, fallback: F) -> NonNegativeSampler<Self, F>
    where
        F: Fn(&mut dyn Rng, SimTime) -> Duration,
    {
        NonNegativeSampler::new(self, limit_try_count, fallback)
    }
}
impl<T: DurationSampler + Sized + 'static> CombinatorExt for T {}
