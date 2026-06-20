use crate::modeling::hook::Hook;
use crate::primitive::time::{MicroStep, SimTime};

pub trait UserContext<E> {
    fn current_tick(&self) -> SimTime;
    fn current_micro_step(&self) -> MicroStep;
    fn hook(&self) -> &impl Hook<E>;
}
