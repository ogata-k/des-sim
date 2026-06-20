use crate::modeling::event::Event;
use crate::modeling::hook::Hook;
use crate::modeling::hook::instance::SharedHook;
use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::source_handler::{SourceReadyEntry, SourceView};

pub(crate) struct HookDelegate<E> {
    hooks: Vec<Box<dyn Hook<E>>>,
}

impl<E> Default for HookDelegate<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> Hook<E> for HookDelegate<E> {
    // Simulation lifecycle

    fn before_simulation(&self) {
        self.delegate(|hook| hook.before_simulation())
    }

    /// skipped_duration は前回Tickから今回Tickまでに
    /// スキップされた時間。
    ///
    /// スキップ無効Runnerの場合は常に Duration::zero()。
    fn after_simulation(&self, end_sim_time: SimTime) {
        self.reverse_delegate(|hook| hook.after_simulation(end_sim_time))
    }

    // Tick lifecycle

    /// skipped_duration は前回Tickから今回Tickまでに
    /// スキップされた時間。
    ///
    /// スキップ無効Runnerの場合は常に Duration::zero()。
    fn before_tick(&self, now: SimTime, skipped_duration: Duration) {
        self.delegate(|hook| hook.before_tick(now, skipped_duration))
    }

    /// micro_step_count はこのTickで実行された
    /// micro_step数。
    fn after_tick(&self, now: SimTime, micro_step_count: MicroStep) {
        self.reverse_delegate(|hook| hook.after_tick(now, micro_step_count))
    }

    fn before_micro_step(&self, now: SimTime, micro_step: MicroStep) {
        self.delegate(|hook| hook.before_micro_step(now, micro_step))
    }

    fn after_micro_step(&self, now: SimTime, micro_step: MicroStep) {
        self.reverse_delegate(|hook| hook.after_micro_step(now, micro_step))
    }

    fn on_discard_remain_micro_step(
        &self,
        now: SimTime,
        first_discarded_micro_step: MicroStep,
        discarded_sources: &[SourceReadyEntry],
        discarded_events: &[Event<E>],
    ) {
        self.reverse_delegate(|hook| {
            hook.on_discard_remain_micro_step(
                now,
                first_discarded_micro_step,
                discarded_sources,
                discarded_events,
            )
        })
    }

    // Source lifecycle

    fn before_source_phase(&self, now: SimTime, micro_step: MicroStep) {
        self.delegate(|hook| hook.before_source_phase(now, micro_step))
    }

    fn before_source(&self, now: SimTime, micro_step: MicroStep, source: &SourceView) {
        self.delegate(|hook| hook.before_source(now, micro_step, source))
    }

    fn after_source(
        &self,
        now: SimTime,
        micro_step: MicroStep,
        source: &SourceView,
        computed_next_fire: Option<SimTime>,
    ) {
        self.reverse_delegate(|hook| hook.after_source(now, micro_step, source, computed_next_fire))
    }

    fn discard_source(&self, now: SimTime, micro_step: MicroStep, source_view: &SourceView) {
        self.delegate(|hook| hook.discard_source(now, micro_step, source_view))
    }

    fn after_source_phase(&self, now: SimTime, micro_step: MicroStep) {
        self.delegate(|hook| hook.after_source_phase(now, micro_step))
    }

    // Event lifecycle

    fn before_event_phase(&self, now: SimTime, micro_step: MicroStep) {
        self.delegate(|hook| hook.before_event_phase(now, micro_step))
    }

    fn before_event(&self, now: SimTime, micro_step: MicroStep, event: &Event<E>) {
        self.delegate(|hook| hook.before_event(now, micro_step, event))
    }

    fn after_event(&self, now: SimTime, micro_step: MicroStep, event: &Event<E>) {
        self.reverse_delegate(|hook| hook.after_event(now, micro_step, event))
    }

    fn cancel_event(
        &self,
        now: SimTime,
        micro_step: MicroStep,
        scheduled_at: SimTime,
        event: &Event<E>,
    ) {
        self.delegate(|hook| hook.cancel_event(now, micro_step, scheduled_at, event))
    }

    fn discard_event(&self, now: SimTime, micro_step: MicroStep, event: &Event<E>) {
        self.delegate(|hook| hook.discard_event(now, micro_step, event))
    }

    fn after_event_phase(&self, now: SimTime, micro_step: MicroStep) {
        self.delegate(|hook| hook.after_event_phase(now, micro_step))
    }
}

impl<E> HookDelegate<E> {
    pub(crate) fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    pub(crate) fn add_hook<H>(&mut self, hook: H)
    where
        H: Hook<E> + 'static,
    {
        self.hooks.push(Box::new(hook));
    }

    pub(crate) fn add_shared_hook<H>(&mut self, shared_hook: SharedHook<E, H>)
    where
        E: Sync + Send + 'static,
        H: Hook<E> + Sync + Send + 'static,
    {
        self.hooks.push(Box::new(shared_hook));
    }

    fn delegate<F, R>(&self, f: F)
    where
        F: Fn(&dyn Hook<E>) -> R,
    {
        for hook in self.hooks.iter() {
            f(hook.as_ref());
        }
    }

    fn reverse_delegate<F, R>(&self, f: F)
    where
        F: Fn(&dyn Hook<E>) -> R,
    {
        for hook in self.hooks.iter().rev() {
            f(hook.as_ref());
        }
    }
}
