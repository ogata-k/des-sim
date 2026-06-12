use crate::execution::scheduler::scheduled_event::ScheduledEvent;
use crate::primitive::id::EventId;
use crate::primitive::time::{Duration, SimTime};
use crate::world::event::{Event, Priority};
use std::collections::BinaryHeap;

mod scheduled_event;

pub(crate) struct Scheduler<E> {
    queue: BinaryHeap<ScheduledEvent<E>>,
    next_event_id: u64,
}

impl<E> Scheduler<E> {
    pub fn new() -> Self {
        Self {
            queue: BinaryHeap::new(),
            next_event_id: 0,
        }
    }

    pub fn schedule(
        &mut self,
        now: SimTime,
        delay: Duration,
        priority: Priority,
        event_payload: E,
    ) {
        let time = now + delay;
        let event_id = EventId(self.next_event_id);
        self.next_event_id += 1;

        self.queue.push(ScheduledEvent {
            time,
            event: Event {
                id: event_id,
                priority,
                payload: event_payload,
            },
        });
    }

    /// now時点で発火しているべきイベントをpopしてVecに詰めて返す。
    ///
    /// 実行時に[Duration::zero()]でイベントをスケジュールした場合に、同一時間内でも再度とれる。
    /// なので、同一時間内でさらにループして[self::collect()]した結果が空になるまで取得し続けること。
    /// ※再取得漏れが発生するとエラーになるようになっているので注意。
    pub fn collect(&mut self, now: SimTime) -> Vec<Event<E>> {
        let mut events = Vec::new();
        while let Some(event) = self.queue.peek() {
            assert!(
                event.time >= now,
                "Scheduler invariant violated: event.time={} < now={}",
                event.time,
                now
            );
            if event.time != now {
                break;
            }

            events.push(self.queue.pop().unwrap().event);
        }

        events
    }

    pub fn peek(&self) -> Option<(SimTime, &Event<E>)> {
        self.queue.peek().map(|i| (i.time, &i.event))
    }
}
