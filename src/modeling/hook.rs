pub mod instance;

use crate::modeling::event::Event;
use crate::modeling::model::Model;
use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::source_handler::{SourceReadyEntry, SourceView};

pub trait Hook<E, M: Model<E>> {
    // Simulation lifecycle

    fn before_simulation(&self, model: &M);

    fn after_simulation(&self, model: &M, end_tick: SimTime);

    // Tick lifecycle

    /// skipped_duration は前回Tickから今回Tickまでに
    /// スキップされた時間。
    ///
    /// スキップ無効Runnerの場合は常に Duration::zero()。
    fn before_tick(&self, model: &M, current_tick: SimTime, skipped_duration: Duration);

    fn after_tick(&self, model: &M, current_tick: SimTime, last_micro_step: MicroStep);

    fn before_micro_step(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep);

    fn after_micro_step(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep);

    fn on_discard_remain_micro_step(
        &self,
        model: &M,
        current_tick: SimTime,
        first_discarded_micro_step: MicroStep,
        discarded_sources: &[SourceReadyEntry],
        discarded_events: &[Event<E>],
    );

    // Source lifecycle

    fn before_register_source(&self, model: &M, name: &str);

    fn after_register_source(&self, model: &M, name: &str);

    fn before_source_phase(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep);

    fn before_source(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        source_view: &SourceView,
    );

    fn after_source(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        source_view: &SourceView,
        computed_next_fire: Option<SimTime>,
    );

    fn cancel_source(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        scheduled_at: SimTime,
        source_view: &SourceView,
    );

    fn discard_source(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        source_view: &SourceView,
    );

    fn after_source_phase(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep);

    // Event lifecycle

    fn before_event_phase(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep);

    fn before_event(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        event: &Event<E>,
    );

    fn after_event(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        event: &Event<E>,
    );

    fn cancel_event(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        scheduled_at: SimTime,
        event: &Event<E>,
    );

    fn discard_event(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        event: &Event<E>,
    );

    fn after_event_phase(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep);
}
