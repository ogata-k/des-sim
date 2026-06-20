use crate::execution::engine::{EventContext, SourceContext};
use crate::world::event::Event;

pub trait Model<E>: Sized {
    // @todo executionに依存してしまっている
    fn handle_event(
        &mut self,
        context: &mut EventContext<E, Self, SourceContext<E, Self>>,
        event: &Event<E>,
    );
}
