use crate::modeling::hook::Hook;
use crate::modeling::model::Model;
use crate::primitive::time::{MicroStep, SimTime};

pub trait UserContext<E, M: Model<E>> {
    fn current_tick(&self) -> SimTime;
    fn current_micro_step(&self) -> MicroStep;
    fn hook(&self) -> &impl Hook<E, M>;
}
