use crate::context::{EventContext, UserContext};
use crate::execution::phase::MicroStepHandler;
use crate::modeling::event::Event;
use crate::modeling::hook::Hook;
use crate::modeling::model::Model;
use std::collections::VecDeque;

pub struct EventPhase<E, M: Model<E>> {
    context: EventContext<E, M>,
    ready_events: VecDeque<Event<E>>,
}

impl<E, M: Model<E>> EventPhase<E, M> {
    pub(crate) fn new(context: EventContext<E, M>, ready_events: VecDeque<Event<E>>) -> Self {
        EventPhase {
            context,
            ready_events,
        }
    }

    pub fn get_context(&mut self) -> &mut EventContext<E, M> {
        &mut self.context
    }

    pub fn complete_event_phase(self, model: &M) -> MicroStepHandler<EventContext<E, M>> {
        self.context.hook().after_event_phase(
            model,
            self.context.current_tick(),
            self.context.current_micro_step(),
        );

        MicroStepHandler::new(self.context)
    }

    pub fn take_one(&mut self) -> Option<Event<E>> {
        self.ready_events.pop_front()
    }

    pub fn handle_event(&mut self, model: &mut M, event: Event<E>) {
        self.context.hook().before_event(
            model,
            self.context.current_tick(),
            self.context.current_micro_step(),
            &event,
        );
        model.handle_event(self.get_context(), &event);
        self.context.hook().after_event(
            model,
            self.context.current_tick(),
            self.context.current_micro_step(),
            &event,
        );
    }

    // @todo イベントをまとめて処理できるやつ

    pub fn discard(&mut self, model: &M, event: Event<E>) {
        self.context.hook().discard_event(
            model,
            self.context.current_tick(),
            self.context.current_micro_step(),
            &event,
        );
    }
}
