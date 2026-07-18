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
