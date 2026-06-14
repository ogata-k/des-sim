mod hook;
mod shared;

use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::world::event::Event;
use crate::world::source::SourceView;
pub use hook::*;
pub use shared::*;

pub struct HookDelegate<E> {
    hooks: Vec<Box<dyn Hook<E>>>,
}

impl<E> Default for HookDelegate<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> Hook<E> for HookDelegate<E> {
    // Simulation lifecycle

    fn before_simulation(&mut self) {
        self.delegate(|hook| hook.before_simulation())
    }

    /// skipped_duration は前回Tickから今回Tickまでに
    /// スキップされた時間。
    ///
    /// スキップ無効Runnerの場合は常に Duration::zero()。
    fn after_simulation(&mut self, end_sim_time: SimTime, skipped_duration: Duration) {
        self.delegate(|hook| hook.after_simulation(end_sim_time, skipped_duration))
    }

    // Tick lifecycle

    /// skipped_duration は前回Tickから今回Tickまでに
    /// スキップされた時間。
    ///
    /// スキップ無効Runnerの場合は常に Duration::zero()。
    fn before_tick(&mut self, now: SimTime, skipped_duration: Duration) {
        self.delegate(|hook| hook.before_tick(now, skipped_duration))
    }

    /// microstep_count はこのTickで実行された
    /// microstep数。
    fn after_tick(&mut self, now: SimTime, microstep_count: MicroStep) {
        self.delegate(|hook| hook.after_tick(now, microstep_count))
    }

    fn before_microstep(&mut self, now: SimTime, microstep: MicroStep) {
        self.delegate(|hook| hook.before_microstep(now, microstep))
    }

    fn after_microstep(&mut self, now: SimTime, microstep: MicroStep) {
        self.delegate(|hook| hook.after_microstep(now, microstep))
    }

    // Source lifecycle

    fn before_source(&mut self, now: SimTime, microstep: MicroStep, source: &SourceView) {
        self.delegate(|hook| hook.before_source(now, microstep, source))
    }

    fn after_source(
        &mut self,
        now: SimTime,
        microstep: MicroStep,
        source: &SourceView,
        computed_next_fire: Option<SimTime>,
    ) {
        self.delegate(|hook| hook.after_source(now, microstep, source, computed_next_fire))
    }

    // Event lifecycle

    fn before_event(&mut self, now: SimTime, microstep: MicroStep, event: &Event<E>) {
        self.delegate(|hook| hook.before_event(now, microstep, event))
    }

    fn after_event(&mut self, now: SimTime, microstep: MicroStep, event: &Event<E>) {
        self.delegate(|hook| hook.after_event(now, microstep, event))
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

    pub(crate) fn add_shared_hook<H>(&mut self, hook: H) -> SharedHook<H>
    where
        H: Hook<E> + AsInnerSharedHook + 'static,
    {
        let shared = SharedHook::new(hook);

        self.hooks.push(Box::new(shared.clone()));

        shared
    }

    fn delegate<F, R>(&mut self, f: F)
    where
        F: Fn(&mut dyn Hook<E>) -> R,
    {
        for hook in self.hooks.iter_mut() {
            f(hook.as_mut());
        }
    }
}
