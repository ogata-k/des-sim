//! The `source_id` module defines the `SourceId` type, a unique identifier
//! for each source within the simulation.
//!
//! This ID is essential for tracking, referencing, and managing individual
//! sources throughout their lifecycle.

/// Represents a unique identifier for a simulation source.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct SourceId(usize);

impl SourceId {
    /// Creates a new `SourceId` with the specified value.
    pub(crate) fn new(value: usize) -> Self {
        SourceId(value)
    }

    /// Returns the raw numerical value of the identifier.
    pub fn value(&self) -> usize {
        self.0
    }
}
