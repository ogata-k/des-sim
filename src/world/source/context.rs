use crate::execution::scheduler::EventScheduler;
use crate::primitive::time::{Duration, SimTime};
use crate::world::event::Priority;

pub struct SourceContext<'a, E> {
    now: SimTime,
    event_scheduler: &'a mut EventScheduler<E>,
}

impl<'a, E> SourceContext<'a, E> {
    pub(crate) fn new(now: SimTime, scheduler: &'a mut EventScheduler<E>) -> SourceContext<'a, E> {
        SourceContext {
            now,
            event_scheduler: scheduler,
        }
    }

    pub fn now(&self) -> SimTime {
        self.now
    }

    pub fn schedule_event(&mut self, delay: Duration, priority: Priority, event_payload: E) {
        self.event_scheduler
            .schedule(self.now, delay, priority, event_payload);
    }
}
