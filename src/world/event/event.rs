use crate::primitive::id::EventId;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
/// priorityは値が大きいほど優先
pub struct Priority(u8);

impl Default for Priority {
    fn default() -> Self {
        Priority::minimum()
    }
}

impl Priority {
    pub const fn new(v: u8) -> Priority {
        Priority(v)
    }

    pub const fn minimum() -> Priority {
        Priority(u8::MIN)
    }

    pub const fn maximum() -> Priority {
        Priority(u8::MAX)
    }

    pub const fn value(&self) -> u8 {
        self.0
    }
}

#[derive(Clone, Debug)]
pub struct Event<E> {
    pub id: EventId,
    pub priority: Priority,
    pub payload: E,
}
