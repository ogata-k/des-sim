//! The `event_id` module defines the `EventId` type, a unique identifier
//! for each event within the simulation.
//!
//! This ID is crucial for tracking, referencing, and managing individual
//! events throughout their lifecycle.

/// Represents a unique identifier for an event within the simulation.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct EventId(u64);

impl EventId {
    /// Creates a new `EventId` with the specified value.
    pub(crate) fn new(value: u64) -> Self {
        EventId(value)
    }

    /// Returns the raw numerical value of the identifier.
    pub fn value(&self) -> u64 {
        self.0
    }
}
