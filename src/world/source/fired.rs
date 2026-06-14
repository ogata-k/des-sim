use crate::primitive::id::SourceId;
use std::sync::Arc;

pub(crate) struct FiredSourceReady {
    source_ids: Vec<(SourceId, Arc<str>)>,
}

impl FiredSourceReady {
    pub fn new(fired_ids: Vec<(SourceId, Arc<str>)>) -> Self {
        Self {
            source_ids: fired_ids,
        }
    }

    pub fn take_next(&mut self) -> Option<(SourceId, Arc<str>)> {
        if self.source_ids.is_empty() {
            None
        } else {
            Some(self.source_ids.remove(0))
        }
    }
}
