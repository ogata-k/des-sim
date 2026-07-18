use crate::primitive::id::SourceId;
use std::sync::Arc;

/// A read-only view of a simulation source, providing access to its identity and name.
#[derive(Debug)]
pub struct SourceView {
    source_id: SourceId,
    name: Arc<str>,
}

impl SourceView {
    /// Creates a new `SourceView`.
    pub(crate) fn new(source_id: SourceId, name: Arc<str>) -> SourceView {
        SourceView { source_id, name }
    }

    /// Returns the unique identifier of the source.
    pub fn source_id(&self) -> SourceId {
        self.source_id
    }

    /// Returns the name of the source as a string slice.
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}
