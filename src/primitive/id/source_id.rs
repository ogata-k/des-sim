#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct SourceId(usize);

impl SourceId {
    pub(crate) fn new(value: usize) -> Self {
        SourceId(value)
    }

    pub fn value(&self) -> usize {
        self.0
    }
}
