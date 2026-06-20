use crate::modeling::event::Event;
use crate::modeling::hook::Hook;
use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::source_handler::{SourceReadyEntry, SourceView};
use std::marker::PhantomData;
use std::sync::Arc;

pub struct SharedHook<E, H: Hook<E>> {
    _event: PhantomData<E>,
    inner: Arc<H>,
}

impl<E, H> Hook<E> for SharedHook<E, H>
where
    H: Hook<E>,
{
    // Simulation lifecycle

    fn before_simulation(&self) {
        self.inner.as_ref().before_simulation()
    }

    /// skipped_duration は前回Tickから今回Tickまでに
    /// スキップされた時間。
    ///
    /// スキップ無効Runnerの場合は常に Duration::zero()。
    fn after_simulation(&self, end_sim_time: SimTime) {
        self.inner.as_ref().after_simulation(end_sim_time)
    }

    // Tick lifecycle

    /// skipped_duration は前回Tickから今回Tickまでに
    /// スキップされた時間。
    ///
    /// スキップ無効Runnerの場合は常に Duration::zero()。
    fn before_tick(&self, now: SimTime, skipped_duration: Duration) {
        self.inner.as_ref().before_tick(now, skipped_duration)
    }

    /// micro_step_count はこのTickで実行された
    /// micro_step数。
    fn after_tick(&self, now: SimTime, micro_step_count: MicroStep) {
        self.inner.as_ref().after_tick(now, micro_step_count)
    }

    fn before_micro_step(&self, now: SimTime, micro_step: MicroStep) {
        self.inner.as_ref().before_micro_step(now, micro_step)
    }

    fn after_micro_step(&self, now: SimTime, micro_step: MicroStep) {
        self.inner.as_ref().after_micro_step(now, micro_step)
    }

    fn on_discard_remain_micro_step(
        &self,
        now: SimTime,
        first_discarded_micro_step: MicroStep,
        discarded_sources: &[SourceReadyEntry],
        discarded_events: &[Event<E>],
    ) {
        self.inner.as_ref().on_discard_remain_micro_step(
            now,
            first_discarded_micro_step,
            discarded_sources,
            discarded_events,
        )
    }

    // Source lifecycle

    fn before_source_phase(&self, now: SimTime, micro_step: MicroStep) {
        self.inner.as_ref().before_source_phase(now, micro_step)
    }

    fn before_source(&self, now: SimTime, micro_step: MicroStep, source_view: &SourceView) {
        self.inner
            .as_ref()
            .before_source(now, micro_step, source_view)
    }

    fn after_source(
        &self,
        now: SimTime,
        micro_step: MicroStep,
        source_view: &SourceView,
        computed_next_fire: Option<SimTime>,
    ) {
        self.inner
            .as_ref()
            .after_source(now, micro_step, source_view, computed_next_fire)
    }

    fn discard_source(&self, now: SimTime, micro_step: MicroStep, source_view: &SourceView) {
        self.inner
            .as_ref()
            .discard_source(now, micro_step, source_view)
    }

    fn after_source_phase(&self, now: SimTime, micro_step: MicroStep) {
        self.inner.as_ref().after_source_phase(now, micro_step)
    }

    // Event lifecycle

    fn before_event_phase(&self, now: SimTime, micro_step: MicroStep) {
        self.inner.as_ref().before_event_phase(now, micro_step)
    }

    fn before_event(&self, now: SimTime, micro_step: MicroStep, event: &Event<E>) {
        self.inner.as_ref().before_event(now, micro_step, event)
    }

    fn after_event(&self, now: SimTime, micro_step: MicroStep, event: &Event<E>) {
        self.inner.as_ref().after_event(now, micro_step, event)
    }

    fn cancel_event(
        &self,
        now: SimTime,
        micro_step: MicroStep,
        scheduled_at: SimTime,
        event: &Event<E>,
    ) {
        self.inner
            .as_ref()
            .cancel_event(now, micro_step, scheduled_at, event)
    }

    fn discard_event(&self, now: SimTime, micro_step: MicroStep, event: &Event<E>) {
        self.inner.as_ref().discard_event(now, micro_step, event)
    }

    fn after_event_phase(&self, now: SimTime, micro_step: MicroStep) {
        self.inner.as_ref().after_event_phase(now, micro_step)
    }
}

impl<E, H> SharedHook<E, H>
where
    H: Hook<E>,
    E: Send + Sync,
{
    pub fn new(hook: H) -> Self {
        Self {
            _event: Default::default(),
            inner: Arc::new(hook),
        }
    }
}
