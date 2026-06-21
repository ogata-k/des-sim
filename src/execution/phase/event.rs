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

    /// 全体から条件を満たす一件を取得して取り出す。
    pub fn take_one_if<F>(&mut self, predicate: F) -> Option<Event<E>>
    where
        F: FnOnce(&Event<E>) -> bool,
    {
        self.ready_events.pop_front_if(|e| predicate(e))
    }

    /// 先頭が条件を満たす時だけ取り出す。
    pub fn take_front_if<F>(&mut self, predicate: F) -> Option<Event<E>>
    where
        F: FnOnce(&Event<E>) -> bool,
    {
        // 先頭要素を覗いて、条件に合致するか判定
        if self.ready_events.front().map_or(false, predicate) {
            self.ready_events.pop_front()
        } else {
            None
        }
    }

    pub fn take_all(&mut self) -> VecDeque<Event<E>> {
        std::mem::take(&mut self.ready_events)
    }

    pub fn take_all_if<F>(&mut self, predicate: F) -> VecDeque<Event<E>>
    where
        F: FnMut(&Event<E>) -> bool,
    {
        // 一時的にすべて取得して抽出して差し替える
        let all_sources = std::mem::take(&mut self.ready_events);

        let (taken, remaining): (VecDeque<_>, VecDeque<_>) =
            all_sources.into_iter().partition(predicate);

        self.ready_events = remaining;

        taken
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

    pub fn discard(&mut self, model: &M, event: Event<E>) {
        self.context.hook().discard_event(
            model,
            self.context.current_tick(),
            self.context.current_micro_step(),
            &event,
        );
    }
}
