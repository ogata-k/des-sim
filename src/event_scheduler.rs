use crate::modeling::event::{Event, EventPriority};
use crate::primitive::id::EventId;
use crate::primitive::time::{Duration, SimTime};
use std::cmp::Ordering;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

#[derive(Clone)]
pub(crate) struct ScheduledEvent<E> {
    pub scheduled_at: SimTime,
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
        self.scheduled_at
            .cmp(&other.scheduled_at)
            // priorityは同一時間の範囲でしか効かない
            // Runner次第では同一時間内でも順番に実行されるわけではないので、priorityで指定した順に実行できるかはRunner次第
            .then_with(|| other.event.priority.cmp(&self.event.priority))
            .then_with(|| self.event.event_id.cmp(&other.event.event_id))
    }
}

#[derive(Clone)]
pub(crate) struct EventScheduler<E> {
    ready_queue: BinaryHeap<Reverse<ScheduledEvent<E>>>,
    pending_queue: BinaryHeap<Reverse<ScheduledEvent<E>>>,
    next_event_id: u64,
}

impl<E> EventScheduler<E> {
    pub fn new() -> Self {
        Self {
            ready_queue: BinaryHeap::new(),
            pending_queue: BinaryHeap::new(),
            next_event_id: 0,
        }
    }

    pub fn schedule(
        &mut self,
        current_tick: SimTime,
        delay: Duration,
        priority: EventPriority,
        event_payload: E,
    ) {
        let time = current_tick + delay;
        let event_id = EventId::new(self.next_event_id);
        self.next_event_id += 1;

        self.pending_queue.push(Reverse(ScheduledEvent {
            scheduled_at: time,
            event: Event::new(event_id, priority, event_payload),
        }));
    }

    /// now時点で発火しているべきイベントをpopしてVecに詰めて返す。
    ///
    /// 実行時に[Duration::zero()]でイベントをスケジュールした場合に、同一時間内でも再度とれる。
    /// なので、同一時間内でさらにループして[self::drain_ready()]した結果が空になるまで取得し続けること。
    /// ※再取得漏れが発生するとpanicするので注意。
    pub fn drain_ready(&mut self, current_tick: SimTime) -> VecDeque<Event<E>> {
        let mut events = VecDeque::new();
        while let Some(Reverse(event)) = self.ready_queue.peek() {
            assert!(
                event.scheduled_at >= current_tick,
                "EventScheduler invariant violated: event.scheduled_at={} < now={}",
                event.scheduled_at,
                current_tick
            );
            if event.scheduled_at != current_tick {
                break;
            }

            events.push_back(self.ready_queue.pop().unwrap().0.event);
        }

        events
    }

    pub fn drain_cancel_scheduled<F>(&mut self, mut pred: F) -> VecDeque<(SimTime, Event<E>)>
    where
        F: FnMut(SimTime, &Event<E>) -> bool,
    {
        let mut cancelled = Vec::new();

        // 対象がある場合だけ対応する
        if self
            .pending_queue
            .iter()
            .any(|Reverse(scheduled)| pred(scheduled.scheduled_at, &scheduled.event))
        {
            // ヒープを分解して Vec として取り出す
            let items = std::mem::take(&mut self.pending_queue).into_vec();
            let mut to_keep = Vec::with_capacity(items.len());

            // 振り分け
            for Reverse(scheduled) in items {
                if pred(scheduled.scheduled_at, &scheduled.event) {
                    cancelled.push(scheduled);
                } else {
                    to_keep.push(Reverse(scheduled));
                }
            }

            // 残った要素でヒープを再構築
            self.pending_queue = BinaryHeap::from(to_keep);
        }

        if self
            .ready_queue
            .iter()
            .any(|Reverse(scheduled)| pred(scheduled.scheduled_at, &scheduled.event))
        {
            // ヒープを分解して Vec として取り出す
            let items = std::mem::take(&mut self.ready_queue).into_vec();
            let mut to_keep = Vec::with_capacity(items.len());

            // 振り分け
            for Reverse(scheduled) in items {
                if pred(scheduled.scheduled_at, &scheduled.event) {
                    cancelled.push(scheduled);
                } else {
                    to_keep.push(Reverse(scheduled));
                }
            }

            // 残った要素でヒープを再構築
            self.ready_queue = BinaryHeap::from(to_keep);
        }

        // 扱いやすいように発火順にしておく
        cancelled.sort();
        cancelled
            .into_iter()
            .map(|scheduled| (scheduled.scheduled_at, scheduled.event))
            .collect()
    }

    pub fn peek_next_time(&self) -> Option<SimTime> {
        self.ready_queue.peek().map(|i| i.0.scheduled_at)
    }

    #[cfg(test)]
    pub fn peek(&self) -> Option<(SimTime, &Event<E>)> {
        self.ready_queue
            .peek()
            .map(|i| (i.0.scheduled_at, &i.0.event))
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn ready_queue_len(&self) -> usize {
        self.ready_queue.len()
    }

    /// スケジュールされたEventを反映させる
    pub fn flush_pending(&mut self) {
        self.ready_queue.append(&mut self.pending_queue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modeling::event::EventPriority;
    use crate::primitive::time::{Duration, SimTime};

    #[test]
    fn collect_returns_only_matching_time() {
        let mut scheduler = EventScheduler::<&'static str>::new();

        let now = SimTime::from_ticks(0);
        let priority = EventPriority::new(0);
        scheduler.schedule(now, Duration::ticks(10), priority, "t10");
        scheduler.schedule(now, Duration::ticks(20), priority, "t20");
        scheduler.schedule(now, Duration::ticks(30), priority, "t30");
        scheduler.flush_pending();

        let events = scheduler.drain_ready(now + Duration::ticks(10));

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload, "t10");

        let (time, event) = scheduler.peek().unwrap();
        assert_eq!(time, SimTime::from_ticks(20));
        assert_eq!(event.payload, "t20");
    }

    #[test]
    fn collect_orders_by_priority_when_time_is_same() {
        let mut scheduler = EventScheduler::<u8>::new();

        let now = SimTime::from_ticks(0);
        let duration = Duration::ticks(10);
        scheduler.schedule(now, duration, EventPriority::new(10), 10);
        scheduler.schedule(now, duration, EventPriority::new(20), 20);
        scheduler.schedule(now, duration, EventPriority::new(30), 30);
        scheduler.flush_pending();

        let events = scheduler.drain_ready(now + duration);

        let payloads: Vec<_> = events.into_iter().map(|e| e.payload).collect();

        assert_eq!(payloads, vec![30, 20, 10]);
    }

    #[test]
    fn collect_before_flush() {
        let mut scheduler = EventScheduler::<u8>::new();

        let now = SimTime::from_ticks(0);
        let duration = Duration::ticks(10);
        scheduler.schedule(now, duration, EventPriority::new(10), 10);
        scheduler.schedule(now, duration, EventPriority::new(20), 20);
        scheduler.schedule(now, duration, EventPriority::new(30), 30);

        // flush前は取得できない
        let events = scheduler.drain_ready(now + duration);
        assert!(events.is_empty());

        scheduler.flush_pending();

        let events = scheduler.drain_ready(now + duration);

        let payloads: Vec<_> = events.into_iter().map(|e| e.payload).collect();

        assert_eq!(payloads, vec![30, 20, 10]);
    }

    #[test]
    fn collect_orders_by_time_then_priority() {
        let mut scheduler = EventScheduler::<u8>::new();

        let now = SimTime::from_ticks(0);
        let duration = Duration::ticks(10);
        scheduler.schedule(now, duration, EventPriority::new(20), 1);
        scheduler.schedule(now, duration, EventPriority::new(10), 2);
        scheduler.schedule(now, duration + duration, EventPriority::new(5), 3);
        scheduler.schedule(now, duration + duration, EventPriority::new(30), 4);
        scheduler.flush_pending();

        let events_10 = scheduler.drain_ready(now + duration);

        let payloads_10: Vec<_> = events_10.into_iter().map(|e| e.payload).collect();

        assert_eq!(payloads_10, vec![1, 2]);

        let events_20 = scheduler.drain_ready(now + duration + duration);

        let payloads_20: Vec<_> = events_20.into_iter().map(|e| e.payload).collect();

        assert_eq!(payloads_20, vec![4, 3]);
    }

    #[test]
    fn collect_from_empty_scheduler_returns_empty_vec() {
        let mut scheduler = EventScheduler::<()>::new();

        let events = scheduler.drain_ready(SimTime::from_ticks(0));

        assert!(events.is_empty());

        assert_eq!(scheduler.peek_next_time(), None);
    }

    #[test]
    fn collect_returns_empty_when_no_event_matches_now() {
        let mut scheduler = EventScheduler::<u8>::new();

        let now = SimTime::from_ticks(30);
        let priority = EventPriority::new(0);
        scheduler.schedule(now, Duration::ticks(10), priority, 1);
        scheduler.schedule(now, Duration::ticks(30), priority, 2);
        scheduler.schedule(now, Duration::ticks(40), priority, 3);
        scheduler.flush_pending();

        let events = scheduler.drain_ready(SimTime::from_ticks(30));

        assert!(events.is_empty());

        let time = scheduler.peek_next_time().unwrap();
        assert_eq!(time, SimTime::from_ticks(40));
    }

    #[test]
    fn collect_orders_by_event_id_when_time_and_priority_are_same() {
        let mut scheduler = EventScheduler::<u8>::new();

        let now = SimTime::from_ticks(0);
        let duration = Duration::ticks(10);
        let priority = EventPriority::new(10);
        scheduler.schedule(now, duration, priority, 1);
        scheduler.schedule(now, duration, priority, 2);
        scheduler.schedule(now, duration, priority, 3);
        scheduler.flush_pending();

        let events = scheduler.drain_ready(SimTime::from_ticks(10));

        let payloads: Vec<_> = events.into_iter().map(|e| e.payload).collect();

        assert_eq!(payloads, vec![1, 2, 3]);
    }

    #[test]
    fn collect_leaves_future_events_in_queue() {
        let mut scheduler = EventScheduler::<u8>::new();

        let now = SimTime::from_ticks(0);
        let priority = EventPriority::new(10);
        scheduler.schedule(now, Duration::ticks(10), priority, 1);
        scheduler.schedule(now, Duration::ticks(10), priority, 1);
        scheduler.schedule(now, Duration::ticks(20), priority, 3);
        scheduler.flush_pending();

        let events = scheduler.drain_ready(SimTime::from_ticks(10));

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].payload, 1);
        assert_eq!(events[1].payload, 1);

        let (time, event) = scheduler.peek().unwrap();

        assert_eq!(time, SimTime::from_ticks(20));
        assert_eq!(event.payload, 3);
    }

    #[test]
    fn collect_reversed_payload() {
        let mut scheduler = EventScheduler::<u8>::new();

        let now = SimTime::from_ticks(0);
        let priority = EventPriority::new(10);
        scheduler.schedule(now, Duration::ticks(10), priority, 2);
        scheduler.schedule(now, Duration::ticks(10), priority, 1);
        scheduler.schedule(now, Duration::ticks(20), priority, 3);
        scheduler.flush_pending();

        let events = scheduler.drain_ready(SimTime::from_ticks(10));

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].payload, 2);
        assert_eq!(events[1].payload, 1);

        let (time, event) = scheduler.peek().unwrap();

        assert_eq!(time, SimTime::from_ticks(20));
        assert_eq!(event.payload, 3);
    }

    #[test]
    fn collect_same_payload_in_queue() {
        let mut scheduler = EventScheduler::<u8>::new();

        let now = SimTime::from_ticks(0);
        let priority = EventPriority::new(10);
        scheduler.schedule(now, Duration::ticks(10), priority, 1);
        scheduler.schedule(now, Duration::ticks(10), priority, 2);
        scheduler.schedule(now, Duration::ticks(20), priority, 3);
        scheduler.flush_pending();

        let events = scheduler.drain_ready(SimTime::from_ticks(10));

        assert_eq!(events.len(), 2);

        let (time, event) = scheduler.peek().unwrap();

        assert_eq!(time, SimTime::from_ticks(20));
        assert_eq!(event.payload, 3);
    }

    #[test]
    #[should_panic(expected = "EventScheduler invariant violated")]
    fn collect_panics_if_past_event_exists() {
        let mut scheduler = EventScheduler::<u8>::new();

        scheduler.schedule(
            SimTime::from_ticks(0),
            Duration::ticks(10),
            EventPriority::new(0),
            1,
        );
        scheduler.schedule(
            SimTime::from_ticks(0),
            Duration::ticks(20),
            EventPriority::new(0),
            2,
        );
        scheduler.flush_pending();

        scheduler.drain_ready(SimTime::from_ticks(20));
    }

    #[test]
    fn cancel_single_event_from_pending_queue() {
        let mut scheduler = EventScheduler::<&'static str>::new();
        let now = SimTime::from_ticks(0);
        let priority = EventPriority::new(0);

        scheduler.schedule(now, Duration::ticks(10), priority, "event_to_cancel");
        scheduler.schedule(now, Duration::ticks(20), priority, "event_to_keep_1");
        scheduler.schedule(now, Duration::ticks(30), priority, "event_to_keep_2");

        let cancelled_events =
            scheduler.drain_cancel_scheduled(|_, event| event.payload == "event_to_cancel");

        assert_eq!(cancelled_events.len(), 1);
        assert_eq!(cancelled_events[0].1.payload, "event_to_cancel");

        // Verify remaining events in pending queue
        scheduler.flush_pending();
        let events_at_20 = scheduler.drain_ready(now + Duration::ticks(20));
        assert_eq!(events_at_20.len(), 1);
        assert_eq!(events_at_20[0].payload, "event_to_keep_1");

        let events_at_30 = scheduler.drain_ready(now + Duration::ticks(30));
        assert_eq!(events_at_30.len(), 1);
        assert_eq!(events_at_30[0].payload, "event_to_keep_2");
    }

    #[test]
    fn cancel_single_event_from_ready_queue() {
        let mut scheduler = EventScheduler::<&'static str>::new();
        let now = SimTime::from_ticks(0);
        let priority = EventPriority::new(0);

        scheduler.schedule(now, Duration::ticks(10), priority, "event_to_keep_1");
        scheduler.schedule(now, Duration::ticks(20), priority, "event_to_cancel");
        scheduler.schedule(now, Duration::ticks(30), priority, "event_to_keep_2");
        scheduler.flush_pending();

        let cancelled_events =
            scheduler.drain_cancel_scheduled(|_, event| event.payload == "event_to_cancel");

        assert_eq!(cancelled_events.len(), 1);
        assert_eq!(cancelled_events[0].1.payload, "event_to_cancel");

        // Verify remaining events in ready queue
        let events_at_10 = scheduler.drain_ready(now + Duration::ticks(10));
        assert_eq!(events_at_10.len(), 1);
        assert_eq!(events_at_10[0].payload, "event_to_keep_1");

        let events_at_30 = scheduler.drain_ready(now + Duration::ticks(30));
        assert_eq!(events_at_30.len(), 1);
        assert_eq!(events_at_30[0].payload, "event_to_keep_2");
    }

    #[test]
    fn cancel_multiple_events() {
        let mut scheduler = EventScheduler::<&'static str>::new();
        let now = SimTime::from_ticks(0);
        let priority = EventPriority::new(0);

        scheduler.schedule(now, Duration::ticks(10), priority, "cancel_me_1");
        scheduler.schedule(now, Duration::ticks(20), priority, "keep_me");
        scheduler.schedule(now, Duration::ticks(30), priority, "cancel_me_2");
        scheduler.schedule(now, Duration::ticks(40), priority, "cancel_me_3");
        scheduler.flush_pending();

        let cancelled_events =
            scheduler.drain_cancel_scheduled(|_, event| event.payload.contains("cancel_me"));

        assert_eq!(cancelled_events.len(), 3);
        let payloads: Vec<_> = cancelled_events
            .into_iter()
            .map(|(_, event)| event.payload)
            .collect();
        assert!(payloads.contains(&"cancel_me_1"));
        assert!(payloads.contains(&"cancel_me_2"));
        assert!(payloads.contains(&"cancel_me_3"));

        let events_at_20 = scheduler.drain_ready(now + Duration::ticks(20));
        assert_eq!(events_at_20.len(), 1);
        assert_eq!(events_at_20[0].payload, "keep_me");
    }

    #[test]
    fn cancel_no_events() {
        let mut scheduler = EventScheduler::<&'static str>::new();
        let now = SimTime::from_ticks(0);
        let priority = EventPriority::new(0);

        scheduler.schedule(now, Duration::ticks(10), priority, "event_1");
        scheduler.schedule(now, Duration::ticks(20), priority, "event_2");
        scheduler.flush_pending();

        let cancelled_events =
            scheduler.drain_cancel_scheduled(|_, event| event.payload == "non_existent_event");

        assert!(cancelled_events.is_empty());

        // Verify all events are still present
        let events_at_10 = scheduler.drain_ready(now + Duration::ticks(10));
        assert_eq!(events_at_10.len(), 1);
        assert_eq!(events_at_10[0].payload, "event_1");

        let events_at_20 = scheduler.drain_ready(now + Duration::ticks(20));
        assert_eq!(events_at_20.len(), 1);
        assert_eq!(events_at_20[0].payload, "event_2");
    }

    #[test]
    fn cancel_all_events() {
        let mut scheduler = EventScheduler::<&'static str>::new();
        let now = SimTime::from_ticks(0);
        let priority = EventPriority::new(0);

        scheduler.schedule(now, Duration::ticks(10), priority, "event_1");
        scheduler.schedule(now, Duration::ticks(20), priority, "event_2");
        scheduler.flush_pending();

        let cancelled_events = scheduler.drain_cancel_scheduled(|_, _| true); // Cancel all

        assert_eq!(cancelled_events.len(), 2);
        let payloads: Vec<_> = cancelled_events
            .into_iter()
            .map(|(_, event)| event.payload)
            .collect();
        assert!(payloads.contains(&"event_1"));
        assert!(payloads.contains(&"event_2"));

        // Verify no events left
        let events = scheduler.drain_ready(now + Duration::ticks(10));
        assert!(events.is_empty());
        let events = scheduler.drain_ready(now + Duration::ticks(20));
        assert!(events.is_empty());
    }

    #[test]
    fn cancel_event_before_flush_only_affects_pending() {
        let mut scheduler = EventScheduler::<&'static str>::new();
        let now = SimTime::from_ticks(0);
        let priority = EventPriority::new(0);

        scheduler.schedule(now, Duration::ticks(10), priority, "pending_cancel");
        scheduler.schedule(now, Duration::ticks(20), priority, "pending_keep");
        scheduler.flush_pending(); // Flush some events to ready queue
        scheduler.schedule(now, Duration::ticks(30), priority, "ready_cancel");
        scheduler.schedule(now, Duration::ticks(40), priority, "ready_keep");

        // This flush is intentionally missing to test pending queue cancellation
        // scheduler.flush_pending();

        let cancelled_events =
            scheduler.drain_cancel_scheduled(|_, event| event.payload.contains("cancel"));

        assert_eq!(cancelled_events.len(), 2);
        let payloads: Vec<_> = cancelled_events
            .into_iter()
            .map(|(_, event)| event.payload)
            .collect();
        assert!(payloads.contains(&"pending_cancel"));
        assert!(payloads.contains(&"ready_cancel"));

        // Verify remaining events
        scheduler.flush_pending(); // Now flush the remaining pending events

        let events_at_20 = scheduler.drain_ready(now + Duration::ticks(20));
        assert_eq!(events_at_20.len(), 1);
        assert_eq!(events_at_20[0].payload, "pending_keep");

        let events_at_40 = scheduler.drain_ready(now + Duration::ticks(40));
        assert_eq!(events_at_40.len(), 1);
        assert_eq!(events_at_40[0].payload, "ready_keep");
    }

    #[test]
    fn cancel_event_by_id() {
        let mut scheduler = EventScheduler::<&'static str>::new();
        let now = SimTime::from_ticks(0);
        let priority = EventPriority::new(0);

        scheduler.schedule(now, Duration::ticks(10), priority, "event_1"); // id 0
        scheduler.schedule(now, Duration::ticks(20), priority, "event_2"); // id 1
        scheduler.schedule(now, Duration::ticks(30), priority, "event_3"); // id 2
        scheduler.flush_pending();

        let event_to_cancel_id = EventId::new(1); // Cancel event_2

        let cancelled_events =
            scheduler.drain_cancel_scheduled(|_, event| event.event_id == event_to_cancel_id);

        assert_eq!(cancelled_events.len(), 1);
        assert_eq!(cancelled_events[0].1.payload, "event_2");
        assert_eq!(cancelled_events[0].1.event_id, event_to_cancel_id);

        // Verify remaining events
        let events_at_10 = scheduler.drain_ready(now + Duration::ticks(10));
        assert_eq!(events_at_10.len(), 1);
        assert_eq!(events_at_10[0].payload, "event_1");

        let events_at_30 = scheduler.drain_ready(now + Duration::ticks(30));
        assert_eq!(events_at_30.len(), 1);
        assert_eq!(events_at_30[0].payload, "event_3");
    }

    #[test]
    fn cancel_event_with_mixed_queues() {
        let mut scheduler = EventScheduler::<&'static str>::new();
        let now = SimTime::from_ticks(0);
        let priority = EventPriority::new(0);

        // Events in pending_queue initially
        scheduler.schedule(now, Duration::ticks(10), priority, "pending_keep_1");
        scheduler.schedule(now, Duration::ticks(20), priority, "pending_cancel_1");

        scheduler.flush_pending(); // Move pending_keep_1 and pending_cancel_1 to ready_queue

        // Events now in pending_queue
        scheduler.schedule(now, Duration::ticks(15), priority, "pending_cancel_2");
        scheduler.schedule(now, Duration::ticks(25), priority, "pending_keep_2");

        let cancelled_events =
            scheduler.drain_cancel_scheduled(|_, event| event.payload.contains("cancel"));

        assert_eq!(cancelled_events.len(), 2);
        let payloads: Vec<_> = cancelled_events
            .into_iter()
            .map(|(_, event)| event.payload)
            .collect();
        assert!(payloads.contains(&"pending_cancel_1"));
        assert!(payloads.contains(&"pending_cancel_2"));

        // Verify remaining events
        scheduler.flush_pending(); // Flush the remaining pending events

        let events_at_10 = scheduler.drain_ready(now + Duration::ticks(10));
        assert_eq!(events_at_10.len(), 1);
        assert_eq!(events_at_10[0].payload, "pending_keep_1");

        let events_at_25 = scheduler.drain_ready(now + Duration::ticks(25));
        assert_eq!(events_at_25.len(), 1);
        assert_eq!(events_at_25[0].payload, "pending_keep_2");
    }
}
