pub mod combinator;
pub mod instance;

use std::ops::{Add, Sub};

use crate::primitive::time::{Duration, SimTime, TimeTick};
use combinator::*;
use rand::Rng;
use rand_distr::num_traits::ToPrimitive;

/// A wrapper for floating-point representations of [Duration] to mitigate rounding errors.
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
        PendingDuration::new(self.0 + rhs.as_time_tick() as f64)
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
        PendingDuration::new(self.0 - rhs.as_time_tick() as f64)
    }
}

impl Sub<PendingDuration> for PendingDuration {
    type Output = PendingDuration;

    fn sub(self, rhs: PendingDuration) -> Self::Output {
        PendingDuration::new(self.0 - rhs.0)
    }
}

impl PendingDuration {
    /// Creates a new `PendingDuration` from a raw `f64` value.
    pub fn new(value: f64) -> Self {
        Self(value)
    }

    /// Creates a new `PendingDuration` from a [Duration].
    pub fn from_duration(duration: Duration) -> Self {
        Self::new(duration.as_time_tick() as f64)
    }

    /// Returns the raw floating-point value.
    pub fn raw_value(&self) -> f64 {
        self.0
    }

    /// Attempts to convert this instance into a [Duration].
    /// Returns `None` if the value is negative.
    pub fn try_to_duration(&self) -> Option<Duration> {
        // Rounding and integer conversion occur here.
        Some(Duration::ticks(self.raw_value().round().to_usize()?))
    }

    /// Converts this instance into a [Duration].
    ///
    /// # Panics
    ///
    /// Panics if the value is negative.
    pub fn to_duration(&self) -> Duration {
        self.try_to_duration().unwrap_or_else(|| {
            panic!(
                "Duration does not support negative values: {}",
                self.raw_value()
            )
        })
    }

    /// Converts to a [Duration], clamping the result between 0 and `max` ticks.
    pub fn to_duration_with_clamp(&self, max: TimeTick) -> Duration {
        let raw_value = self.raw_value();
        if raw_value >= max as f64 {
            Duration::ticks(max)
        } else {
            Duration::ticks(
                raw_value
                    .round()
                    .clamp(0.0, max as f64)
                    .to_usize()
                    .unwrap_or_else(|| {
                        panic!("Unexpected clamping error for value: {}", raw_value)
                    }),
            )
        }
    }

    /// Converts to a [Duration], invoking the provided closure if the conversion fails.
    pub fn to_duration_or_else<F>(&self, f: F) -> Duration
    where
        F: FnOnce() -> Duration,
    {
        self.try_to_duration().unwrap_or_else(f)
    }

    /// Applies a function to the internal value and returns a new `PendingDuration`.
    pub fn apply<F>(&mut self, f: F) -> PendingDuration
    where
        F: Fn(f64) -> f64,
    {
        PendingDuration::new(f(self.0))
    }
}

/// A trait for generating [Duration] samples.
pub trait DurationSampler {
    /// Generates a sample given a random number generator and the current simulation time.
    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> PendingDuration;
}

/// A trait for cloning trait objects of [DurationSampler].
pub trait ClonableDurationSampler: DurationSampler + Send + Sync {
    fn box_clone(&self) -> Box<dyn ClonableDurationSampler>;
}

// Automatically implement `ClonableDurationSampler` for all compatible types.
impl<S> ClonableDurationSampler for S
where
    S: DurationSampler + Clone + Send + Sync + 'static,
{
    fn box_clone(&self) -> Box<dyn ClonableDurationSampler> {
        Box::new(self.clone())
    }
}

// Enable cloning for boxed trait objects.
impl Clone for Box<dyn ClonableDurationSampler> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

/// An extension trait for [DurationSampler] that provides fluent combinator methods
/// for creating and composing complex duration sampling logic.
pub trait CombinatorExt: DurationSampler + Sized + 'static {
    /// Boxes the sampler into a `Box<dyn DurationSampler>` to erase its concrete type.
    fn boxed(self) -> Box<dyn DurationSampler> {
        Box::new(self)
    }

    /// Boxes the sampler as a cloneable trait object.
    /// Requires the underlying sampler to be `Clone`, `Send`, and `Sync`.
    fn boxed_clonable(self) -> Box<dyn ClonableDurationSampler>
    where
        Self: Clone + Send + Sync,
    {
        Box::new(self)
    }

    /// Transforms the output of this sampler using the provided closure `f`.
    fn map<F>(self, f: F) -> MapSampler<Self, F>
    where
        F: FnMut(&mut dyn Rng, SimTime, f64) -> f64,
    {
        MapSampler::new(self, f)
    }

    /// Adds a delay to the result of this sampler using another sampler for the duration.
    fn delay(self, delay: Box<dyn DurationSampler>) -> DelaySampler<Self> {
        DelaySampler::new(self, delay)
    }

    /// Adds a jitter (noise) to the result of this sampler using another sampler for the variation.
    fn jitter(self, jitter: Box<dyn DurationSampler>) -> JitterSampler<Self> {
        JitterSampler::new(self, jitter)
    }

    /// Chains this sampler with another, combining their outputs using the provided closure `f`.
    fn chain<F>(self, sampler: Box<dyn DurationSampler>, f: F) -> ChainSampler<Self, F>
    where
        F: FnMut(&mut dyn Rng, SimTime, f64, f64) -> f64,
    {
        ChainSampler::new(self, sampler, f)
    }

    /// Aggregates the results of this sampler and a collection of others into a single value
    /// using the provided closure `f`.
    fn aggregate<F>(
        self,
        others: impl IntoIterator<Item = Box<dyn DurationSampler>>,
        f: F,
    ) -> AggregateSampler<F>
    where
        F: FnMut(&mut dyn Rng, SimTime, Vec<f64>) -> f64,
    {
        let mut samplers = vec![self.boxed()];
        samplers.extend(others);

        AggregateSampler::new(samplers, f)
    }

    /// Returns an `AggregateBuilder` initialized with this sampler, allowing for
    /// a more flexible construction of aggregate samplers.
    fn aggregate_builder(self) -> AggregateBuilder {
        AggregateBuilder::from_sampler(self)
    }

    /// Clamps the output of this sampler within the range `[min, max]`.
    fn clamp(self, min: f64, max: f64) -> ClampSampler<Self> {
        ClampSampler::new(self, min, max)
    }

    /// Ensures the sampled duration is non-negative.
    ///
    /// If the value remains negative after `limit_try_count` attempts, the `fallback` closure is invoked.
    fn ensure_non_negative<F>(
        self,
        limit_try_count: u8,
        fallback: F,
    ) -> EnsureNonNegativeSampler<Self, F>
    where
        F: FnMut(&mut dyn Rng, SimTime) -> Duration,
    {
        EnsureNonNegativeSampler::new(self, limit_try_count, fallback)
    }
}

impl<T: DurationSampler + Sized + 'static> CombinatorExt for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::prelude::SmallRng;

    #[test]
    fn test_pending_duration_new() {
        let pd = PendingDuration::new(10.5);
        assert_eq!(pd.raw_value(), 10.5);
    }

    #[test]
    fn test_pending_duration_from_duration() {
        let d = Duration::ticks(100);
        let pd = PendingDuration::from_duration(d);
        assert_eq!(pd.raw_value(), 100.0);
    }

    #[test]
    fn test_pending_duration_from_trait() {
        let d = Duration::ticks(200);
        let pd: PendingDuration = d.into();
        assert_eq!(pd.raw_value(), 200.0);
    }

    #[test]
    fn test_pending_duration_to_duration() {
        let pd = PendingDuration::new(10.5);
        let d = pd.to_duration();
        assert_eq!(d, Duration::ticks(11)); // Rounds up
    }

    #[test]
    fn test_pending_duration_to_duration_round_down() {
        let pd = PendingDuration::new(10.4);
        let d = pd.to_duration();
        assert_eq!(d, Duration::ticks(10)); // Rounds down
    }

    #[test]
    fn test_pending_duration_to_duration_negative_panics() {
        let pd = PendingDuration::new(-5.0);
        let result = std::panic::catch_unwind(|| pd.to_duration());
        assert!(result.is_err());
    }

    #[test]
    fn test_pending_duration_to_duration_with_clamp() {
        let pd = PendingDuration::new(10.5);
        let d = pd.to_duration_with_clamp(100);
        assert_eq!(d, Duration::ticks(11));

        let pd_large = PendingDuration::new(150.0);
        let d_clamped = pd_large.to_duration_with_clamp(100);
        assert_eq!(d_clamped, Duration::ticks(100));

        let pd_negative = PendingDuration::new(-5.0);
        let d_clamped_negative = pd_negative.to_duration_with_clamp(100);
        assert_eq!(d_clamped_negative, Duration::ticks(0));
    }

    #[test]
    fn test_pending_duration_to_duration_or_else() {
        let pd_positive = PendingDuration::new(5.5);
        let d = pd_positive.to_duration_or_else(|| Duration::ticks(0));
        assert_eq!(d, Duration::ticks(6));

        let pd_negative = PendingDuration::new(-5.5);
        let d_else = pd_negative.to_duration_or_else(|| Duration::ticks(10));
        assert_eq!(d_else, Duration::ticks(10));
    }

    #[test]
    fn test_pending_duration_try_duration() {
        let pd_positive = PendingDuration::new(5.5);
        assert_eq!(pd_positive.try_to_duration(), Some(Duration::ticks(6)));

        let pd_negative = PendingDuration::new(-5.5);
        assert_eq!(pd_negative.try_to_duration(), None);
    }

    #[test]
    fn test_pending_duration_add_duration() {
        let pd = PendingDuration::new(10.0);
        let d = Duration::ticks(5);
        let result = pd + d;
        assert_eq!(result.raw_value(), 15.0);
    }

    #[test]
    fn test_pending_duration_add_pending_duration() {
        let pd1 = PendingDuration::new(10.0);
        let pd2 = PendingDuration::new(7.5);
        let result = pd1 + pd2;
        assert_eq!(result.raw_value(), 17.5);
    }

    #[test]
    fn test_pending_duration_sub_duration() {
        let pd = PendingDuration::new(10.0);
        let d = Duration::ticks(3);
        let result = pd - d;
        assert_eq!(result.raw_value(), 7.0);
    }

    #[test]
    fn test_pending_duration_sub_pending_duration() {
        let pd1 = PendingDuration::new(10.0);
        let pd2 = PendingDuration::new(2.5);
        let result = pd1 - pd2;
        assert_eq!(result.raw_value(), 7.5);
    }

    #[test]
    fn test_pending_duration_apply() {
        let mut pd = PendingDuration::new(10.0);
        let result = pd.apply(|v| v * 2.0);
        assert_eq!(result.raw_value(), 20.0);
        assert_eq!(pd.raw_value(), 10.0); // Original value remains unchanged
    }

    /// Mock sampler for testing `CombinatorExt` functionality.
    struct MockSampler {
        value: f64,
    }

    impl DurationSampler for MockSampler {
        fn sample(&mut self, _rng: &mut dyn Rng, _current_tick: SimTime) -> PendingDuration {
            PendingDuration::new(self.value)
        }
    }

    #[test]
    fn test_combinator_ext_boxed() {
        let mut sampler = MockSampler { value: 10.0 };
        let mut rng = SmallRng::seed_from_u64(2);
        let current_tick = SimTime::from_ticks(0);
        assert_eq!(sampler.sample(&mut rng, current_tick).raw_value(), 10.0);
    }

    #[test]
    fn test_combinator_ext_map() {
        let sampler = MockSampler { value: 10.0 };
        let mut map_sampler = sampler.map(|_rng, _tick, v| v * 2.0);
        let mut rng = SmallRng::seed_from_u64(2);
        let current_tick = SimTime::from_ticks(0);
        assert_eq!(map_sampler.sample(&mut rng, current_tick).raw_value(), 20.0);
    }

    #[test]
    fn test_combinator_ext_delay() {
        let base_sampler = MockSampler { value: 10.0 };
        let delay_sampler_impl = MockSampler { value: 5.0 };
        let mut delay_sampler = base_sampler.delay(delay_sampler_impl.boxed());
        let mut rng = SmallRng::seed_from_u64(2);
        let current_tick = SimTime::from_ticks(0);
        assert_eq!(
            delay_sampler.sample(&mut rng, current_tick).raw_value(),
            15.0
        ); // base + delay
    }

    #[test]
    fn test_combinator_ext_jitter() {
        let base_sampler = MockSampler { value: 10.0 };
        let jitter_sampler_impl = MockSampler { value: 2.0 };
        let mut jitter_sampler = base_sampler.jitter(jitter_sampler_impl.boxed());
        let mut rng = SmallRng::seed_from_u64(2);
        let current_tick = SimTime::from_ticks(0);
        // Jitter adds or subtracts; for a fixed mock value, we expect base + jitter.
        assert_eq!(
            jitter_sampler.sample(&mut rng, current_tick).raw_value(),
            12.0
        );
    }

    #[test]
    fn test_combinator_ext_chain() {
        let s1 = MockSampler { value: 10.0 };
        let s2 = MockSampler { value: 5.0 };
        let mut chain_sampler = s1.chain(s2.boxed(), |_rng, _tick, v1, v2| v1 + v2 * 2.0);
        let mut rng = SmallRng::seed_from_u64(2);
        let current_tick = SimTime::from_ticks(0);
        assert_eq!(
            chain_sampler.sample(&mut rng, current_tick).raw_value(),
            20.0
        ); // 10.0 + 5.0 * 2.0
    }

    #[test]
    fn test_combinator_ext_aggregate() {
        let s1 = MockSampler { value: 10.0 };
        let s2 = MockSampler { value: 5.0 };
        let s3 = MockSampler { value: 2.0 };
        let others = vec![s2.boxed(), s3.boxed()];
        let mut aggregate_sampler =
            s1.aggregate(others, |_rng, _tick, values| values.iter().sum::<f64>());
        let mut rng = SmallRng::seed_from_u64(2);
        let current_tick = SimTime::from_ticks(0);
        assert_eq!(
            aggregate_sampler.sample(&mut rng, current_tick).raw_value(),
            17.0
        ); // 10.0 + 5.0 + 2.0
    }

    #[test]
    fn test_combinator_ext_clamp() {
        let sampler = MockSampler { value: 10.0 };
        let mut clamp_sampler = sampler.clamp(5.0, 8.0);
        let mut rng = SmallRng::seed_from_u64(2);
        let current_tick = SimTime::from_ticks(0);
        // Clamped to max
        assert_eq!(
            clamp_sampler.sample(&mut rng, current_tick).raw_value(),
            8.0
        );

        let sampler_low = MockSampler { value: 3.0 };
        let mut clamp_sampler_low = sampler_low.clamp(5.0, 8.0);
        // Clamped to min
        assert_eq!(
            clamp_sampler_low.sample(&mut rng, current_tick).raw_value(),
            5.0
        );

        let sampler_in_range = MockSampler { value: 6.0 };
        let mut clamp_sampler_in_range = sampler_in_range.clamp(5.0, 8.0);
        // In range
        assert_eq!(
            clamp_sampler_in_range
                .sample(&mut rng, current_tick)
                .raw_value(),
            6.0
        );
    }

    #[test]
    fn test_combinator_ext_ensure_non_negative() {
        struct NegativeSampler;
        impl DurationSampler for NegativeSampler {
            fn sample(&mut self, _rng: &mut dyn Rng, _current_tick: SimTime) -> PendingDuration {
                PendingDuration::new(-5.0)
            }
        }

        let sampler = NegativeSampler;
        let mut ensure_sampler = sampler.ensure_non_negative(3, |_rng, _tick| Duration::ticks(10));
        let mut rng = SmallRng::seed_from_u64(2);
        let current_tick = SimTime::from_ticks(0);
        // Should fall back to the provided duration
        assert_eq!(
            ensure_sampler.sample(&mut rng, current_tick).raw_value(),
            10.0
        );

        struct PositiveSampler;
        impl DurationSampler for PositiveSampler {
            fn sample(&mut self, _rng: &mut dyn Rng, _current_tick: SimTime) -> PendingDuration {
                PendingDuration::new(5.0)
            }
        }
        let sampler_pos = PositiveSampler;
        let mut ensure_sampler_pos =
            sampler_pos.ensure_non_negative(3, |_rng, _tick| Duration::ticks(10));
        assert_eq!(
            ensure_sampler_pos
                .sample(&mut rng, current_tick)
                .raw_value(),
            5.0
        );
    }
}
