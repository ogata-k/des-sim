use crate::execution::engine::{ExecutorContext, SourceContext};
use crate::execution::scheduler::EventScheduler;
use crate::execution::utility::TickStatus;
use crate::primitive::time::{Duration, SimTime};
use crate::world::event::Priority;
use crate::world::hook::{Hook, HookDelegate, SharedHook};
use crate::world::model::Model;
use crate::world::source::{Source, SourceHandler};

pub struct Engine<E, M: Model<E>> {
    pub(crate) hook_delegate: HookDelegate<E>,
    pub(crate) source_handler: SourceHandler<E, M, SourceContext<E, M>>,
    pub(crate) event_scheduler: EventScheduler<E>,
}

impl<E, M: Model<E>> Engine<E, M> {
    pub fn new() -> Engine<E, M> {
        Engine {
            hook_delegate: HookDelegate::new(),
            source_handler: SourceHandler::new(),
            event_scheduler: EventScheduler::new(),
        }
    }

    pub fn begin_simulation(mut self) -> ExecutorContext<E, M> {
        self.hook_delegate.before_simulation();
        // Engineで登録したソースを反映させておく
        self.source_handler.flush_pending();
        // Engineで登録したイベントを反映させておく
        self.event_scheduler.flush_pending();

        ExecutorContext {
            tick_status: TickStatus::initialize(),
            current_tick: SimTime::zero(),
            hook_delegate: self.hook_delegate,
            source_handler: self.source_handler,
            event_scheduler: self.event_scheduler,
        }
    }

    pub fn add_hook<H>(&mut self, hook: H) -> &mut Self
    where
        H: Hook<E> + 'static,
    {
        self.hook_delegate.add_hook(hook);
        self
    }

    pub fn add_shared_hook<H>(&mut self, shared_hook: SharedHook<E, H>) -> &mut Self
    where
        E: Sync + Send + 'static,
        H: Hook<E> + Sync + Send + 'static,
    {
        self.hook_delegate.add_shared_hook(shared_hook);
        self
    }

    pub fn add_source<S>(&mut self, name: String, first_fire_time: SimTime, source: S) -> &mut Self
    where
        S: Source<E, M, SourceContext<E, M>> + 'static,
    {
        self.source_handler
            .add_source(name, first_fire_time, source);
        self
    }

    pub fn schedule_event_at(
        &mut self,
        sim_time: SimTime,
        priority: Priority,
        event_payload: E,
    ) -> &mut Self {
        self.event_scheduler
            .schedule(sim_time, Duration::zero(), priority, event_payload);

        self
    }
}
