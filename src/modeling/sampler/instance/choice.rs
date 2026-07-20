//! The `choice` module provides the `ChoiceSampler`, a `DurationSampler`
//! that selects one of several sub-samplers based on a weighted distribution.
//!
//! This allows for modeling scenarios where the next event's duration
//! can come from different underlying processes, each with a specific
//! probability or weight. It supports both custom weighted distributions
//! and uniform selection.

use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::{Rng, RngExt};

/// A sampler that selects one of several sub-samplers based on a weighted distribution.
pub struct ChoiceSampler {
    cdf: Vec<u64>, // Cumulative distribution function (boundary values)
    values: Vec<Box<dyn DurationSampler>>, // Corresponding samplers
    total_weight: u64, // Sum of all weights
}

impl DurationSampler for ChoiceSampler {
    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> PendingDuration {
        // Generate a random number in the range [0, total_weight)
        let r = rng.random_range(0..self.total_weight);

        // Find the index of the interval to which r belongs using binary search.
        // `partition_point` returns the index of the first element that satisfies `!predicate`.
        // Since we want the last index where `x <= r`, we search for `x <= r` and subtract 1.
        let idx = self.cdf.partition_point(|&x| x <= r) - 1;

        self.values[idx].sample(rng, current_tick)
    }
}

impl ChoiceSampler {
    /// Constructs a `ChoiceSampler` from a collection of `(DurationSampler, Weight)` pairs.
    pub fn new(histogram: impl IntoIterator<Item = (Box<dyn DurationSampler>, u64)>) -> Self {
        // Converted in advance so that it can be sampled using Inverse Transform Sampling
        // from the empirical distribution
        let mut cdf = Vec::new();
        let mut values = Vec::new();
        let mut current_sum = 0;

        // The first boundary is 0
        cdf.push(0);
        for (duration, weight) in histogram.into_iter() {
            current_sum += weight;
            cdf.push(current_sum);
            values.push(duration);
        }

        assert!(
            !values.is_empty(),
            "ChoiceSampler must have at least one sampler with positive weight"
        );
        assert!(current_sum > 0, "Total weight must be greater than 0");

        Self {
            cdf,
            values,
            total_weight: current_sum,
        }
    }

    /// Constructs a `ChoiceSampler` with uniform weight (1) for all provided samplers.
    pub fn new_as_uniform(histogram: impl IntoIterator<Item = Box<dyn DurationSampler>>) -> Self {
        ChoiceSampler::new(histogram.into_iter().map(|s| (s, 1)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modeling::sampler::CombinatorExt;
    use crate::modeling::sampler::instance::ConstantSampler;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;
    use std::collections::HashMap;

    #[test]
    fn test_choice_sampler_weighted() {
        let mut rng = SmallRng::seed_from_u64(2);
        let s1 = ConstantSampler::new(10.0);
        let s2 = ConstantSampler::new(20.0);
        let s3 = ConstantSampler::new(30.0);

        // Weights: 1, 2, 1. Total = 4.
        let mut sampler =
            ChoiceSampler::new(vec![(s1.boxed(), 1), (s2.boxed(), 2), (s3.boxed(), 1)]);

        let mut results: HashMap<String, usize> = HashMap::new();
        for _ in 0..1000 {
            let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
            let entry = results
                .entry(format!("{:<.2}", sample.raw_value()))
                .or_insert(0);
            *entry += 1;
        }

        // Check if the distribution is roughly as expected
        // With 1000 samples, and weights 1:2:1, we expect roughly 250:500:250
        let count_10 = *results.get(&format!("{:<.2}", 10.0)).unwrap_or(&0);
        let count_20 = *results.get(&format!("{:<.2}", 20.0)).unwrap_or(&0);
        let count_30 = *results.get(&format!("{:<.2}", 30.0)).unwrap_or(&0);

        // Allow for some deviation due to randomness
        assert!(count_10 > 200 && count_10 < 300);
        assert!(count_20 > 450 && count_20 < 550);
        assert!(count_30 > 200 && count_30 < 300);
    }

    #[test]
    fn test_choice_sampler_uniform() {
        let mut rng = SmallRng::seed_from_u64(2);
        let s1 = ConstantSampler::new(10.0);
        let s2 = ConstantSampler::new(20.0);
        let s3 = ConstantSampler::new(30.0);

        let mut sampler = ChoiceSampler::new_as_uniform(vec![s1.boxed(), s2.boxed(), s3.boxed()]);

        let mut results: HashMap<String, usize> = HashMap::new();
        for _ in 0..1000 {
            let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
            let entry = results
                .entry(format!("{:<.2}", sample.raw_value()))
                .or_insert(0);
            *entry += 1_usize;
        }

        // With 1000 samples and 3 choices, we expect roughly 333 for each
        let count_10 = *results.get(&format!("{:<.2}", 10.0)).unwrap_or(&0);
        let count_20 = *results.get(&format!("{:<.2}", 20.0)).unwrap_or(&0);
        let count_30 = *results.get(&format!("{:<.2}", 30.0)).unwrap_or(&0);

        assert!(count_10 > 280 && count_10 < 380);
        assert!(count_20 > 280 && count_20 < 380);
        assert!(count_30 > 280 && count_30 < 380);
    }

    #[test]
    #[should_panic(expected = "ChoiceSampler must have at least one sampler")]
    fn test_choice_sampler_empty_histogram() {
        let _sampler = ChoiceSampler::new(vec![]);
    }

    #[test]
    #[should_panic(expected = "Total weight must be greater than 0")]
    fn test_choice_sampler_zero_total_weight() {
        let s1 = ConstantSampler::new(10.0);
        let _sampler = ChoiceSampler::new(vec![(s1.boxed(), 0)]);
    }
}
