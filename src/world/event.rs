use crate::primitive::id::EventId;

#[derive(Clone, Debug)]
pub struct Event<E> {
    pub id: EventId,
    /// priorityは値が大きいほど優先
    pub priority: u8,
    pub payload: E,
}
