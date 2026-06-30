use crate::context::UserContext;
use crate::event_scheduler::EventScheduler;
use crate::modeling::event::{Event, EventPriority};
use crate::modeling::hook::Hook;
use crate::modeling::hook::instance::HookDelegate;
use crate::modeling::model::Model;
use crate::modeling::source::Source;
use crate::primitive::time::{Duration, MicroStep, MicroStepStatus, SimTime, TickStatus};
use crate::source_handler::SourceHandler;

pub struct EventContext<E, M: Model<E>> {
    pub(crate) current_tick_status: TickStatus,
    pub(crate) current_micro_step_status: MicroStepStatus,
    pub(crate) hook_delegate: HookDelegate<E, M>,
    pub(crate) source_handler: SourceHandler<E, M>,
    pub(crate) event_scheduler: EventScheduler<E>,
}

impl<E, M: Model<E>> UserContext<E, M> for EventContext<E, M> {
    fn current_tick(&self) -> SimTime {
        self.current_tick_status.current()
    }

    fn current_micro_step(&self) -> MicroStep {
        self.current_micro_step_status.current()
    }

    fn schedule_event(&mut self, delay: Duration, priority: EventPriority, event_payload: E) {
        self.event_scheduler
            .schedule(self.current_tick(), delay, priority, event_payload);
    }

    fn cancel_scheduled_events<F>(&mut self, model: &M, pred: F) -> Vec<(SimTime, Event<E>)>
    where
        F: FnMut(SimTime, &Event<E>) -> bool,
    {
        let mut result = Vec::new();
        let now = self.current_tick();
        let micro_step = self.current_micro_step();
        let canceled = self.event_scheduler.drain_cancel_events(pred);
        canceled.into_iter().for_each(|(scheduled_at, event)| {
            self.hook()
                .cancel_event(model, now, micro_step, scheduled_at, &event);
            result.push((scheduled_at, event));
        });

        result
    }
}

impl<E, M: Model<E>> EventContext<E, M> {
    pub fn hook(&self) -> &impl Hook<E, M> {
        &self.hook_delegate
    }

    pub fn add_source_after<S>(&mut self, name: &'static str, delay: Duration, source: S)
    where
        S: Source<E, M> + 'static,
    {
        self.source_handler
            .add_source_after(name, self.current_tick(), delay, source);
    }

    pub fn add_source_at_now<S>(&mut self, name: &'static str, source: S)
    where
        S: Source<E, M> + 'static,
    {
        self.source_handler
            .add_source_at_now(name, self.current_tick(), source);
    }
}
