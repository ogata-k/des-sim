use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::{Duration, SimTime};
use rand::{Rng, RngExt};

/// A sampler that draws from a discrete empirical distribution.
///
/// Samples are generated using inverse transform sampling, where values are selected
/// based on their relative weights.
#[derive(Debug, Clone)]
pub struct EmpiricalSampler {
    cdf: Vec<u64>,         // Cumulative distribution function (boundary values)
    values: Vec<Duration>, // Corresponding values
    total_weight: u64,     // Sum of all weights
}

impl DurationSampler for EmpiricalSampler {
    fn sample(&mut self, rng: &mut dyn Rng, _current_tick: SimTime) -> PendingDuration {
        // Generate a random number in the range [0, total_weight)
        let r = rng.random_range(0..self.total_weight);

        // Find the index of the interval to which r belongs using binary search.
        // `partition_point` returns the index of the first element that satisfies `!predicate`.
        // Since we want the last index where `x <= r`, we search for `x <= r` and subtract 1.
        let idx = self.cdf.partition_point(|&x| x <= r) - 1;

        self.values[idx].into()
    }
}

impl EmpiricalSampler {
    /// Constructs an `EmpiricalSampler` from a collection of `(Duration, Weight)` pairs.
    pub fn new(histogram: impl IntoIterator<Item = (Duration, u64)>) -> Self {
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
            "EmpiricalSampler must have at least one sampler"
        );
        assert!(current_sum > 0, "Total weight must be greater than 0");

        Self {
            cdf,
            values,
            total_weight: current_sum,
        }
    }

    /// Constructs an `EmpiricalSampler` with uniform weight (1) for all provided durations.
    pub fn new_as_uniform(histogram: impl IntoIterator<Item = Duration>) -> Self {
        EmpiricalSampler::new(histogram.into_iter().map(|d| (d, 1)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modeling::sampler::instance::ExponentialSampler;
    use crate::primitive::time::Duration;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;
    use std::collections::HashMap;

    #[test]
    fn test_empirical_sampler_weighted() {
        let mut rng = SmallRng::seed_from_u64(2);
        let d1 = Duration::ticks(10);
        let d2 = Duration::ticks(20);
        let d3 = Duration::ticks(30);

        // Weights: 1, 2, 1. Total: 4.
        let mut sampler = EmpiricalSampler::new(vec![(d1, 1), (d2, 2), (d3, 1)]);

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
    fn test_empirical_sampler_uniform() {
        let mut rng = SmallRng::seed_from_u64(2);
        let d1 = Duration::ticks(10);
        let d2 = Duration::ticks(20);
        let d3 = Duration::ticks(30);

        let mut sampler = EmpiricalSampler::new_as_uniform(vec![d1, d2, d3]);

        let mut results: HashMap<String, usize> = HashMap::new();
        for _ in 0..1000 {
            let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
            let entry = results
                .entry(format!("{:<.2}", sample.raw_value()))
                .or_insert(0);
            *entry += 1;
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
    #[should_panic(expected = "EmpiricalSampler must have at least one sampler")]
    fn test_empirical_sampler_empty_histogram() {
        let _sampler = EmpiricalSampler::new(vec![]);
    }

    #[test]
    #[should_panic(expected = "Total weight must be greater than 0")]
    fn test_empirical_sampler_zero_total_weight() {
        let d1 = Duration::ticks(10);
        let _sampler = EmpiricalSampler::new(vec![(d1, 0)]);
    }

    #[test]
    fn test_exponential_sampler() {
        let mut rng = SmallRng::seed_from_u64(2);
        let lambda = 0.5; // mean = 1/lambda = 2.0
        let mut sampler = ExponentialSampler::new(lambda).unwrap();

        let mut samples = Vec::new();
        for _ in 0..10000 {
            samples.push(sampler.sample(&mut rng, SimTime::from_ticks(0)).raw_value());
        }

        let mean: f64 = samples.iter().sum::<f64>() / samples.len() as f64;
        let expected_mean = 1.0 / lambda;

        // Check if the sampled mean is close to the expected mean
        assert!(
            (mean - expected_mean).abs() < 0.1,
            "Mean: {}, Expected: {}",
            mean,
            expected_mean
        );

        // Check if all samples are non-negative
        assert!(samples.iter().all(|&x| x >= 0.0));
    }
}
