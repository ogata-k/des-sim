use crate::primitive::id::SourceId;
use std::sync::Arc;

/// Represents an entry for a source that is ready to be utilized in the simulation.
#[derive(Debug)]
pub struct SourceReadyEntry {
    source_id: SourceId,
    name: Arc<str>,
}

impl SourceReadyEntry {
    /// Creates a new `SourceReadyEntry`.
    pub(crate) fn new(source_id: SourceId, name: Arc<str>) -> Self {
        Self { source_id, name }
    }

    /// Returns the unique identifier of the source.
    pub fn source_id(&self) -> SourceId {
        self.source_id
    }

    /// Returns a string slice representing the name of the source.
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// Returns a cloned `Arc` pointer to the source's name.
    pub(crate) fn clone_name_arc(&self) -> Arc<str> {
        Arc::clone(&self.name)
    }
}
