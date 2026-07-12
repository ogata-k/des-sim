#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub struct EventId(u64);

impl EventId {
    pub(crate) fn new(value: u64) -> Self {
        EventId(value)
    }

    pub fn value(&self) -> u64 {
        self.0
    }
}
