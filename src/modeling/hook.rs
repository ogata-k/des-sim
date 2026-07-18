pub mod instance;

use crate::modeling::event::Event;
use crate::modeling::model::Model;
use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::source_handler::{SourceReadyEntry, SourceView};

/// A trait for hooking into lifecycle events of a simulation for extension or monitoring.
///
/// Implementing `Hook` allows for logging, tracing, state assertions, or dynamic
/// intervention during simulation execution. Since every method receives the
/// precise simulation state (time, step, model context), this trait provides the
/// foundation for building powerful diagnostic and analysis tools.
pub trait Hook<E, M: Model<E>> {
    // --- Simulation Lifecycle ---

    /// Invoked immediately before the simulation starts.
    fn before_simulation(&self, model: &M);

    /// Invoked immediately after the simulation finishes.
    fn after_simulation(&self, model: &M, end_tick: SimTime);

    // --- Tick Lifecycle ---

    /// Invoked at the beginning of a tick process.
    ///
    /// # Arguments
    /// * `skipped_duration` - The time skipped since the previous tick. When use not-skippable `Runner`, always 0 duration.
    fn before_tick(&self, model: &M, current_tick: SimTime, skipped_duration: Duration);

    /// Invoked immediately after a tick process has completed.
    fn after_tick(&self, model: &M, current_tick: SimTime, last_micro_step: MicroStep);

    /// Invoked at the beginning of a micro-step.
    fn before_micro_step(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep);

    /// Invoked at the end of a micro-step.
    fn after_micro_step(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep);

    /// Invoked when remaining tasks (events or sources) are discarded due to execution limits.
    fn on_discard_remain_micro_step(
        &self,
        model: &M,
        current_tick: SimTime,
        first_discarded_micro_step: MicroStep,
        discarded_sources: &[SourceReadyEntry],
        discarded_events: &[Event<E>],
    );

    // --- Source Lifecycle ---

    /// Invoked when a source registration begins.
    fn before_register_source(&self, model: &M, name: &str);

    /// Invoked when a source registration completes.
    fn after_register_source(&self, model: &M, name: &str);

    /// Invoked at the beginning of the source processing phase.
    fn before_source_phase(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep);

    /// Invoked immediately before an individual source fires.
    fn before_source(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        source_view: &SourceView,
    );

    /// Invoked immediately after an individual source fires.
    fn after_source(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        source_view: &SourceView,
        computed_next_fire: Option<SimTime>,
    );

    /// Invoked when a scheduled source is canceled.
    fn cancel_source(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        scheduled_at: SimTime,
        source_view: &SourceView,
    );

    /// Invoked when a source is explicitly discarded.
    fn discard_source(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        source_view: &SourceView,
    );

    /// Invoked at the end of the source processing phase.
    fn after_source_phase(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep);

    // --- Event Lifecycle ---

    /// Invoked at the beginning of the event processing phase.
    fn before_event_phase(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep);

    /// Invoked immediately before an event is processed.
    fn before_event(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        event: &Event<E>,
    );

    /// Invoked immediately after an event is processed.
    fn after_event(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        event: &Event<E>,
    );

    /// Invoked when a scheduled event is canceled.
    fn cancel_event(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        scheduled_at: SimTime,
        event: &Event<E>,
    );

    /// Invoked when an event is discarded.
    fn discard_event(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        event: &Event<E>,
    );

    /// Invoked at the end of the event processing phase.
    fn after_event_phase(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep);
}
