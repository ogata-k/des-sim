use crate::modeling::event::Event;
use crate::modeling::hook::Hook;
use crate::modeling::model::Model;
use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::source_handler::{SourceReadyEntry, SourceView};
use std::marker::PhantomData;
use std::rc::Rc;

pub struct SharedHook<E, M: Model<E>, H: Hook<E, M>> {
    inner: Rc<H>,
    _event: PhantomData<E>,
    _model: PhantomData<M>,
}

impl<E, M: Model<E>, H: Hook<E, M>> Clone for SharedHook<E, M, H> {
    fn clone(&self) -> Self {
        SharedHook {
            inner: Rc::clone(&self.inner),
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

    fn after_simulation(&self, model: &M, end_tick: SimTime) {
        self.inner.as_ref().after_simulation(model, end_tick)
    }

    // Tick lifecycle

    /// skipped_duration は前回Tickから今回Tickまでに
    /// スキップされた時間。
    ///
    /// スキップ無効Runnerの場合は常に Duration::zero()。
    fn before_tick(&self, model: &M, current_tick: SimTime, skipped_duration: Duration) {
        self.inner
            .as_ref()
            .before_tick(model, current_tick, skipped_duration)
    }

    fn after_tick(&self, model: &M, current_tick: SimTime, last_micro_step: MicroStep) {
        self.inner
            .as_ref()
            .after_tick(model, current_tick, last_micro_step)
    }

    fn before_micro_step(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep) {
        self.inner
            .as_ref()
            .before_micro_step(model, current_tick, current_micro_step)
    }

    fn after_micro_step(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep) {
        self.inner
            .as_ref()
            .after_micro_step(model, current_tick, current_micro_step)
    }

    fn on_discard_remain_micro_step(
        &self,
        model: &M,
        current_tick: SimTime,
        first_discarded_micro_step: MicroStep,
        discarded_sources: &[SourceReadyEntry],
        discarded_events: &[Event<E>],
    ) {
        self.inner.as_ref().on_discard_remain_micro_step(
            model,
            current_tick,
            first_discarded_micro_step,
            discarded_sources,
            discarded_events,
        )
    }

    // Source lifecycle

    fn before_register_source(&self, model: &M, name: &str) {
        self.inner.as_ref().before_register_source(model, name)
    }

    fn after_register_source(&self, model: &M, name: &str) {
        self.inner.as_ref().after_register_source(model, name)
    }

    fn before_source_phase(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep) {
        self.inner
            .as_ref()
            .before_source_phase(model, current_tick, current_micro_step)
    }

    fn before_source(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        source_view: &SourceView,
    ) {
        self.inner
            .as_ref()
            .before_source(model, current_tick, current_micro_step, source_view)
    }

    fn after_source(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        source_view: &SourceView,
        computed_next_fire: Option<SimTime>,
    ) {
        self.inner.as_ref().after_source(
            model,
            current_tick,
            current_micro_step,
            source_view,
            computed_next_fire,
        )
    }

    fn cancel_source(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        scheduled_at: SimTime,
        source_view: &SourceView,
    ) {
        self.inner.as_ref().cancel_source(
            model,
            current_tick,
            current_micro_step,
            scheduled_at,
            source_view,
        )
    }

    fn discard_source(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        source_view: &SourceView,
    ) {
        self.inner
            .as_ref()
            .discard_source(model, current_tick, current_micro_step, source_view)
    }

    fn after_source_phase(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep) {
        self.inner
            .as_ref()
            .after_source_phase(model, current_tick, current_micro_step)
    }

    // Event lifecycle

    fn before_event_phase(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep) {
        self.inner
            .as_ref()
            .before_event_phase(model, current_tick, current_micro_step)
    }

    fn before_event(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        event: &Event<E>,
    ) {
        self.inner
            .as_ref()
            .before_event(model, current_tick, current_micro_step, event)
    }

    fn after_event(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        event: &Event<E>,
    ) {
        self.inner
            .as_ref()
            .after_event(model, current_tick, current_micro_step, event)
    }

    fn cancel_event(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        scheduled_at: SimTime,
        event: &Event<E>,
    ) {
        self.inner.as_ref().cancel_event(
            model,
            current_tick,
            current_micro_step,
            scheduled_at,
            event,
        )
    }

    fn discard_event(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        event: &Event<E>,
    ) {
        self.inner
            .as_ref()
            .discard_event(model, current_tick, current_micro_step, event)
    }

    fn after_event_phase(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep) {
        self.inner
            .as_ref()
            .after_event_phase(model, current_tick, current_micro_step)
    }
}

impl<E, M: Model<E>, H: Hook<E, M>> SharedHook<E, M, H> {
    pub fn new(hook: H) -> Self {
        Self {
            inner: Rc::new(hook),
            _event: PhantomData,
            _model: PhantomData,
        }
    }

    pub fn get_ref(&self) -> &H {
        &self.inner
    }
}
