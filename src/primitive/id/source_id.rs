#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct SourceId(usize);

impl SourceId {
    pub(crate) fn new(value: usize) -> Self {
        SourceId(value)
    }

    pub fn value(&self) -> usize {
        self.0
    }
}
