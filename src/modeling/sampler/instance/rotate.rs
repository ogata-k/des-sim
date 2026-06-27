use crate::modeling::sampler::DurationSampler;
use crate::primitive::time::{Duration, SimTime};
use rand::Rng;

///
/// ※ 内部状態を持つので、毎回作り直していると常に最初から二なってしまうので注意
pub struct RotateSampler {
    pub next_index: usize,
    list: Vec<Duration>,
    item_count: usize,
}

impl DurationSampler for RotateSampler {
    fn try_sample(
        &mut self,
        rng: &mut dyn Rng,
        current_tick: SimTime,
        _try_count: u8,
    ) -> Option<Duration> {
        Some(self.sample(rng, current_tick))
    }

    fn sample(&mut self, _rng: &mut dyn Rng, _current_tick: SimTime) -> Duration {
        let result = self.list[self.next_index];
        self.next_index = (self.next_index + 1) % self.item_count;
        result
    }
}

impl RotateSampler {
    pub fn new(duration_list: Vec<Duration>) -> RotateSampler {
        assert!(!duration_list.is_empty());

        let item_count = duration_list.len();
        RotateSampler {
            next_index: 0,
            list: duration_list,
            item_count,
        }
    }

    pub fn new_with_index(duration_list: Vec<Duration>, next_index: usize) -> RotateSampler {
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
