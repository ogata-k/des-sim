use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::{Rng, RngExt};

pub struct ChoiceSampler {
    cdf: Vec<u64>,                         // 累積重みの境界値
    values: Vec<Box<dyn DurationSampler>>, // 対応するサンプラー
    total_weight: u64,                     // 合計重み
}

impl DurationSampler for ChoiceSampler {
    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> PendingDuration {
        // [0, total_weight) の範囲で乱数を生成
        let r = rng.random_range(0..self.total_weight);

        // 二分探索でrが属する区間のインデックスを探す
        // partition_point は x <= r となる最後のインデックスを返すので調整が必要
        let idx = self.cdf.partition_point(|&x| x <= r) - 1;

        self.values[idx].sample(rng, current_tick)
    }
}

impl ChoiceSampler {
    /// ヒストグラムの形式 [(DurationSampler, Weight)] からサンプラーを構築
    pub fn new(histogram: impl IntoIterator<Item = (Box<dyn DurationSampler>, u64)>) -> Self {
        // 経験分布からの逆変換サンプリング（Inverse Transform Sampling）でサンプリングできるようにあらかじめ変換
        let mut cdf = Vec::new();
        let mut values = Vec::new();
        let mut current_sum = 0;

        // 最初の境界は 0
        cdf.push(0);
        for (duration, weight) in histogram.into_iter() {
            current_sum += weight;
            cdf.push(current_sum);
            values.push(duration);
        }

        assert!(
            !values.is_empty(),
            "ChoiceSampler must have at least one sampler"
        );
        assert!(current_sum > 0, "Total weight must be greater than 0");

        Self {
            cdf,
            values,
            total_weight: current_sum,
        }
    }

    /// ヒストグラムの形式 [DurationSampler] からサンプラーを構築
    pub fn new_as_uniform(histogram: impl IntoIterator<Item = Box<dyn DurationSampler>>) -> Self {
        // 経験分布からの逆変換サンプリング（Inverse Transform Sampling）でサンプリングできるようにあらかじめ変換
        let mut cdf = Vec::new();
        let mut values = Vec::new();
        let mut current_sum = 0;

        // 最初の境界は 0
        cdf.push(0);
        for duration in histogram.into_iter() {
            current_sum += 1;
            cdf.push(current_sum);
            values.push(duration);
        }

        assert!(
            !values.is_empty(),
            "ChoiceSampler must have at least one sampler"
        );
        assert!(current_sum > 0, "Total weight must be greater than 0");

        Self {
            cdf,
            values,
            total_weight: current_sum,
        }
    }
}
