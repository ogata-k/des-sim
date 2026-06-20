use crate::primitive::id::SourceId;
use std::sync::Arc;

#[derive(Debug)]
pub struct SourceReadyEntry {
    source_id: SourceId,
    name: Arc<str>,
}

impl SourceReadyEntry {
    pub(crate) fn new(source_id: SourceId, name: Arc<str>) -> Self {
        Self { source_id, name }
    }

    pub fn source_id(&self) -> SourceId {
        self.source_id
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub fn name_arc(&self) -> Arc<str> {
        Arc::clone(&self.name)
    }
}
