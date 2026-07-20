//! The `trace` module provides the `TraceHook`, a built-in `Hook` implementation
//! for logging and tracing simulation events and state changes.
//!
//! This hook outputs detailed information about the simulation's progression,
//! including tick and micro-step transitions, source firings, event processing,
//! and model summaries. It is highly configurable via feature flags for verbosity.

use crate::modeling::event::Event;
use crate::modeling::hook::Hook;
use crate::modeling::model::Model;
use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::source_handler::{SourceReadyEntry, SourceView};
use log::{debug, info, trace};
use std::fmt;

/// A trait for providing a concise summary representation of a model.
pub trait ModelSummary {
    /// Formats the model's summary for display.
    fn summary(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
}

/// An adapter that bridges the `ModelSummary` trait with `fmt::Display`.
struct ModelLogAdapter<'a, M>(&'a M);

impl<'a, M: ModelSummary> fmt::Display for ModelLogAdapter<'a, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.summary(f)
    }
}

/// A [Hook] that traces the simulation state.
/// If the `verbose_debug` feature is enabled, it also outputs the model's detailed state
/// using the [Debug] trait.
pub struct TraceHook;

impl<E, M> Hook<E, M> for TraceHook
where
    E: fmt::Debug,
    M: Model<E> + ModelSummary + fmt::Debug,
{
    fn before_simulation(&self, model: &M) {
        info!("--- [SIMULATION START] Time: {:?} ---", SimTime::zero());
        self.info_log_model(model, "");
    }

    fn after_simulation(&self, model: &M, end_tick: SimTime) {
        info!("--- [SIMULATION END] Time: {:?} ---", end_tick);
        self.info_log_model(model, "");
    }

    fn before_tick(&self, model: &M, current_tick: SimTime, skipped_duration: Duration) {
        info!(
            "  >>> Tick at {:?} (skipped: {} ticks)",
            current_tick, skipped_duration
        );
        self.info_log_model(model, "  ");
    }

    fn after_tick(&self, model: &M, current_tick: SimTime, last_micro_step: MicroStep) {
        info!(
            "  <<< Tick at {:?} finished (last μSteps: {})",
            current_tick, last_micro_step
        );
        self.info_log_model(model, "  ");
    }

    fn before_micro_step(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep) {
        debug!(
            "  [μStep: {} at {:?}] START",
            current_micro_step, current_tick
        );
        self.debug_log_model(model, "  ");
    }

    fn after_micro_step(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep) {
        debug!(
            "  [μStep: {} at {:?}] END",
            current_micro_step, current_tick
        );
        self.debug_log_model(model, "  ");
    }

    fn on_discard_remain_micro_step(
        &self,
        model: &M,
        current_tick: SimTime,
        first_discarded_micro_step: MicroStep,
        discarded_sources: &[SourceReadyEntry],
        discarded_events: &[Event<E>],
    ) {
        debug!(
            "  !!! [DISCARD REMAINS at {:?}] Start μStep: {}, Sources: {:?}, Events: {:?}",
            current_tick, first_discarded_micro_step, discarded_sources, discarded_events
        );
        self.debug_log_model(model, "  ");
    }

    fn before_register_source(&self, model: &M, name: &str) {
        debug!("> Start Register Source: {}", name);
        self.debug_log_model(model, "");
    }

    fn after_register_source(&self, model: &M, name: &str) {
        debug!("< After Register Source: {}", name);
        self.debug_log_model(model, "");
    }

    fn before_source_phase(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep) {
        debug!(
            "    > Source Phase at {:?} (μStep: {})",
            current_tick, current_micro_step
        );
        self.debug_log_model(model, "    ");
    }

    fn before_source(
        &self,
        model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        source_view: &SourceView,
    ) {
        trace!("      + Source firing | Source: {:?}", source_view);
        self.trace_log_model(model, "      ");
    }

    fn after_source(
        &self,
        model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        source_view: &SourceView,
        computed_next_fire: Option<SimTime>,
    ) {
        trace!(
            "      - Source finished (next: after {:?} tick) | Source: {:?}",
            computed_next_fire, source_view
        );
        self.trace_log_model(model, "      ");
    }

    fn cancel_source(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        scheduled_at: SimTime,
        source_view: &SourceView,
    ) {
        debug!(
            "    ! Source canceled at {:?} (current μStep: {}) | Source: {:?} at scheduled: {}",
            current_tick, current_micro_step, source_view, scheduled_at
        );
        self.debug_log_model(model, "    ");
    }

    fn discard_source(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        source_view: &SourceView,
    ) {
        debug!(
            "    ! Source discarded at {:?} (last handle μStep: {}) | Source: {:?}",
            current_tick, current_micro_step, source_view
        );
        self.debug_log_model(model, "    ");
    }

    fn after_source_phase(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep) {
        debug!(
            "    < Source Phase finished at {:?} (μStep: {})",
            current_tick, current_micro_step,
        );
        self.debug_log_model(model, "    ");
    }

    fn before_event_phase(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep) {
        debug!(
            "    > Event Phase at {:?} (μStep: {})",
            current_tick, current_micro_step
        );
        self.debug_log_model(model, "    ");
    }

    fn before_event(
        &self,
        model: &M,
        _current_tick: SimTime,
        _micro_step: MicroStep,
        event: &Event<E>,
    ) {
        trace!("      + Event firing | Event: {:?}", event);
        self.trace_log_model(model, "      ");
    }

    fn after_event(
        &self,
        model: &M,
        _current_tick: SimTime,
        _micro_step: MicroStep,
        event: &Event<E>,
    ) {
        trace!("      - Event finished | Event: {:?}", event);
        self.trace_log_model(model, "      ");
    }

    fn cancel_event(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        scheduled_at: SimTime,
        event: &Event<E>,
    ) {
        debug!(
            "    ! Event canceled at {:?} (current μStep: {}) | Event: {:?} at scheduled: {}",
            current_tick, current_micro_step, event, scheduled_at
        );
        self.debug_log_model(model, "    ");
    }

    fn discard_event(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        event: &Event<E>,
    ) {
        debug!(
            "    ! Event canceled at {:?} (last handle μStep: {}) | Event: {:?}",
            current_tick, current_micro_step, event
        );
        self.debug_log_model(model, "    ");
    }

    fn after_event_phase(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep) {
        debug!(
            "    < Event Phase finished at {:?} (μStep: {})",
            current_tick, current_micro_step,
        );
        self.debug_log_model(model, "    ");
    }
}

impl TraceHook {
    fn info_log_model<M>(&self, model: &M, prefix: &'static str)
    where
        M: ModelSummary + fmt::Debug,
    {
        info!("{}  model summary: {}", prefix, ModelLogAdapter(model));

        if cfg!(feature = "verbose_debug") {
            info!("{}  model: {:?}", prefix, model);
        }
    }

    fn debug_log_model<M>(&self, model: &M, prefix: &'static str)
    where
        M: ModelSummary + fmt::Debug,
    {
        debug!("{}  model summary: {}", prefix, ModelLogAdapter(model));

        if cfg!(feature = "verbose_debug") {
            debug!("{}  model: {:?}", prefix, model);
        }
    }

    fn trace_log_model<M>(&self, model: &M, prefix: &'static str)
    where
        M: ModelSummary + fmt::Debug,
    {
        trace!("{}  model summary: {}", prefix, ModelLogAdapter(model));

        if cfg!(feature = "verbose_debug") {
            trace!("{}  model: {:?}", prefix, model);
        }
    }
}
