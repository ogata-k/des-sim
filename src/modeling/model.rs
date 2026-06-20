use crate::context::EventContext;
use crate::modeling::event::Event;

pub trait Model<E>: Sized {
    fn handle_event(&mut self, context: &mut EventContext<E, Self>, event: &Event<E>);
}
