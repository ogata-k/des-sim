use crate::primitive::time::{Duration, SimTime};
use crate::world::context::{SourceContext, UserContext};
use crate::world::event::{Event, Priority};
use crate::world::model::Model;
use crate::world::source::Source;

pub trait EventContext<E, M: Model<E>, SC: SourceContext<E>>: UserContext<E> {
    fn add_source_after<S>(&mut self, name: String, delay: Duration, source: S)
    where
        S: Source<E, M, SC> + 'static;

    fn add_source_at_now<S>(&mut self, name: String, source: S)
    where
        S: Source<E, M, SC> + 'static;

    fn schedule(&mut self, delay: Duration, priority: Priority, event_payload: E);

    fn cancel_scheduled_events<F>(&mut self, pred: F)
    where
        F: Fn(SimTime, &Event<E>) -> bool;
}
