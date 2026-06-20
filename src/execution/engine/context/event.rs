use crate::execution::engine::SourceContext;
use crate::execution::scheduler::EventScheduler;
use crate::execution::utility::{MicroStepStatus, TickStatus};
use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::world::context::SourceContext as SourceContextTrait;
use crate::world::context::{EventContext as EventContextTrait, UserContext};
use crate::world::event::{Event, Priority};
use crate::world::hook::{Hook, HookDelegate};
use crate::world::model::Model;
use crate::world::source::{Source, SourceHandler};

pub struct EventContext<E, M: Model<E>, SC: SourceContextTrait<E>> {
    pub(crate) current_tick_status: TickStatus,
    pub(crate) current_micro_step_status: MicroStepStatus,
    pub(crate) hook_delegate: HookDelegate<E>,
    pub(crate) source_handler: SourceHandler<E, M, SC>,
    pub(crate) event_scheduler: EventScheduler<E>,
}

impl<E, M: Model<E>> UserContext<E> for EventContext<E, M, SourceContext<E, M>> {
    fn current_tick(&self) -> SimTime {
        self.current_tick_status.current()
    }

    fn current_micro_step(&self) -> MicroStep {
        self.current_micro_step_status.current()
    }

    fn hook(&self) -> &impl Hook<E> {
        &self.hook_delegate
    }
}

impl<E, M: Model<E>> EventContextTrait<E, M, SourceContext<E, M>>
    for EventContext<E, M, SourceContext<E, M>>
{
    fn add_source_after<S>(&mut self, name: String, delay: Duration, source: S)
    where
        S: Source<E, M, SourceContext<E, M>> + 'static,
    {
        self.source_handler
            .add_source_after(name, self.current_tick(), delay, source);
    }

    fn add_source_at_now<S>(&mut self, name: String, source: S)
    where
        S: Source<E, M, SourceContext<E, M>> + 'static,
    {
        self.source_handler
            .add_source_at_now(name, self.current_tick(), source);
    }

    fn schedule(&mut self, delay: Duration, priority: Priority, event_payload: E) {
        self.event_scheduler
            .schedule(self.current_tick(), delay, priority, event_payload);
    }

    fn cancel_scheduled_events<F>(&mut self, pred: F)
    where
        F: Fn(SimTime, &Event<E>) -> bool,
    {
        let now = self.current_tick();
        let micro_step = self.current_micro_step();
        let canceled = self.event_scheduler.drain_pending_to_cancel(pred);
        canceled.into_iter().for_each(|(scheduled_at, event)| {
            self.hook()
                .cancel_event(now, micro_step, scheduled_at, &event);
        });
    }
}
