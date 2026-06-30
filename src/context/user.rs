use crate::modeling::event::{Event, EventPriority};
use crate::modeling::model::Model;
use crate::primitive::time::{Duration, MicroStep, SimTime};

pub trait UserContext<E, M: Model<E>> {
    fn current_tick(&self) -> SimTime;
    fn current_micro_step(&self) -> MicroStep;
    fn schedule_event(&mut self, delay: Duration, priority: EventPriority, event_payload: E);
    fn cancel_scheduled_events<F>(&mut self, model: &M, pred: F) -> Vec<(SimTime, Event<E>)>
    where
        F: FnMut(SimTime, &Event<E>) -> bool;
}
