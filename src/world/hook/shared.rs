use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::world::event::Event;
use crate::world::hook::Hook;
use crate::world::source::SourceView;
use std::sync::{Arc, LockResult, Mutex, MutexGuard};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum SharedHookFailLockKind {
    /// Otherはエンドユーザー用。
    /// それ以外は下記Hookするイベントごとに自動的に割り当てられる。
    Other,
    BeforeSimulation,
    AfterSimulation,
    BeforeTick,
    AfterTick,
    BeforeMicrostep,
    AfterMicrostep,
    BeforeSource,
    AfterSource,
    BeforeEvent,
    AfterEvent,
}

pub trait AsInnerSharedHook {
    /// ArcのLockに失敗したPoison状態から[into_inner()]して可変状態をとってくるので、
    /// 半分自己責任で実装すること。
    fn on_lock_error(&mut self, kind: SharedHookFailLockKind);
}

pub struct SharedHook<H: AsInnerSharedHook> {
    inner: Arc<Mutex<H>>,
}

impl<H: AsInnerSharedHook> Clone for SharedHook<H> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<E, H> Hook<E> for SharedHook<H>
where
    H: Hook<E> + AsInnerSharedHook,
{
    // Simulation lifecycle

    fn before_simulation(&mut self) {
        let _ = self.with_lock(SharedHookFailLockKind::BeforeSimulation, |g| {
            g.before_simulation();
        });
    }

    /// skipped_duration は前回Tickから今回Tickまでに
    /// スキップされた時間。
    ///
    /// スキップ無効Runnerの場合は常に Duration::zero()。
    fn after_simulation(&mut self, end_sim_time: SimTime, skipped_duration: Duration) {
        let _ = self.with_lock(SharedHookFailLockKind::AfterSimulation, |g| {
            g.after_simulation(end_sim_time, skipped_duration);
        });
    }

    // Tick lifecycle

    /// skipped_duration は前回Tickから今回Tickまでに
    /// スキップされた時間。
    ///
    /// スキップ無効Runnerの場合は常に Duration::zero()。
    fn before_tick(&mut self, now: SimTime, skipped_duration: Duration) {
        let _ = self.with_lock(SharedHookFailLockKind::BeforeTick, |g| {
            g.before_tick(now, skipped_duration);
        });
    }

    /// microstep_count はこのTickで実行された
    /// microstep数。
    fn after_tick(&mut self, now: SimTime, microstep_count: MicroStep) {
        let _ = self.with_lock(SharedHookFailLockKind::AfterTick, |g| {
            g.after_tick(now, microstep_count);
        });
    }

    fn before_microstep(&mut self, now: SimTime, microstep: MicroStep) {
        let _ = self.with_lock(SharedHookFailLockKind::BeforeMicrostep, |g| {
            g.before_microstep(now, microstep);
        });
    }

    fn after_microstep(&mut self, now: SimTime, microstep: MicroStep) {
        let _ = self.with_lock(SharedHookFailLockKind::AfterMicrostep, |g| {
            g.after_microstep(now, microstep);
        });
    }

    // Source lifecycle

    fn before_source(&mut self, now: SimTime, microstep: MicroStep, source_view: &SourceView) {
        let _ = self.with_lock(SharedHookFailLockKind::BeforeSource, |g| {
            g.before_source(now, microstep, source_view);
        });
    }

    fn after_source(
        &mut self,
        now: SimTime,
        microstep: MicroStep,
        source_view: &SourceView,
        computed_next_fire: Option<SimTime>,
    ) {
        let _ = self.with_lock(SharedHookFailLockKind::AfterSource, |g| {
            g.after_source(now, microstep, source_view, computed_next_fire);
        });
    }

    // Event lifecycle

    fn before_event(&mut self, now: SimTime, microstep: MicroStep, event: &Event<E>) {
        let _ = self.with_lock(SharedHookFailLockKind::BeforeEvent, |g| {
            g.before_event(now, microstep, event);
        });
    }

    fn after_event(&mut self, now: SimTime, microstep: MicroStep, event: &Event<E>) {
        let _ = self.with_lock(SharedHookFailLockKind::AfterEvent, |g| {
            g.after_event(now, microstep, event);
        });
    }
}

impl<H: AsInnerSharedHook> SharedHook<H> {
    pub(crate) fn new(hook: H) -> Self {
        Self {
            inner: Arc::new(Mutex::new(hook)),
        }
    }

    pub fn lock(&self) -> LockResult<MutexGuard<'_, H>> {
        self.inner.lock()
    }

    pub fn with_lock<F, R>(&self, kind: SharedHookFailLockKind, f: F) -> Result<R, ()>
    where
        F: FnOnce(&mut MutexGuard<'_, H>) -> R,
    {
        match self.inner.lock() {
            Ok(mut guard) => Ok(f(&mut guard)),
            Err(poison) => {
                let mut guard = poison.into_inner();
                guard.on_lock_error(kind);
                Err(())
            }
        }
    }
}
