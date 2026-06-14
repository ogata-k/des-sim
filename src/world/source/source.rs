use crate::primitive::time::{Duration, SimTime};
use crate::world::source::SourceContext;

pub trait Source<E>: Send {
    /// 発火したときの処理。戻り値は次の発火時刻。
    fn fire(&mut self, now: SimTime, context: &mut SourceContext<E>) -> Option<Duration>;
}
