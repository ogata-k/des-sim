use crate::primitive::id::SourceId;
use std::sync::Arc;

pub struct SourceView {
    source_id: SourceId,
    name: Arc<str>,
}

impl SourceView {
    pub(crate) fn new(source_id: SourceId, name: Arc<str>) -> SourceView {
        SourceView { source_id, name }
    }

    pub fn source_id(&self) -> SourceId {
        self.source_id
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}
