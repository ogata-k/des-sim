use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::{Duration, SimTime};
use rand::Rng;

///
/// ※ 内部状態を持つので、毎回作り直していると常に最初から二なってしまうので注意
#[derive(Debug, Clone)]
pub struct RotateSampler {
    next_index: usize,
    list: Vec<Duration>,
    item_count: usize,
}

impl DurationSampler for RotateSampler {
    fn sample(&mut self, _rng: &mut dyn Rng, _current_tick: SimTime) -> PendingDuration {
        let result = self.list[self.next_index];
        self.next_index = (self.next_index + 1) % self.item_count;
        result.into()
    }
}

impl RotateSampler {
    pub fn new(duration_list: impl IntoIterator<Item = Duration>) -> RotateSampler {
        let duration_list: Vec<_> = duration_list.into_iter().collect();

        assert!(!duration_list.is_empty());

        let item_count = duration_list.len();
        RotateSampler {
            next_index: 0,
            list: duration_list,
            item_count,
        }
    }

    pub fn new_with_index(
        duration_list: impl IntoIterator<Item = Duration>,
        next_index: usize,
    ) -> RotateSampler {
        let duration_list: Vec<_> = duration_list.into_iter().collect();

        assert!(!duration_list.is_empty());
        assert!(next_index < duration_list.len());

        let item_count = duration_list.len();
        RotateSampler {
            next_index,
            list: duration_list,
            item_count,
        }
    }

    pub fn peek_next_index(&self) -> usize {
        self.next_index
    }

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
        let durations = vec![Duration::ticks(1), Duration::ticks(2), Duration::ticks(3)];
        let mut sampler = RotateSampler::new(durations.clone());

        assert_eq!(sampler.peek_next_index(), 0);
        assert_eq!(sampler.item_count(), 3);

        assert_eq!(sampler.sample(&mut rng, SimTime::new(0)).raw_value(), 1.0);
        assert_eq!(sampler.peek_next_index(), 1);

        assert_eq!(sampler.sample(&mut rng, SimTime::new(0)).raw_value(), 2.0);
        assert_eq!(sampler.peek_next_index(), 2);

        assert_eq!(sampler.sample(&mut rng, SimTime::new(0)).raw_value(), 3.0);
        assert_eq!(sampler.peek_next_index(), 0); // Rotates back to start

        assert_eq!(sampler.sample(&mut rng, SimTime::new(0)).raw_value(), 1.0);
        assert_eq!(sampler.peek_next_index(), 1);
    }

    #[test]
    fn test_rotate_sampler_with_initial_index() {
        let mut rng = SmallRng::seed_from_u64(2);
        let durations = vec![Duration::ticks(1), Duration::ticks(2), Duration::ticks(3)];
        let mut sampler = RotateSampler::new_with_index(durations.clone(), 1);

        assert_eq!(sampler.peek_next_index(), 1);

        assert_eq!(sampler.sample(&mut rng, SimTime::new(0)).raw_value(), 2.0);
        assert_eq!(sampler.peek_next_index(), 2);
    }

    #[test]
    #[should_panic]
    fn test_rotate_sampler_empty_list() {
        let _sampler = RotateSampler::new(vec![]);
    }

    #[test]
    #[should_panic]
    fn test_rotate_sampler_invalid_initial_index() {
        let durations = vec![Duration::ticks(1)];
        let _sampler = RotateSampler::new_with_index(durations, 1); // Index out of bounds
    }
}
