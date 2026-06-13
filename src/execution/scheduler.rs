use crate::execution::scheduler::scheduled_event::ScheduledEvent;
use crate::primitive::id::EventId;
use crate::primitive::time::{Duration, SimTime};
use crate::world::event::{Event, Priority};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

mod scheduled_event;

pub(crate) struct Scheduler<E> {
    queue: BinaryHeap<Reverse<ScheduledEvent<E>>>,
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

        self.queue.push(Reverse(ScheduledEvent {
            time,
            event: Event {
                id: event_id,
                priority,
                payload: event_payload,
            },
        }));
    }

    /// now時点で発火しているべきイベントをpopしてVecに詰めて返す。
    ///
    /// 実行時に[Duration::zero()]でイベントをスケジュールした場合に、同一時間内でも再度とれる。
    /// なので、同一時間内でさらにループして[self::collect()]した結果が空になるまで取得し続けること。
    /// ※再取得漏れが発生するとエラーになるようになっているので注意。
    pub fn collect(&mut self, now: SimTime) -> Vec<Event<E>> {
        let mut events = Vec::new();
        while let Some(Reverse(event)) = self.queue.peek() {
            assert!(
                event.time >= now,
                "Scheduler invariant violated: event.time={} < now={}",
                event.time,
                now
            );
            if event.time != now {
                break;
            }

            events.push(self.queue.pop().unwrap().0.event);
        }

        events
    }

    pub fn peek(&self) -> Option<(SimTime, &Event<E>)> {
        self.queue.peek().map(|i| (i.0.time, &i.0.event))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive::time::{Duration, SimTime};
    use crate::world::event::Priority;

    #[test]
    fn collect_returns_only_matching_time() {
        let mut scheduler = Scheduler::<&'static str>::new();

        let now = SimTime::new(0);
        let priority = Priority::new(0);
        scheduler.schedule(now, Duration::ticks(10), priority, "t10");
        scheduler.schedule(now, Duration::ticks(20), priority, "t20");
        scheduler.schedule(now, Duration::ticks(30), priority, "t30");

        let events = scheduler.collect(now + Duration::ticks(10));

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload, "t10");

        let (time, event) = scheduler.peek().unwrap();
        assert_eq!(time, SimTime::new(20));
        assert_eq!(event.payload, "t20");
    }

    #[test]
    fn collect_orders_by_priority_when_time_is_same() {
        let mut scheduler = Scheduler::<u8>::new();

        let now = SimTime::new(0);
        let duration = Duration::ticks(10);
        scheduler.schedule(now, duration, Priority::new(10), 10);
        scheduler.schedule(now, duration, Priority::new(20), 20);
        scheduler.schedule(now, duration, Priority::new(30), 30);

        let events = scheduler.collect(now + duration);

        let payloads: Vec<_> = events.into_iter().map(|e| e.payload).collect();

        assert_eq!(payloads, vec![30, 20, 10]);
    }

    #[test]
    fn collect_orders_by_time_then_priority() {
        let mut scheduler = Scheduler::<u8>::new();

        let now = SimTime::new(0);
        let duration = Duration::ticks(10);
        scheduler.schedule(now, duration, Priority::new(20), 1);
        scheduler.schedule(now, duration, Priority::new(10), 2);
        scheduler.schedule(now, duration + duration, Priority::new(5), 3);
        scheduler.schedule(now, duration + duration, Priority::new(30), 4);

        let events_10 = scheduler.collect(now + duration);

        let payloads_10: Vec<_> = events_10.into_iter().map(|e| e.payload).collect();

        assert_eq!(payloads_10, vec![1, 2]);

        let events_20 = scheduler.collect(now + duration + duration);

        let payloads_20: Vec<_> = events_20.into_iter().map(|e| e.payload).collect();

        assert_eq!(payloads_20, vec![4, 3]);
    }

    #[test]
    fn collect_from_empty_scheduler_returns_empty_vec() {
        let mut scheduler = Scheduler::<()>::new();

        let events = scheduler.collect(SimTime::new(0));

        assert!(events.is_empty());
    }

    #[test]
    fn collect_returns_empty_when_no_event_matches_now() {
        let mut scheduler = Scheduler::<u8>::new();

        let now = SimTime::new(30);
        let priority = Priority::new(0);
        scheduler.schedule(now, Duration::ticks(10), priority, 1);
        scheduler.schedule(now, Duration::ticks(30), priority, 2);
        scheduler.schedule(now, Duration::ticks(40), priority, 3);

        let events = scheduler.collect(SimTime::new(30));

        assert!(events.is_empty());

        let (time, _) = scheduler.peek().unwrap();
        assert_eq!(time, SimTime::new(40));
    }

    #[test]
    fn collect_orders_by_event_id_when_time_and_priority_are_same() {
        let mut scheduler = Scheduler::<u8>::new();

        let now = SimTime::new(0);
        let duration = Duration::ticks(10);
        let priority = Priority::new(10);
        scheduler.schedule(now, duration, priority, 1);
        scheduler.schedule(now, duration, priority, 2);
        scheduler.schedule(now, duration, priority, 3);

        let events = scheduler.collect(SimTime::new(10));

        let payloads: Vec<_> = events.into_iter().map(|e| e.payload).collect();

        assert_eq!(payloads, vec![1, 2, 3]);
    }

    #[test]
    fn collect_leaves_future_events_in_queue() {
        let mut scheduler = Scheduler::<u8>::new();

        let now = SimTime::new(0);
        let priority = Priority::new(10);
        scheduler.schedule(now, Duration::ticks(10), priority, 1);
        scheduler.schedule(now, Duration::ticks(10), priority, 1);
        scheduler.schedule(now, Duration::ticks(20), priority, 3);

        let events = scheduler.collect(SimTime::new(10));

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].payload, 1);
        assert_eq!(events[1].payload, 1);

        let (time, event) = scheduler.peek().unwrap();

        assert_eq!(time, SimTime::new(20));
        assert_eq!(event.payload, 3);
    }

    #[test]
    fn collect_reversed_payload() {
        let mut scheduler = Scheduler::<u8>::new();

        let now = SimTime::new(0);
        let priority = Priority::new(10);
        scheduler.schedule(now, Duration::ticks(10), priority, 2);
        scheduler.schedule(now, Duration::ticks(10), priority, 1);
        scheduler.schedule(now, Duration::ticks(20), priority, 3);

        let events = scheduler.collect(SimTime::new(10));

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].payload, 2);
        assert_eq!(events[1].payload, 1);

        let (time, event) = scheduler.peek().unwrap();

        assert_eq!(time, SimTime::new(20));
        assert_eq!(event.payload, 3);
    }

    #[test]
    fn collect_same_payload_in_queue() {
        let mut scheduler = Scheduler::<u8>::new();

        let now = SimTime::new(0);
        let priority = Priority::new(10);
        scheduler.schedule(now, Duration::ticks(10), priority, 1);
        scheduler.schedule(now, Duration::ticks(10), priority, 2);
        scheduler.schedule(now, Duration::ticks(20), priority, 3);

        let events = scheduler.collect(SimTime::new(10));

        assert_eq!(events.len(), 2);

        let (time, event) = scheduler.peek().unwrap();

        assert_eq!(time, SimTime::new(20));
        assert_eq!(event.payload, 3);
    }

    #[test]
    #[should_panic(expected = "Scheduler invariant violated")]
    fn collect_panics_if_past_event_exists() {
        let mut scheduler = Scheduler::<u8>::new();

        scheduler.schedule(SimTime::new(0), Duration::ticks(10), Priority::new(0), 1);
        scheduler.schedule(SimTime::new(0), Duration::ticks(20), Priority::new(0), 2);

        scheduler.collect(SimTime::new(20));
    }
}
