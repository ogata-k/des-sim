//! The `aggregate` module provides the `AggregateSampler` and `AggregateBuilder`.
//!
//! `AggregateSampler` combines the outputs of multiple `DurationSampler` instances
//! using a custom aggregation function, allowing for complex statistical modeling.
//! `AggregateBuilder` offers a fluent API for constructing these aggregated samplers.

use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;

/// A builder for constructing an `AggregateSampler` in a fluent manner.
pub struct AggregateBuilder {
    samplers: Vec<Box<dyn DurationSampler>>,
}

impl AggregateBuilder {
    /// Initializes the builder with the first sampler.
    pub fn from_sampler(sampler: impl DurationSampler + 'static) -> Self {
        AggregateBuilder {
            samplers: vec![Box::new(sampler)],
        }
    }

    /// Adds another sampler to the aggregation list.
    pub fn add_sampler(mut self, sampler: Box<dyn DurationSampler>) -> Self {
        self.samplers.push(sampler);
        self
    }

    /// Consumes the builder to produce an `AggregateSampler` using the provided function `f`.
    pub fn build<F>(self, f: F) -> AggregateSampler<F>
    where
        F: FnMut(&mut dyn Rng, SimTime, Vec<f64>) -> f64,
    {
        AggregateSampler {
            samplers: self.samplers,
            f,
        }
    }
}

/// A sampler that aggregates results from multiple sub-samplers using a closure.
pub struct AggregateSampler<F>
where
    F: FnMut(&mut dyn Rng, SimTime, Vec<f64>) -> f64,
{
    samplers: Vec<Box<dyn DurationSampler>>,
    f: F,
}

impl<F> DurationSampler for AggregateSampler<F>
where
    F: FnMut(&mut dyn Rng, SimTime, Vec<f64>) -> f64,
{
    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> PendingDuration {
        let mut sampled_list = Vec::with_capacity(self.samplers.len());
        for sampler in &mut self.samplers {
            let sampled = sampler.sample(rng, current_tick);
            sampled_list.push(sampled.raw_value());
        }

        PendingDuration::new((self.f)(rng, current_tick, sampled_list))
    }
}

impl<F> AggregateSampler<F>
where
    F: FnMut(&mut dyn Rng, SimTime, Vec<f64>) -> f64,
{
    /// Creates a new `AggregateSampler`. Panics if `samplers` is empty.
    pub fn new(samplers: impl IntoIterator<Item = Box<dyn DurationSampler>>, f: F) -> Self {
        let samplers: Vec<_> = samplers.into_iter().collect();

        assert!(
            !samplers.is_empty(),
            "AggregateSampler requires at least one sampler."
        );

        AggregateSampler { samplers, f }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modeling::sampler::CombinatorExt;
    use crate::modeling::sampler::instance::ConstantSampler;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    #[test]
    fn test_aggregate_sampler_sum() {
        let mut rng = SmallRng::seed_from_u64(2);
        let s1 = ConstantSampler::new(10.0);
        let s2 = ConstantSampler::new(20.0);
        let s3 = ConstantSampler::new(30.0);

        let mut sampler =
            AggregateSampler::new(vec![s1.boxed(), s2.boxed(), s3.boxed()], |_, _, samples| {
                samples.iter().sum()
            });

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 60.0);
    }

    #[test]
    fn test_aggregate_sampler_builder() {
        let mut rng = SmallRng::seed_from_u64(2);
        let s1 = ConstantSampler::new(5.0);
        let s2 = ConstantSampler::new(15.0);

        let mut sampler = AggregateBuilder::from_sampler(s1)
            .add_sampler(s2.boxed())
            .build(|_, _, samples| samples[0] * samples[1]);

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 75.0); // 5.0 * 15.0
    }

    #[test]
    #[should_panic]
    fn test_aggregate_sampler_empty_samplers() {
        let _sampler = AggregateSampler::new(vec![], |_, _, _| 0.0);
    }
}
