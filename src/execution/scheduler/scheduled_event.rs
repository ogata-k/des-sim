use crate::primitive::time::SimTime;
use crate::world::event::Event;
use std::cmp::Ordering;

#[derive(Clone)]
pub(crate) struct ScheduledEvent<E> {
    pub time: SimTime,
    pub event: Event<E>,
}

impl<E> PartialEq<Self> for ScheduledEvent<E> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<E> Eq for ScheduledEvent<E> {}

impl<E> PartialOrd<Self> for ScheduledEvent<E> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<E> Ord for ScheduledEvent<E> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time
            .cmp(&other.time)
            // priorityは同一時間の範囲でしか効かない
            // Runner次第では同一時間内でも順番に実行されるわけではないので、priorityで指定した順に実行できるかはRunner次第
            .then_with(|| other.event.priority.cmp(&self.event.priority))
            .then_with(|| self.event.id.cmp(&other.event.id))
    }
}
