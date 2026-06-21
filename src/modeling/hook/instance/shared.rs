use crate::modeling::event::Event;
use crate::modeling::hook::Hook;
use crate::modeling::model::Model;
use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::source_handler::{SourceReadyEntry, SourceView};
use std::marker::PhantomData;
use std::sync::Arc;

pub struct SharedHook<E, M: Model<E>, H: Hook<E, M>> {
    inner: Arc<H>,
    _event: PhantomData<E>,
    _model: PhantomData<M>,
}

impl<E, M: Model<E>, H: Hook<E, M>> Clone for SharedHook<E, M, H> {
    fn clone(&self) -> Self {
        SharedHook {
            inner: Arc::clone(&self.inner),
            _event: PhantomData,
            _model: PhantomData,
        }
    }
}

impl<E, M: Model<E>, H> Hook<E, M> for SharedHook<E, M, H>
where
    H: Hook<E, M>,
{
    // Simulation lifecycle

    fn before_simulation(&self, model: &M) {
        self.inner.as_ref().before_simulation(model)
    }

    /// skipped_duration は前回Tickから今回Tickまでに
    /// スキップされた時間。
    ///
    /// スキップ無効Runnerの場合は常に Duration::zero()。
    fn after_simulation(&self, model: &M, end_sim_time: SimTime) {
        self.inner.as_ref().after_simulation(model, end_sim_time)
    }

    // Tick lifecycle

    /// skipped_duration は前回Tickから今回Tickまでに
    /// スキップされた時間。
    ///
    /// スキップ無効Runnerの場合は常に Duration::zero()。
    fn before_tick(&self, model: &M, now: SimTime, skipped_duration: Duration) {
        self.inner
            .as_ref()
            .before_tick(model, now, skipped_duration)
    }

    /// micro_step_count はこのTickで実行された
    /// micro_step数。
    fn after_tick(&self, model: &M, now: SimTime, micro_step_count: MicroStep) {
        self.inner.as_ref().after_tick(model, now, micro_step_count)
    }

    fn before_micro_step(&self, model: &M, now: SimTime, micro_step: MicroStep) {
        self.inner
            .as_ref()
            .before_micro_step(model, now, micro_step)
    }

    fn after_micro_step(&self, model: &M, now: SimTime, micro_step: MicroStep) {
        self.inner.as_ref().after_micro_step(model, now, micro_step)
    }

    fn on_discard_remain_micro_step(
        &self,
        model: &M,
        now: SimTime,
        first_discarded_micro_step: MicroStep,
        discarded_sources: &[SourceReadyEntry],
        discarded_events: &[Event<E>],
    ) {
        self.inner.as_ref().on_discard_remain_micro_step(
            model,
            now,
            first_discarded_micro_step,
            discarded_sources,
            discarded_events,
        )
    }

    // Source lifecycle

    fn before_source_phase(&self, model: &M, now: SimTime, micro_step: MicroStep) {
        self.inner
            .as_ref()
            .before_source_phase(model, now, micro_step)
    }

    fn before_source(
        &self,
        model: &M,
        now: SimTime,
        micro_step: MicroStep,
        source_view: &SourceView,
    ) {
        self.inner
            .as_ref()
            .before_source(model, now, micro_step, source_view)
    }

    fn after_source(
        &self,
        model: &M,
        now: SimTime,
        micro_step: MicroStep,
        source_view: &SourceView,
        computed_next_fire: Option<SimTime>,
    ) {
        self.inner
            .as_ref()
            .after_source(model, now, micro_step, source_view, computed_next_fire)
    }

    fn discard_source(
        &self,
        model: &M,
        now: SimTime,
        micro_step: MicroStep,
        source_view: &SourceView,
    ) {
        self.inner
            .as_ref()
            .discard_source(model, now, micro_step, source_view)
    }

    fn after_source_phase(&self, model: &M, now: SimTime, micro_step: MicroStep) {
        self.inner
            .as_ref()
            .after_source_phase(model, now, micro_step)
    }

    // Event lifecycle

    fn before_event_phase(&self, model: &M, now: SimTime, micro_step: MicroStep) {
        self.inner
            .as_ref()
            .before_event_phase(model, now, micro_step)
    }

    fn before_event(&self, model: &M, now: SimTime, micro_step: MicroStep, event: &Event<E>) {
        self.inner
            .as_ref()
            .before_event(model, now, micro_step, event)
    }

    fn after_event(&self, model: &M, now: SimTime, micro_step: MicroStep, event: &Event<E>) {
        self.inner
            .as_ref()
            .after_event(model, now, micro_step, event)
    }

    fn cancel_event(
        &self,
        model: &M,
        now: SimTime,
        micro_step: MicroStep,
        scheduled_at: SimTime,
        event: &Event<E>,
    ) {
        self.inner
            .as_ref()
            .cancel_event(model, now, micro_step, scheduled_at, event)
    }

    fn discard_event(&self, model: &M, now: SimTime, micro_step: MicroStep, event: &Event<E>) {
        self.inner
            .as_ref()
            .discard_event(model, now, micro_step, event)
    }

    fn after_event_phase(&self, model: &M, now: SimTime, micro_step: MicroStep) {
        self.inner
            .as_ref()
            .after_event_phase(model, now, micro_step)
    }
}

impl<E, M: Model<E>, H> SharedHook<E, M, H>
where
    H: Hook<E, M>,
    E: Send + Sync,
{
    pub fn new(hook: H) -> Self {
        Self {
            inner: Arc::new(hook),
            _event: PhantomData,
            _model: PhantomData,
        }
    }

    pub fn get_ref(&self) -> &H {
        &self.inner
    }
}
