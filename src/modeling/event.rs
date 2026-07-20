//! The `event` module defines the core components for representing events in the simulation.
//!
//! It includes `EventPriority` for ordering events and the `Event` struct itself,
//! which encapsulates a unique ID, priority, and a generic payload for application-specific data.

use crate::primitive::id::EventId;

/// Represents the priority of an event.
/// Higher numerical values indicate higher priority.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct EventPriority(u8);

impl Default for EventPriority {
    fn default() -> Self {
        EventPriority::minimum()
    }
}

impl EventPriority {
    /// Creates a new `EventPriority` with the specified value.
    pub const fn new(v: u8) -> EventPriority {
        EventPriority(v)
    }

    /// Returns the lowest possible event priority.
    pub const fn minimum() -> EventPriority {
        EventPriority(u8::MIN)
    }

    /// Returns the highest possible event priority.
    pub const fn maximum() -> EventPriority {
        EventPriority(u8::MAX)
    }

    /// Returns the raw numerical value of the priority.
    pub const fn value(&self) -> u8 {
        self.0
    }
}

/// Represents a scheduled event within the simulation.
#[derive(Clone, Debug)]
pub struct Event<E> {
    /// Unique identifier for the event.
    pub event_id: EventId,
    /// Priority level that determines the execution order for events at the same simulation time.
    pub priority: EventPriority,
    /// The application-specific data payload associated with this event.
    pub payload: E,
}

impl<E> Event<E> {
    /// Creates a new `Event` instance.
    pub(crate) fn new(event_id: EventId, priority: EventPriority, payload: E) -> Self {
        Self {
            event_id,
            priority,
            payload,
        }
    }
}
