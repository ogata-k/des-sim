use crate::modeling::event::Event;
use crate::modeling::hook::Hook;
use crate::modeling::model::Model;
use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::source_handler::{SourceReadyEntry, SourceView};
use log::{debug, info, trace};
use std::fmt;

pub trait ModelSummary {
    fn summary(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
}

struct ModelLogAdapter<'a, M>(&'a M);

impl<'a, M: ModelSummary> fmt::Display for ModelLogAdapter<'a, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.summary(f)
    }
}

pub struct TraceHook;

impl<E, M> Hook<E, M> for TraceHook
where
    E: fmt::Debug,
    M: Model<E> + ModelSummary + fmt::Debug,
{
    fn before_simulation(&self, model: &M) {
        info!("--- [SIMULATION START] Time: {:?} ---", SimTime::zero());
        self.debug_log_model(model);
    }

    fn after_simulation(&self, model: &M, end_sim_time: SimTime) {
        info!("--- [SIMULATION END] Time: {:?} ---", end_sim_time);
        self.debug_log_model(model);
    }

    fn before_tick(&self, model: &M, now: SimTime, skipped_duration: Duration) {
        info!(
            ">>> Tick at {:?} (skipped: {:?} ticks)",
            now, skipped_duration
        );
        self.debug_log_model(model);
    }

    fn after_tick(&self, model: &M, now: SimTime, micro_step_count: MicroStep) {
        info!(
            "<<< Tick at {:?} finished (last μSteps: {})",
            now, micro_step_count
        );
        self.debug_log_model(model);
    }

    fn before_micro_step(&self, model: &M, now: SimTime, micro_step: MicroStep) {
        trace!("  [μStep: {} at {:?}] START", micro_step, now);
        self.trace_log_model(model);
    }

    fn after_micro_step(&self, model: &M, now: SimTime, micro_step: MicroStep) {
        trace!("  [μStep: {} at {:?}] END", micro_step, now);
        self.trace_log_model(model);
    }

    fn on_discard_remain_micro_step(
        &self,
        model: &M,
        now: SimTime,
        first_discarded_micro_step: MicroStep,
        discarded_sources: &[SourceReadyEntry],
        discarded_events: &[Event<E>],
    ) {
        debug!(
            "!!! [DISCARD REMAINS at {:?}] Start μStep: {}, Sources: {:?}, Events: {:?}",
            now, first_discarded_micro_step, discarded_sources, discarded_events
        );
        self.debug_log_model(model);
    }

    fn before_source_phase(&self, model: &M, now: SimTime, micro_step: MicroStep) {
        trace!("    > Source Phase at {:?} (μStep: {})", now, micro_step);
        self.trace_log_model(model);
    }

    fn before_source(
        &self,
        model: &M,
        _now: SimTime,
        _micro_step: MicroStep,
        source_view: &SourceView,
    ) {
        trace!("      + Source firing | Source: {:?}", source_view);
        self.trace_log_model(model);
    }

    fn after_source(
        &self,
        model: &M,
        _now: SimTime,
        _micro_step: MicroStep,
        source_view: &SourceView,
        computed_next_fire: Option<SimTime>,
    ) {
        trace!(
            "      - Source finished (next: after {:?} tick) | Source: {:?}",
            computed_next_fire, source_view
        );
        self.trace_log_model(model);
    }

    fn discard_source(
        &self,
        model: &M,
        now: SimTime,
        micro_step: MicroStep,
        source_view: &SourceView,
    ) {
        debug!(
            "    ! Source discarded at {:?} (last handle μStep: {}) | Source: {:?}",
            now, micro_step, source_view
        );
        self.debug_log_model(model);
    }

    fn after_source_phase(&self, model: &M, now: SimTime, micro_step: MicroStep) {
        trace!(
            "    < Source Phase finished at {:?} (μStep: {})",
            now, micro_step,
        );
        self.trace_log_model(model);
    }

    fn before_event_phase(&self, model: &M, now: SimTime, micro_step: MicroStep) {
        trace!("    > Event Phase at {:?} (μStep: {})", now, micro_step);
        self.trace_log_model(model);
    }

    fn before_event(&self, model: &M, _now: SimTime, _micro_step: MicroStep, event: &Event<E>) {
        trace!("      + Event firing | Event: {:?}", event);
        self.trace_log_model(model);
    }

    fn after_event(&self, model: &M, _now: SimTime, _micro_step: MicroStep, event: &Event<E>) {
        trace!("      - Event finished | Event: {:?}", event);
        self.trace_log_model(model);
    }

    fn cancel_event(
        &self,
        model: &M,
        now: SimTime,
        micro_step: MicroStep,
        scheduled_at: SimTime,
        event: &Event<E>,
    ) {
        debug!(
            "    ! Event canceled at {:?} (current μStep: {}) | Event: {:?} at scheduled: {}",
            now, micro_step, event, scheduled_at
        );
        self.debug_log_model(model);
    }

    fn discard_event(&self, model: &M, now: SimTime, micro_step: MicroStep, event: &Event<E>) {
        debug!(
            "    ! Event canceled at {:?} (last handle μStep: {}) | Event: {:?}",
            now, micro_step, event
        );
        self.debug_log_model(model);
    }

    fn after_event_phase(&self, model: &M, now: SimTime, micro_step: MicroStep) {
        trace!(
            "    < Event Phase finished at {:?} (μStep: {})",
            now, micro_step,
        );
        self.trace_log_model(model);
    }
}

impl TraceHook {
    fn debug_log_model<M>(&self, model: &M)
    where
        M: ModelSummary + fmt::Debug,
    {
        debug!("model summary: {}", ModelLogAdapter(model));

        if cfg!(feature = "verbose_debug") {
            debug!("model: {:?}", model);
        }
    }

    fn trace_log_model<M>(&self, model: &M)
    where
        M: ModelSummary + fmt::Debug,
    {
        trace!("model summary: {}", ModelLogAdapter(model));

        if cfg!(feature = "verbose_debug") {
            trace!("model: {:?}", model);
        }
    }
}
