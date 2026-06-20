use crate::execution::engine::handler::MicroStepHandler;
use crate::execution::engine::{EventContext, SourceContext};
use crate::world::context::UserContext;
use crate::world::event::Event;
use crate::world::hook::Hook;
use crate::world::model::Model;

pub struct EventPhase<E, M: Model<E>> {
    context: EventContext<E, M, SourceContext<E, M>>,
    ready_events: Vec<Event<E>>,
}

impl<E, M: Model<E>> EventPhase<E, M> {
    pub(crate) fn new(
        context: EventContext<E, M, SourceContext<E, M>>,
        ready_events: Vec<Event<E>>,
    ) -> Self {
        EventPhase {
            context,
            ready_events,
        }
    }

    pub fn get_context(&mut self) -> &mut EventContext<E, M, SourceContext<E, M>> {
        &mut self.context
    }

    pub fn finish_event_phase(self) -> MicroStepHandler<EventContext<E, M, SourceContext<E, M>>> {
        MicroStepHandler::new(self.context)
    }

    pub fn take_one(&mut self) -> Option<Event<E>> {
        if self.ready_events.is_empty() {
            None
        } else {
            Some(self.ready_events.remove(0))
        }
    }

    pub fn handle_event(&mut self, model: &mut M, event: Event<E>) {
        self.context.hook().before_event(
            self.context.current_tick(),
            self.context.current_micro_step(),
            &event,
        );
        model.handle_event(&mut self.get_context(), &event);
        self.context.hook().after_event(
            self.context.current_tick(),
            self.context.current_micro_step(),
            &event,
        );
    }

    // @todo イベントをまとめて処理できるやつ

    pub fn discard(&mut self, event: Event<E>) {
        self.context.hook().discard_event(
            self.context.current_tick(),
            self.context.current_micro_step(),
            &event,
        );
    }
}
