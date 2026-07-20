//! The `model` module defines the `Model` trait, which is the fundamental interface
//! for any simulation model in `des-sim`.
//!
//! Any custom simulation logic must implement this trait to process events
//! and interact with the simulation environment via the provided `EventContext`.

use crate::context::EventContext;
use crate::modeling::event::Event;

/// Defines the interface for a simulation model that processes events.
///
/// Models implementing this trait are responsible for reacting to events
/// triggered within the simulation and updating their internal state accordingly.
pub trait Model<E>: Sized {
    /// Handles an incoming event within the provided simulation context.
    ///
    /// # Arguments
    ///
    /// * `context` - The simulation context, providing access to state, scheduling, and hooks.
    /// * `event` - The event to be processed by this model.
    fn handle_event(&mut self, context: &mut EventContext<E, Self>, event: &Event<E>);
}
