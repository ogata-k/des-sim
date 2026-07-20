//! The `mode` module provides the `ModeSampler`, a `DurationSampler` that
//! dynamically switches between different sub-samplers based on a `TimeTrigger`.
//!
//! This allows for modeling systems whose behavior changes over time, such as
//! different operating modes or time-of-day effects. The `TimeTrigger` determines
//! which sampler is active at any given simulation time.

use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;

/// A trait to determine which sampler should be active based on the current simulation time.
pub trait TimeTrigger {
    /// Returns the index of the sampler that should be active at the given simulation time.
    fn get_active_index(&self, now: SimTime) -> usize;

    /// Returns a hint about the maximum possible index to allow for pre-check optimizations.
    fn max_possible_index_hint(&self) -> usize;
}

/// A sampler that switches between different sub-samplers based on a [TimeTrigger].
pub struct ModeSampler<T: TimeTrigger> {
    trigger: T,
    samplers: Vec<Box<dyn DurationSampler>>,
}

impl<T: TimeTrigger> DurationSampler for ModeSampler<T> {
    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> PendingDuration {
        let index = self.trigger.get_active_index(current_tick);

        // Boundary check: if out of range, fall back to the first sampler (index 0).
        if let Some(sampler) = self.samplers.get_mut(index) {
            sampler.sample(rng, current_tick)
        } else {
            log::warn!(
                "ModeSampler index {} out of bounds, falling back to 0",
                index
            );
            self.samplers[0].sample(rng, current_tick)
        }
    }
}

impl<T: TimeTrigger> ModeSampler<T> {
    /// Creates a new `ModeSampler`.
    ///
    /// # Panics
    /// Panics if the provided samplers collection is empty.
    pub fn new(trigger: T, samplers: impl IntoIterator<Item = Box<dyn DurationSampler>>) -> Self {
        let samplers: Vec<_> = samplers.into_iter().collect();

        assert!(
            !samplers.is_empty(),
            "ModeSampler requires at least one sampler"
        );
        debug_assert!(
            trigger.max_possible_index_hint() < samplers.len(),
            "ModeSampler trigger hint exceeds sampler collection bounds"
        );

        Self { trigger, samplers }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modeling::sampler::CombinatorExt;
    use crate::modeling::sampler::instance::ConstantSampler;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    // Mock TimeTrigger for testing ModeSampler
    struct MockTimeTrigger {
        active_index: usize,
        max_index_hint: usize,
    }

    impl TimeTrigger for MockTimeTrigger {
        fn get_active_index(&self, _now: SimTime) -> usize {
            self.active_index
        }

        fn max_possible_index_hint(&self) -> usize {
            self.max_index_hint
        }
    }

    #[test]
    fn test_mode_sampler_valid_index() {
        let mut rng = SmallRng::seed_from_u64(2);
        let s1 = ConstantSampler::new(10.0);
        let s2 = ConstantSampler::new(20.0);
        let s3 = ConstantSampler::new(30.0);

        let trigger = MockTimeTrigger {
            active_index: 1,
            max_index_hint: 2,
        };
        let mut sampler = ModeSampler::new(trigger, vec![s1.boxed(), s2.boxed(), s3.boxed()]);

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 20.0); // Should sample from s2
    }

    #[test]
    fn test_mode_sampler_out_of_bounds_fallback() {
        let mut rng = SmallRng::seed_from_u64(2);
        let s1 = ConstantSampler::new(10.0);
        let s2 = ConstantSampler::new(20.0);

        let trigger = MockTimeTrigger {
            active_index: 5, // Out of bounds
            max_index_hint: 1,
        };
        let mut sampler = ModeSampler::new(trigger, vec![s1.boxed(), s2.boxed()]);

        let sample = sampler.sample(&mut rng, SimTime::from_ticks(0));
        assert_eq!(sample.raw_value(), 10.0); // Should fall back to s1
    }

    #[test]
    #[should_panic(expected = "ModeSampler requires at least one sampler")]
    fn test_mode_sampler_empty_samplers() {
        let trigger = MockTimeTrigger {
            active_index: 0,
            max_index_hint: 0,
        };
        let _sampler = ModeSampler::new(trigger, vec![]);
    }
}
