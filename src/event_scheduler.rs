use crate::modeling::event::{Event, EventPriority};
use crate::primitive::id::EventId;
use crate::primitive::time::{Duration, SimTime};
use std::cmp::Ordering;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

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
            .then_with(|| self.event.id.cmp(&other.event.id))
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
        now: SimTime,
        delay: Duration,
        priority: EventPriority,
        event_payload: E,
    ) {
        let time = now + delay;
        let event_id = EventId(self.next_event_id);
        self.next_event_id += 1;

        self.pending_queue.push(Reverse(ScheduledEvent {
            scheduled_at: time,
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
    /// なので、同一時間内でさらにループして[self::drain_ready()]した結果が空になるまで取得し続けること。
    /// ※再取得漏れが発生するとpanicするので注意。
    pub fn drain_ready(&mut self, now: SimTime) -> Vec<Event<E>> {
        let mut events = Vec::new();
        while let Some(Reverse(event)) = self.ready_queue.peek() {
            assert!(
                event.scheduled_at >= now,
                "EventScheduler invariant violated: event.scheduled_at={} < now={}",
                event.scheduled_at,
                now
            );
            if event.scheduled_at != now {
                break;
            }

            events.push(self.ready_queue.pop().unwrap().0.event);
        }

        events
    }

    pub fn drain_pending_to_cancel<F>(&mut self, pred: F) -> Vec<(SimTime, Event<E>)>
    where
        F: Fn(SimTime, &Event<E>) -> bool,
    {
        let mut cancelled = Vec::new();

        // 対象がある場合だけ対応する
        if self
            .pending_queue
            .iter()
            .any(|Reverse(ev)| pred(ev.scheduled_at, &ev.event))
        {
            // ヒープを分解して Vec として取り出す
            let items = std::mem::take(&mut self.pending_queue).into_vec();
            let mut to_keep = Vec::with_capacity(items.len());

            // 振り分け
            for Reverse(ev) in items {
                if pred(ev.scheduled_at, &ev.event) {
                    cancelled.push(ev);
                } else {
                    to_keep.push(Reverse(ev));
                }
            }

            // 残った要素でヒープを再構築
            self.pending_queue = BinaryHeap::from(to_keep);
        }

        if self
            .ready_queue
            .iter()
            .any(|Reverse(ev)| pred(ev.scheduled_at, &ev.event))
        {
            // ヒープを分解して Vec として取り出す
            let items = std::mem::take(&mut self.ready_queue).into_vec();
            let mut to_keep = Vec::with_capacity(items.len());

            // 振り分け
            for Reverse(ev) in items {
                if pred(ev.scheduled_at, &ev.event) {
                    cancelled.push(ev);
                } else {
                    to_keep.push(Reverse(ev));
                }
            }

            // 残った要素でヒープを再構築
            self.ready_queue = BinaryHeap::from(to_keep);
        }

        // 扱いやすいように発火順にしておく
        cancelled.sort();
        cancelled
            .into_iter()
            .map(|ev| (ev.scheduled_at, ev.event))
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

        let now = SimTime::new(0);
        let priority = EventPriority::new(0);
        scheduler.schedule(now, Duration::ticks(10), priority, "t10");
        scheduler.schedule(now, Duration::ticks(20), priority, "t20");
        scheduler.schedule(now, Duration::ticks(30), priority, "t30");
        scheduler.flush_pending();

        let events = scheduler.drain_ready(now + Duration::ticks(10));

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload, "t10");

        let (time, event) = scheduler.peek().unwrap();
        assert_eq!(time, SimTime::new(20));
        assert_eq!(event.payload, "t20");
    }

    #[test]
    fn collect_orders_by_priority_when_time_is_same() {
        let mut scheduler = EventScheduler::<u8>::new();

        let now = SimTime::new(0);
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

        let now = SimTime::new(0);
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

        let now = SimTime::new(0);
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

        let events = scheduler.drain_ready(SimTime::new(0));

        assert!(events.is_empty());

        assert_eq!(scheduler.peek_next_time(), None);
    }

    #[test]
    fn collect_returns_empty_when_no_event_matches_now() {
        let mut scheduler = EventScheduler::<u8>::new();

        let now = SimTime::new(30);
        let priority = EventPriority::new(0);
        scheduler.schedule(now, Duration::ticks(10), priority, 1);
        scheduler.schedule(now, Duration::ticks(30), priority, 2);
        scheduler.schedule(now, Duration::ticks(40), priority, 3);
        scheduler.flush_pending();

        let events = scheduler.drain_ready(SimTime::new(30));

        assert!(events.is_empty());

        let time = scheduler.peek_next_time().unwrap();
        assert_eq!(time, SimTime::new(40));
    }

    #[test]
    fn collect_orders_by_event_id_when_time_and_priority_are_same() {
        let mut scheduler = EventScheduler::<u8>::new();

        let now = SimTime::new(0);
        let duration = Duration::ticks(10);
        let priority = EventPriority::new(10);
        scheduler.schedule(now, duration, priority, 1);
        scheduler.schedule(now, duration, priority, 2);
        scheduler.schedule(now, duration, priority, 3);
        scheduler.flush_pending();

        let events = scheduler.drain_ready(SimTime::new(10));

        let payloads: Vec<_> = events.into_iter().map(|e| e.payload).collect();

        assert_eq!(payloads, vec![1, 2, 3]);
    }

    #[test]
    fn collect_leaves_future_events_in_queue() {
        let mut scheduler = EventScheduler::<u8>::new();

        let now = SimTime::new(0);
        let priority = EventPriority::new(10);
        scheduler.schedule(now, Duration::ticks(10), priority, 1);
        scheduler.schedule(now, Duration::ticks(10), priority, 1);
        scheduler.schedule(now, Duration::ticks(20), priority, 3);
        scheduler.flush_pending();

        let events = scheduler.drain_ready(SimTime::new(10));

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].payload, 1);
        assert_eq!(events[1].payload, 1);

        let (time, event) = scheduler.peek().unwrap();

        assert_eq!(time, SimTime::new(20));
        assert_eq!(event.payload, 3);
    }

    #[test]
    fn collect_reversed_payload() {
        let mut scheduler = EventScheduler::<u8>::new();

        let now = SimTime::new(0);
        let priority = EventPriority::new(10);
        scheduler.schedule(now, Duration::ticks(10), priority, 2);
        scheduler.schedule(now, Duration::ticks(10), priority, 1);
        scheduler.schedule(now, Duration::ticks(20), priority, 3);
        scheduler.flush_pending();

        let events = scheduler.drain_ready(SimTime::new(10));

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].payload, 2);
        assert_eq!(events[1].payload, 1);

        let (time, event) = scheduler.peek().unwrap();

        assert_eq!(time, SimTime::new(20));
        assert_eq!(event.payload, 3);
    }

    #[test]
    fn collect_same_payload_in_queue() {
        let mut scheduler = EventScheduler::<u8>::new();

        let now = SimTime::new(0);
        let priority = EventPriority::new(10);
        scheduler.schedule(now, Duration::ticks(10), priority, 1);
        scheduler.schedule(now, Duration::ticks(10), priority, 2);
        scheduler.schedule(now, Duration::ticks(20), priority, 3);
        scheduler.flush_pending();

        let events = scheduler.drain_ready(SimTime::new(10));

        assert_eq!(events.len(), 2);

        let (time, event) = scheduler.peek().unwrap();

        assert_eq!(time, SimTime::new(20));
        assert_eq!(event.payload, 3);
    }

    #[test]
    #[should_panic(expected = "EventScheduler invariant violated")]
    fn collect_panics_if_past_event_exists() {
        let mut scheduler = EventScheduler::<u8>::new();

        scheduler.schedule(
            SimTime::new(0),
            Duration::ticks(10),
            EventPriority::new(0),
            1,
        );
        scheduler.schedule(
            SimTime::new(0),
            Duration::ticks(20),
            EventPriority::new(0),
            2,
        );
        scheduler.flush_pending();

        scheduler.drain_ready(SimTime::new(20));
    }
}
