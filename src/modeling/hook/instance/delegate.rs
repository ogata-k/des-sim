use crate::modeling::event::Event;
use crate::modeling::hook::Hook;
use crate::modeling::hook::instance::SharedHook;
use crate::modeling::model::Model;
use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::source_handler::{SourceReadyEntry, SourceView};

pub(crate) struct HookDelegate<E, M: Model<E>> {
    hooks: Vec<Box<dyn Hook<E, M>>>,
}

impl<E, M: Model<E>> Default for HookDelegate<E, M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E, M: Model<E>> Hook<E, M> for HookDelegate<E, M> {
    // Simulation lifecycle

    fn before_simulation(&self, model: &M) {
        self.delegate(|hook| hook.before_simulation(model))
    }

    /// skipped_duration は前回Tickから今回Tickまでに
    /// スキップされた時間。
    ///
    /// スキップ無効Runnerの場合は常に Duration::zero()。
    fn after_simulation(&self, model: &M, end_sim_time: SimTime) {
        self.reverse_delegate(|hook| hook.after_simulation(model, end_sim_time))
    }

    // Tick lifecycle

    /// skipped_duration は前回Tickから今回Tickまでに
    /// スキップされた時間。
    ///
    /// スキップ無効Runnerの場合は常に Duration::zero()。
    fn before_tick(&self, model: &M, now: SimTime, skipped_duration: Duration) {
        self.delegate(|hook| hook.before_tick(model, now, skipped_duration))
    }

    /// micro_step_count はこのTickで実行された
    /// micro_step数。
    fn after_tick(&self, model: &M, now: SimTime, micro_step_count: MicroStep) {
        self.reverse_delegate(|hook| hook.after_tick(model, now, micro_step_count))
    }

    fn before_micro_step(&self, model: &M, now: SimTime, micro_step: MicroStep) {
        self.delegate(|hook| hook.before_micro_step(model, now, micro_step))
    }

    fn after_micro_step(&self, model: &M, now: SimTime, micro_step: MicroStep) {
        self.reverse_delegate(|hook| hook.after_micro_step(model, now, micro_step))
    }

    fn on_discard_remain_micro_step(
        &self,
        model: &M,
        now: SimTime,
        first_discarded_micro_step: MicroStep,
        discarded_sources: &[SourceReadyEntry],
        discarded_events: &[Event<E>],
    ) {
        self.reverse_delegate(|hook| {
            hook.on_discard_remain_micro_step(
                model,
                now,
                first_discarded_micro_step,
                discarded_sources,
                discarded_events,
            )
        })
    }

    // Source lifecycle

    fn before_source_phase(&self, model: &M, now: SimTime, micro_step: MicroStep) {
        self.delegate(|hook| hook.before_source_phase(model, now, micro_step))
    }

    fn before_source(&self, model: &M, now: SimTime, micro_step: MicroStep, source: &SourceView) {
        self.delegate(|hook| hook.before_source(model, now, micro_step, source))
    }

    fn after_source(
        &self,
        model: &M,
        now: SimTime,
        micro_step: MicroStep,
        source: &SourceView,
        computed_next_fire: Option<SimTime>,
    ) {
        self.reverse_delegate(|hook| {
            hook.after_source(model, now, micro_step, source, computed_next_fire)
        })
    }

    fn discard_source(
        &self,
        model: &M,
        now: SimTime,
        micro_step: MicroStep,
        source_view: &SourceView,
    ) {
        self.delegate(|hook| hook.discard_source(model, now, micro_step, source_view))
    }

    fn after_source_phase(&self, model: &M, now: SimTime, micro_step: MicroStep) {
        self.delegate(|hook| hook.after_source_phase(model, now, micro_step))
    }

    // Event lifecycle

    fn before_event_phase(&self, model: &M, now: SimTime, micro_step: MicroStep) {
        self.delegate(|hook| hook.before_event_phase(model, now, micro_step))
    }

    fn before_event(&self, model: &M, now: SimTime, micro_step: MicroStep, event: &Event<E>) {
        self.delegate(|hook| hook.before_event(model, now, micro_step, event))
    }

    fn after_event(&self, model: &M, now: SimTime, micro_step: MicroStep, event: &Event<E>) {
        self.reverse_delegate(|hook| hook.after_event(model, now, micro_step, event))
    }

    fn cancel_event(
        &self,
        model: &M,
        now: SimTime,
        micro_step: MicroStep,
        scheduled_at: SimTime,
        event: &Event<E>,
    ) {
        self.delegate(|hook| hook.cancel_event(model, now, micro_step, scheduled_at, event))
    }

    fn discard_event(&self, model: &M, now: SimTime, micro_step: MicroStep, event: &Event<E>) {
        self.delegate(|hook| hook.discard_event(model, now, micro_step, event))
    }

    fn after_event_phase(&self, model: &M, now: SimTime, micro_step: MicroStep) {
        self.delegate(|hook| hook.after_event_phase(model, now, micro_step))
    }
}

impl<E, M: Model<E>> HookDelegate<E, M> {
    pub(crate) fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    pub(crate) fn add_hook<H>(&mut self, hook: H)
    where
        H: Hook<E, M> + 'static,
    {
        self.hooks.push(Box::new(hook));
    }

    pub(crate) fn add_shared_hook<H>(&mut self, shared_hook: SharedHook<E, M, H>)
    where
        E: 'static,
        M: 'static,
        H: Hook<E, M> + 'static,
    {
        self.hooks.push(Box::new(shared_hook));
    }

    fn delegate<F, R>(&self, f: F)
    where
        F: Fn(&dyn Hook<E, M>) -> R,
    {
        for hook in self.hooks.iter() {
            f(hook.as_ref());
        }
    }

    fn reverse_delegate<F, R>(&self, f: F)
    where
        F: Fn(&dyn Hook<E, M>) -> R,
    {
        for hook in self.hooks.iter().rev() {
            f(hook.as_ref());
        }
    }
}
