pub mod instance;

use crate::modeling::event::Event;
use crate::modeling::model::Model;
use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::source_handler::{SourceReadyEntry, SourceView};

pub trait Hook<E, M: Model<E>> {
    // Simulation lifecycle

    fn before_simulation(&self, model: &M);

    fn after_simulation(&self, model: &M, end_sim_time: SimTime);

    // Tick lifecycle

    /// skipped_duration は前回Tickから今回Tickまでに
    /// スキップされた時間。
    ///
    /// スキップ無効Runnerの場合は常に Duration::zero()。
    fn before_tick(&self, model: &M, now: SimTime, skipped_duration: Duration);

    /// micro_step_count はこのTickで実行された
    /// micro_step数。
    fn after_tick(&self, model: &M, now: SimTime, micro_step_count: MicroStep);

    fn before_micro_step(&self, model: &M, now: SimTime, micro_step: MicroStep);

    fn after_micro_step(&self, model: &M, now: SimTime, micro_step: MicroStep);

    fn on_discard_remain_micro_step(
        &self,
        model: &M,
        now: SimTime,
        first_discarded_micro_step: MicroStep,
        discarded_sources: &[SourceReadyEntry],
        discarded_events: &[Event<E>],
    );

    // Source lifecycle

    fn before_source_phase(&self, model: &M, now: SimTime, micro_step: MicroStep);

    fn before_source(
        &self,
        model: &M,
        now: SimTime,
        micro_step: MicroStep,
        source_view: &SourceView,
    );

    fn after_source(
        &self,
        model: &M,
        now: SimTime,
        micro_step: MicroStep,
        source_view: &SourceView,
        computed_next_fire: Option<SimTime>,
    );

    fn discard_source(
        &self,
        model: &M,
        now: SimTime,
        micro_step: MicroStep,
        source_view: &SourceView,
    );

    fn after_source_phase(&self, model: &M, now: SimTime, micro_step: MicroStep);

    // Event lifecycle

    fn before_event_phase(&self, model: &M, now: SimTime, micro_step: MicroStep);

    fn before_event(&self, model: &M, now: SimTime, micro_step: MicroStep, event: &Event<E>);

    fn after_event(&self, model: &M, now: SimTime, micro_step: MicroStep, event: &Event<E>);

    fn cancel_event(
        &self,
        model: &M,
        now: SimTime,
        micro_step: MicroStep,
        scheduled_at: SimTime,
        event: &Event<E>,
    );

    fn discard_event(&self, model: &M, now: SimTime, micro_step: MicroStep, event: &Event<E>);

    fn after_event_phase(&self, model: &M, now: SimTime, micro_step: MicroStep);
}
