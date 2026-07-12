use crate::primitive::id::EventId;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
/// priorityは値が大きいほど優先
pub struct EventPriority(u8);

impl Default for EventPriority {
    fn default() -> Self {
        EventPriority::minimum()
    }
}

impl EventPriority {
    pub const fn new(v: u8) -> EventPriority {
        EventPriority(v)
    }

    pub const fn minimum() -> EventPriority {
        EventPriority(u8::MIN)
    }

    pub const fn maximum() -> EventPriority {
        EventPriority(u8::MAX)
    }

    pub const fn value(&self) -> u8 {
        self.0
    }
}

#[derive(Clone, Debug)]
pub struct Event<E> {
    pub event_id: EventId,
    pub priority: EventPriority,
    pub payload: E,
}

impl<E> Event<E> {
    pub(crate) fn new(event_id: EventId, priority: EventPriority, payload: E) -> Self {
        Self {
            event_id,
            priority,
            payload,
        }
    }
}
