use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::{Duration, SimTime};
use rand::Rng;

/// A sampler that returns durations from a fixed list in a cyclic (rotating) order.
///
/// # Note
/// This sampler maintains internal state (`next_index`). If you re-instantiate
/// this sampler frequently, the sequence will always start from the beginning.
#[derive(Debug, Clone)]
pub struct RotateSampler {
    next_index: usize,
    list: Vec<PendingDuration>,
    item_count: usize,
}

impl DurationSampler for RotateSampler {
    fn sample(&mut self, _rng: &mut dyn Rng, _current_tick: SimTime) -> PendingDuration {
        let result = self.list[self.next_index];
        self.next_index = (self.next_index + 1) % self.item_count;
        result
    }
}

impl RotateSampler {
    /// Creates a new `RotateSampler` starting from the first element.
    pub fn new(duration_list: impl IntoIterator<Item = PendingDuration>) -> RotateSampler {
        let list: Vec<_> = duration_list.into_iter().collect();
        assert!(
            !list.is_empty(),
            "RotateSampler requires at least one duration"
        );

        let item_count = list.len();
        RotateSampler {
            next_index: 0,
            list,
            item_count,
        }
    }

    /// Creates a new `RotateSampler` starting from a specific index.
    pub fn new_with_index(
        duration_list: impl IntoIterator<Item = PendingDuration>,
        next_index: usize,
    ) -> RotateSampler {
        let list: Vec<_> = duration_list.into_iter().collect();
        assert!(
            !list.is_empty(),
            "RotateSampler requires at least one duration"
        );
        assert!(next_index < list.len(), "Initial index is out of bounds");

        let item_count = list.len();
        RotateSampler {
            next_index,
            list,
            item_count,
        }
    }

    /// Returns the index of the next duration to be sampled.
    pub fn peek_next_index(&self) -> usize {
        self.next_index
    }

    /// Returns the total number of items in the rotation list.
    pub fn item_count(&self) -> usize {
        self.item_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive::time::Duration;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    #[test]
    fn test_rotate_sampler() {
        let mut rng = SmallRng::seed_from_u64(2);
        let durations = vec![
            Duration::ticks(1).into(),
            Duration::ticks(2).into(),
            Duration::ticks(3).into(),
        ];
        let mut sampler = RotateSampler::new(durations.clone());

        assert_eq!(sampler.peek_next_index(), 0);
        assert_eq!(sampler.item_count(), 3);

        assert_eq!(
            sampler.sample(&mut rng, SimTime::from_ticks(0)).raw_value(),
            1.0
        );
        assert_eq!(sampler.peek_next_index(), 1);

        assert_eq!(
            sampler.sample(&mut rng, SimTime::from_ticks(0)).raw_value(),
            2.0
        );
        assert_eq!(sampler.peek_next_index(), 2);

        assert_eq!(
            sampler.sample(&mut rng, SimTime::from_ticks(0)).raw_value(),
            3.0
        );
        assert_eq!(sampler.peek_next_index(), 0); // Wraps around

        assert_eq!(
            sampler.sample(&mut rng, SimTime::from_ticks(0)).raw_value(),
            1.0
        );
        assert_eq!(sampler.peek_next_index(), 1);
    }

    #[test]
    fn test_rotate_sampler_with_initial_index() {
        let mut rng = SmallRng::seed_from_u64(2);
        let durations = vec![
            Duration::ticks(1).into(),
            Duration::ticks(2).into(),
            Duration::ticks(3).into(),
        ];
        let mut sampler = RotateSampler::new_with_index(durations, 1);

        assert_eq!(sampler.peek_next_index(), 1);

        assert_eq!(
            sampler.sample(&mut rng, SimTime::from_ticks(0)).raw_value(),
            2.0
        );
        assert_eq!(sampler.peek_next_index(), 2);
    }

    #[test]
    #[should_panic(expected = "RotateSampler requires at least one duration")]
    fn test_rotate_sampler_empty_list1() {
        let _ = RotateSampler::new(vec![]);
    }

    #[test]
    #[should_panic(expected = "RotateSampler requires at least one duration")]
    fn test_rotate_sampler_empty_list2() {
        let _ = RotateSampler::new_with_index(vec![], 0);
    }

    #[test]
    #[should_panic(expected = "Initial index is out of bounds")]
    fn test_rotate_sampler_invalid_initial_index() {
        let durations = vec![Duration::ticks(1).into()];
        let _ = RotateSampler::new_with_index(durations, 1);
    }
}
