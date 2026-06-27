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
    pub fn new(trigger: T, samplers: Vec<Box<dyn DurationSampler>>) -> Self {
        // フォールバック先に必要なのでどんな場合でも必須
        assert!(!samplers.is_empty());
        // 想定最大値をもとに範囲外にあふれる可能性があるかどうかを判定
        debug_assert!(trigger.max_possible_index_hint() < samplers.len());

        Self { trigger, samplers }
    }
}
