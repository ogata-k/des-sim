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
