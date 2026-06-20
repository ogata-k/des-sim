use crate::primitive::time::{MicroStep, SimTime};
use crate::world::hook::Hook;

pub trait UserContext<E> {
    fn current_tick(&self) -> SimTime;
    fn current_micro_step(&self) -> MicroStep;
    fn hook(&self) -> &impl Hook<E>;
}
