use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;

/// [ModeSampler]がどれを使ってサンプリングすべきかを特定するためのヘルパートレイト
pub trait TimeTrigger {
    /// 戻り値として「どのインデックスの分布を使うべきか」を返す
    fn get_active_index(&self, now: SimTime) -> usize;

    /// 戻り値として想定しているインデックス上限を返す
    fn max_possible_index_hint(&self) -> usize;
}

pub struct ModeSampler<T: TimeTrigger> {
    trigger: T,
    samplers: Vec<Box<dyn DurationSampler>>,
}

impl<T: TimeTrigger> DurationSampler for ModeSampler<T> {
    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> PendingDuration {
        let index = self.trigger.get_active_index(current_tick);

        // 境界チェック：範囲外なら先頭のサンプラーを使う。
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
    pub fn new(trigger: T, samplers: impl IntoIterator<Item = Box<dyn DurationSampler>>) -> Self {
        let samplers: Vec<_> = samplers.into_iter().collect();

        // フォールバック先に必要なのでどんな場合でも必須
        assert!(!samplers.is_empty());
        // 想定最大値をもとに範囲外にあふれる可能性があるかどうかを判定
        debug_assert!(trigger.max_possible_index_hint() < samplers.len());

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

        let sample = sampler.sample(&mut rng, SimTime::new(0));
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

        let sample = sampler.sample(&mut rng, SimTime::new(0));
        assert_eq!(sample.raw_value(), 10.0); // Should fall back to s1
    }

    #[test]
    #[should_panic]
    fn test_mode_sampler_empty_samplers() {
        let trigger = MockTimeTrigger {
            active_index: 0,
            max_index_hint: 0,
        };
        let _sampler = ModeSampler::new(trigger, vec![]);
    }
}
