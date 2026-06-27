pub mod instance;

use crate::primitive::time::{Duration, SimTime};
use rand::Rng;

/// [Duration]を取得するためのヘルパートレイト
// なお、Box<dyn DurationSampler>とかく必要があるところがあるので、各メソッドでジェネリクスは使えない。
pub trait DurationSampler {
    fn try_sample(
        &mut self,
        rng: &mut dyn Rng,
        current_tick: SimTime,
        try_count: u8,
    ) -> Option<Duration>;

    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> Duration;
}
