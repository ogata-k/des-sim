use crate::primitive::time::Duration;
use crate::world::context::UserContext;
use crate::world::event::Priority;

pub trait SourceContext<E>: UserContext<E> {
    fn schedule_event(&mut self, delay: Duration, priority: Priority, event_payload: E);
}
