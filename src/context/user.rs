//! The `user` module defines the `UserContext` trait, which provides a common interface
//! for models and sources to interact with the simulation environment.
//!
//! This trait allows scheduling events and querying the current simulation time.

use crate::modeling::event::EventPriority;
use crate::modeling::model::Model;
use crate::primitive::time::{Duration, MicroStep, SimTime};

/// Provides context information accessible by models during simulation execution.
///
/// Models use this context to query the current simulation time or to schedule
/// future events.
pub trait UserContext<E, M: Model<E>> {
    /// Returns the current simulation tick (absolute time).
    fn current_tick(&self) -> SimTime;

    /// Returns the current micro-step (the index of the step within the current tick).
    fn current_micro_step(&self) -> MicroStep;

    /// Schedules an event with the specified delay and priority.
    ///
    /// # Arguments
    /// * `delay` - The time elapsed from the current simulation time.
    /// * `priority` - The priority level of the event.
    /// * `event_payload` - The data payload associated with the scheduled event.
    fn schedule_event(&mut self, delay: Duration, priority: EventPriority, event_payload: E);
}
