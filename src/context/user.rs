use crate::modeling::event::EventPriority;
use crate::modeling::model::Model;
use crate::primitive::time::{Duration, MicroStep, SimTime};

pub trait UserContext<E, M: Model<E>> {
    fn current_tick(&self) -> SimTime;
    fn current_micro_step(&self) -> MicroStep;
    fn schedule_event(&mut self, delay: Duration, priority: EventPriority, event_payload: E);
}
