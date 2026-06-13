use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::world::event::Event;
use crate::world::source::SourceView;

pub trait Hook<E>: Send {
    // Simulation lifecycle

    fn before_simulation(&mut self) {}

    /// skipped_duration は前回Tickから今回Tickまでに
    /// スキップされた時間。
    ///
    /// スキップ無効Runnerの場合は常に Duration::zero()。
    fn after_simulation(&mut self, end_sim_time: SimTime, skipped_duration: Duration) {}

    // Tick lifecycle

    /// skipped_duration は前回Tickから今回Tickまでに
    /// スキップされた時間。
    ///
    /// スキップ無効Runnerの場合は常に Duration::zero()。
    fn before_tick(&mut self, now: SimTime, skipped_duration: Duration) {}

    /// microstep_count はこのTickで実行された
    /// microstep数。
    fn after_tick(&mut self, now: SimTime, microstep_count: MicroStep) {}

    fn before_microstep(&mut self, now: SimTime, microstep: MicroStep) {}

    fn after_microstep(&mut self, now: SimTime, microstep: MicroStep) {}

    // Source lifecycle

    fn before_source(&mut self, now: SimTime, microstep: MicroStep, source: &SourceView<E>) {}

    fn after_source(
        &mut self,
        now: SimTime,
        microstep: MicroStep,
        source: &SourceView<E>,
        computed_next_fire: Option<SimTime>,
    ) {
    }

    // Event lifecycle

    fn before_event(&mut self, now: SimTime, microstep: MicroStep, event: &Event<E>) {}

    fn after_event(&mut self, now: SimTime, microstep: MicroStep, event: &Event<E>) {}
}
