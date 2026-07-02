mod fired;
mod view;

pub use fired::*;
pub use view::*;

use crate::modeling::model::Model;
use crate::modeling::source::Source;
use crate::primitive::id::SourceId;
use crate::primitive::time::{Duration, SimTime};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use std::sync::Arc;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub(crate) struct ScheduledSource {
    pub(crate) scheduled_at: SimTime,
    pub(crate) source_id: SourceId,
}

pub(crate) struct SourceEntry<E, M: Model<E>> {
    pub(crate) name: Arc<str>,
    pub(crate) source: Box<dyn Source<E, M>>,
}

pub(crate) struct SourceHandler<E, M: Model<E>> {
    source_registry: Vec<SourceEntry<E, M>>,
    next_source_id: usize,
    // Rustは最大ヒープなので、time->source_idの順のソートの小さい順にする
    ready_queue: BinaryHeap<Reverse<ScheduledSource>>,
    pending_queue: BinaryHeap<Reverse<ScheduledSource>>,
}

impl<E, M: Model<E>> SourceHandler<E, M> {
    pub fn new() -> SourceHandler<E, M> {
        SourceHandler {
            source_registry: vec![],
            next_source_id: 0,
            ready_queue: BinaryHeap::new(),
            pending_queue: BinaryHeap::new(),
        }
    }

    fn to_ready_entry(&self, scheduled: ScheduledSource) -> SourceReadyEntry {
        SourceReadyEntry::new(
            scheduled.source_id,
            Arc::clone(&self.source_registry[scheduled.source_id.value()].name),
        )
    }

    pub(crate) fn initialize_sources<F>(&mut self, mut initializer: F)
    where
        F: FnMut(&mut SourceEntry<E, M>),
    {
        self.source_registry.iter_mut().for_each(|e| {
            initializer(e);
        })
    }

    /// [Source]を初回起動日時で実行するように登録する。
    /// 使用用途は、初回登録用途。
    pub fn add_source<S>(&mut self, name: &'static str, first_fire_time: SimTime, source: S)
    where
        S: Source<E, M> + 'static,
    {
        self.source_registry.push(SourceEntry {
            name: Arc::from(name),
            source: Box::new(source),
        });
        self.pending_queue.push(Reverse(ScheduledSource {
            scheduled_at: first_fire_time,
            source_id: SourceId::new(self.next_source_id),
        }));
        self.next_source_id += 1;
    }

    /// [Source]を現在時刻から時間がたった後の時間で実行するように登録する。
    /// 使用用途は、シミュレーション中での使用。
    pub fn add_source_after<S>(
        &mut self,
        name: &'static str,
        current_tick: SimTime,
        delay: Duration,
        source: S,
    ) where
        S: Source<E, M> + 'static,
    {
        self.source_registry.push(SourceEntry {
            name: Arc::from(name),
            source: Box::new(source),
        });
        self.pending_queue.push(Reverse(ScheduledSource {
            scheduled_at: current_tick + delay,
            source_id: SourceId::new(self.next_source_id),
        }));
        self.next_source_id += 1;
    }

    /// now時点で発火しているべきソースをpopしてVecに詰めて返す。
    ///
    /// 実行時に[Duration::zero()]でイベントをスケジュールした場合に、同一時間内でも再度とれる。
    /// なので、同一時間内でさらにループして[self::drain_ready()]した結果が空になるまで取得し続けること。
    /// ※再取得漏れが発生するとpanicするので注意。
    pub fn drain_ready(&mut self, current_tick: SimTime) -> VecDeque<SourceReadyEntry> {
        let mut fired_source_indexes: VecDeque<SourceReadyEntry> = VecDeque::new();
        while let Some(Reverse(scheduled)) = self.ready_queue.peek() {
            assert!(
                scheduled.scheduled_at >= current_tick,
                "SourceScheduler invariant violated: scheduled_source.scheduled_at={} < now={}",
                scheduled.scheduled_at,
                current_tick
            );
            if scheduled.scheduled_at != current_tick {
                break;
            }

            // 各Sourceで処理されてnowの次のマイクロステップに登録されても処理されないように、先に集めておく。
            let scheduled = self.ready_queue.pop().unwrap().0;
            fired_source_indexes.push_back(self.to_ready_entry(scheduled));
        }

        fired_source_indexes
    }

    pub(crate) fn drain_cancel_scheduled<F>(
        &mut self,
        mut pred: F,
    ) -> Vec<(SimTime, SourceReadyEntry)>
    where
        F: FnMut(SimTime, &SourceReadyEntry) -> bool,
    {
        let mut cancelled = Vec::new();

        // 対象がある場合だけ対応する
        if self.pending_queue.iter().any(|Reverse(scheduled)| {
            pred(scheduled.scheduled_at, &self.to_ready_entry(*scheduled))
        }) {
            // ヒープを分解して Vec として取り出す
            let items = std::mem::take(&mut self.pending_queue).into_vec();
            let mut to_keep = Vec::with_capacity(items.len());

            // 振り分け
            for Reverse(scheduled) in items {
                if pred(scheduled.scheduled_at, &self.to_ready_entry(scheduled)) {
                    cancelled.push(scheduled);
                } else {
                    to_keep.push(Reverse(scheduled));
                }
            }

            // 残った要素でヒープを再構築
            self.pending_queue = BinaryHeap::from(to_keep);
        }

        if self.ready_queue.iter().any(|Reverse(scheduled)| {
            pred(scheduled.scheduled_at, &self.to_ready_entry(*scheduled))
        }) {
            // ヒープを分解して Vec として取り出す
            let items = std::mem::take(&mut self.ready_queue).into_vec();
            let mut to_keep = Vec::with_capacity(items.len());

            // 振り分け
            for Reverse(scheduled) in items {
                if pred(scheduled.scheduled_at, &self.to_ready_entry(scheduled)) {
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
            .map(|scheduled| (scheduled.scheduled_at, self.to_ready_entry(scheduled)))
            .collect()
    }

    pub(crate) fn get_by_source_id(&mut self, source_id: SourceId) -> &mut SourceEntry<E, M> {
        &mut self.source_registry[source_id.value()]
    }

    pub(crate) fn schedule_next(
        &mut self,
        current_tick: SimTime,
        next_fire_delay: Duration,
        source_id: SourceId,
    ) {
        self.pending_queue.push(Reverse(ScheduledSource {
            scheduled_at: current_tick + next_fire_delay,
            source_id,
        }));
    }

    pub fn peek_next_time(&self) -> Option<SimTime> {
        self.ready_queue.peek().map(|i| i.0.scheduled_at)
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub fn peek(&self) -> Option<(SimTime, &ScheduledSource)> {
        self.ready_queue.peek().map(|i| (i.0.scheduled_at, &i.0))
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn ready_queue_len(&self) -> usize {
        self.ready_queue.len()
    }

    /// スケジュールされたSourceを反映させる
    pub fn flush_pending(&mut self) {
        self.ready_queue.append(&mut self.pending_queue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{EventContext, SourceContext};
    use crate::modeling::event::Event;
    use crate::modeling::model::Model;
    use crate::modeling::source::Source;
    use crate::primitive::time::{Duration, SimTime};

    // Dummy Event for testing
    #[derive(Debug, PartialEq, Eq, Clone)]
    enum TestEvent {}

    // Dummy Model for testing
    struct TestModel;

    impl Model<TestEvent> for TestModel {
        fn handle_event(
            &mut self,
            _context: &mut EventContext<TestEvent, TestModel>,
            _event: &Event<TestEvent>,
        ) {
            // none
        }
    }

    // Dummy Source for testing
    struct TestSource {
        #[allow(unused)]
        id: usize,
    }

    impl Source<TestEvent, TestModel> for TestSource {
        fn initialize(
            &mut self,
            _context: &mut SourceContext<TestEvent, TestModel>,
            _model: &TestModel,
        ) {
            // none
        }

        fn fire(
            &mut self,
            _context: &mut SourceContext<TestEvent, TestModel>,
            _model: &TestModel,
        ) -> Option<Duration> {
            None
        }
    }

    #[test]
    fn test_new() {
        let handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        assert!(handler.source_registry.is_empty());
        assert_eq!(handler.next_source_id, 0);
        assert!(handler.ready_queue.is_empty());
        assert!(handler.pending_queue.is_empty());
    }

    #[test]
    fn test_add_source() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let source1 = TestSource { id: 1 };
        let source2 = TestSource { id: 2 };

        handler.add_source("source1", SimTime::from(10), source1);
        handler.add_source("source2", SimTime::from(5), source2);

        assert_eq!(handler.source_registry.len(), 2);
        assert_eq!(handler.next_source_id, 2);
        assert!(handler.ready_queue.is_empty());
        assert_eq!(handler.pending_queue.len(), 2);

        // Check pending queue order (smallest scheduled_at first due to Reverse)
        let pending_sources: Vec<ScheduledSource> =
            handler.pending_queue.iter().map(|s| s.0).collect();

        assert_eq!(pending_sources[0].scheduled_at, SimTime::from(5));
        assert_eq!(pending_sources[0].source_id, SourceId::new(1));
        assert_eq!(pending_sources[1].scheduled_at, SimTime::from(10));
        assert_eq!(pending_sources[1].source_id, SourceId::new(0));
    }

    #[test]
    fn test_add_source_after() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let source = TestSource { id: 1 };
        let current_tick = SimTime::from(100);
        let delay = Duration::from(50);

        handler.add_source_after("source_after", current_tick, delay, source);

        assert_eq!(handler.source_registry.len(), 1);
        assert_eq!(handler.next_source_id, 1);
        assert!(handler.ready_queue.is_empty());
        assert_eq!(handler.pending_queue.len(), 1);

        let scheduled = handler.pending_queue.peek().unwrap().0;
        assert_eq!(scheduled.scheduled_at, current_tick + delay);
        assert_eq!(scheduled.source_id, SourceId::new(0));
    }

    #[test]
    fn test_add_source_after_zero_delay() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let source = TestSource { id: 1 };
        let current_tick = SimTime::from(100);
        let delay = Duration::zero();

        handler.add_source_after("source_after", current_tick, delay, source);
    }

    #[test]
    fn test_add_source_at_now() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let source = TestSource { id: 1 };
        let current_tick = SimTime::from(100);

        handler.add_source("source_at_now", current_tick, source);

        assert_eq!(handler.source_registry.len(), 1);
        assert_eq!(handler.next_source_id, 1);
        assert!(handler.ready_queue.is_empty());
        assert_eq!(handler.pending_queue.len(), 1);

        let scheduled = handler.pending_queue.peek().unwrap().0;
        assert_eq!(scheduled.scheduled_at, current_tick);
        assert_eq!(scheduled.source_id, SourceId::new(0));
    }

    #[test]
    fn test_flush_pending() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        handler.add_source("s1", SimTime::from(10), TestSource { id: 1 });
        handler.add_source("s2", SimTime::from(5), TestSource { id: 2 });

        assert!(handler.ready_queue.is_empty());
        assert_eq!(handler.pending_queue.len(), 2);

        handler.flush_pending();

        assert_eq!(handler.ready_queue.len(), 2);
        assert!(handler.pending_queue.is_empty());

        // Check order in ready_queue (smallest scheduled_at first)
        let s1 = handler.ready_queue.pop().unwrap().0;
        let s2 = handler.ready_queue.pop().unwrap().0;

        assert_eq!(s1.scheduled_at, SimTime::from(5));
        assert_eq!(s2.scheduled_at, SimTime::from(10));
    }

    #[test]
    fn test_drain_ready() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        handler.add_source("s1", SimTime::from(10), TestSource { id: 1 });
        handler.add_source("s2", SimTime::from(5), TestSource { id: 2 });
        handler.add_source("s3", SimTime::from(10), TestSource { id: 3 });
        handler.add_source("s4", SimTime::from(15), TestSource { id: 4 });
        handler.flush_pending();

        // Drain at SimTime 5
        let ready_at_5 = handler.drain_ready(SimTime::from(5));
        assert_eq!(ready_at_5.len(), 1);
        assert_eq!(ready_at_5[0].source_id(), SourceId::new(1)); // s2 was scheduled at 5

        // Drain at SimTime 7
        let ready_at_7 = handler.drain_ready(SimTime::from(7));
        assert_eq!(ready_at_7.len(), 0);

        // Drain at SimTime 10
        let ready_at_10 = handler.drain_ready(SimTime::from(10));
        assert_eq!(ready_at_10.len(), 2);
        // Order might vary for same scheduled_at, but both s1 and s3 should be there
        let mut ids: Vec<SourceId> = ready_at_10.iter().map(|e| e.source_id()).collect();
        ids.sort_by_key(|id| id.value());
        assert_eq!(ids[0], SourceId::new(0)); // s1
        assert_eq!(ids[1], SourceId::new(2)); // s3

        // Drain at SimTime 15
        let ready_at_15 = handler.drain_ready(SimTime::from(15));
        assert_eq!(ready_at_15.len(), 1);
        assert_eq!(ready_at_15[0].source_id(), SourceId::new(3)); // s4

        // No more sources
        let ready_at_20 = handler.drain_ready(SimTime::from(20));
        assert!(ready_at_20.is_empty());
    }

    #[test]
    fn test_drain_ready_after_delay_zero() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        handler.add_source("s1", SimTime::from(10), TestSource { id: 1 });
        handler.add_source("s2", SimTime::from(10), TestSource { id: 2 });
        handler.flush_pending();

        // Drain at SimTime 10
        let ready_at_10 = handler.drain_ready(SimTime::from(10));
        assert_eq!(ready_at_10.len(), 2);
        // Order might vary for same scheduled_at, but both s1 and s2 should be there
        let mut ids: Vec<SourceId> = ready_at_10.iter().map(|e| e.source_id()).collect();
        ids.sort_by_key(|id| id.value());
        assert_eq!(ids[0], SourceId::new(0)); // s1
        assert_eq!(ids[1], SourceId::new(1)); // s2

        handler.add_source_after(
            "s3",
            SimTime::from(10),
            Duration::zero(),
            TestSource { id: 3 },
        );
        handler.flush_pending();
        let ready_at_10_after_zero = handler.drain_ready(SimTime::from(10));
        let mut ids: Vec<SourceId> = ready_at_10_after_zero
            .iter()
            .map(|e| e.source_id())
            .collect();
        ids.sort_by_key(|id| id.value());
        assert_eq!(ids[0], SourceId::new(2)); // s3
    }

    #[test]
    #[should_panic(
        expected = "SourceScheduler invariant violated: scheduled_source.scheduled_at=5 < now=10"
    )]
    fn test_drain_ready_invariant_violation() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        handler.add_source("s1", SimTime::from(5), TestSource { id: 1 });
        handler.flush_pending();

        // Try to drain at a time later than the scheduled source
        handler.drain_ready(SimTime::from(10));
    }

    #[test]
    fn test_get_by_source_id() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        handler.add_source("s1", SimTime::from(10), TestSource { id: 1 });
        handler.add_source("s2", SimTime::from(5), TestSource { id: 2 });

        let entry1 = handler.get_by_source_id(SourceId::new(0));
        assert_eq!(entry1.name.as_ref(), "s1");

        let entry2 = handler.get_by_source_id(SourceId::new(1));
        assert_eq!(entry2.name.as_ref(), "s2");
    }

    #[test]
    fn test_schedule_next() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        handler.add_source("s1", SimTime::from(10), TestSource { id: 1 });
        handler.flush_pending(); // Move to ready_queue

        let source_id = SourceId::new(0);
        let current_tick = SimTime::from(10);
        let next_delay = Duration::from(20);

        handler.schedule_next(current_tick, next_delay, source_id);

        assert_eq!(handler.pending_queue.len(), 1);
        let scheduled = handler.pending_queue.peek().unwrap().0;
        assert_eq!(scheduled.scheduled_at, current_tick + next_delay);
        assert_eq!(scheduled.source_id, source_id);
    }

    #[test]
    fn test_peek_next_time() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        assert_eq!(handler.peek_next_time(), None);

        handler.add_source("s1", SimTime::from(10), TestSource { id: 1 });
        handler.add_source("s2", SimTime::from(5), TestSource { id: 2 });

        assert_eq!(handler.peek(), None);

        handler.flush_pending();

        assert_eq!(handler.peek_next_time(), Some(SimTime::from(5)));

        handler.drain_ready(SimTime::from(5));
        assert_eq!(handler.peek_next_time(), Some(SimTime::from(10)));

        handler.drain_ready(SimTime::from(10));
        assert_eq!(handler.peek_next_time(), None);
    }

    #[test]
    fn test_peek() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        assert_eq!(handler.peek(), None);

        handler.add_source("s1", SimTime::from(10), TestSource { id: 1 });
        handler.add_source("s2", SimTime::from(5), TestSource { id: 2 });

        assert_eq!(handler.peek(), None);

        handler.flush_pending();

        let (time, scheduled_source) = handler.peek().unwrap();
        assert_eq!(time, SimTime::from(5));
        assert_eq!(scheduled_source.scheduled_at, SimTime::from(5));
        assert_eq!(scheduled_source.source_id, SourceId::new(1));

        handler.drain_ready(SimTime::from(5));
        let (time, scheduled_source) = handler.peek().unwrap();
        assert_eq!(time, SimTime::from(10));
        assert_eq!(scheduled_source.scheduled_at, SimTime::from(10));
        assert_eq!(scheduled_source.source_id, SourceId::new(0));

        handler.drain_ready(SimTime::from(10));
        assert_eq!(handler.peek(), None);
    }

    #[test]
    fn test_initialize_sources() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        handler.add_source("s1", SimTime::from(10), TestSource { id: 1 });
        handler.add_source("s2", SimTime::from(5), TestSource { id: 2 });

        let mut counter = 0;
        handler.initialize_sources(|entry| {
            // In a real scenario, you might modify the source or its internal state
            // For this test, we just count how many times the initializer is called
            assert!(entry.name.as_ref() == "s1" || entry.name.as_ref() == "s2");
            counter += 1;
        });
        assert_eq!(counter, 2);
    }

    #[test]
    fn cancel_single_source_from_pending_queue() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let now = SimTime::new(0);

        handler.add_source(
            "source_to_cancel",
            now + Duration::ticks(10),
            TestSource { id: 0 },
        );
        handler.add_source(
            "source_to_keep_1",
            now + Duration::ticks(20),
            TestSource { id: 1 },
        );
        handler.add_source(
            "source_to_keep_2",
            now + Duration::ticks(30),
            TestSource { id: 2 },
        );

        let cancelled_sources =
            handler.drain_cancel_scheduled(|_, entry| entry.name() == "source_to_cancel");

        assert_eq!(cancelled_sources.len(), 1);
        assert_eq!(cancelled_sources[0].1.name(), "source_to_cancel");

        // Verify remaining sources in pending queue
        handler.flush_pending();
        let ready_at_20 = handler.drain_ready(now + Duration::ticks(20));
        assert_eq!(ready_at_20.len(), 1);
        assert_eq!(ready_at_20[0].name(), "source_to_keep_1");

        let ready_at_30 = handler.drain_ready(now + Duration::ticks(30));
        assert_eq!(ready_at_30.len(), 1);
        assert_eq!(ready_at_30[0].name(), "source_to_keep_2");
    }

    #[test]
    fn cancel_single_source_from_ready_queue() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let now = SimTime::new(0);

        handler.add_source(
            "source_to_keep_1",
            now + Duration::ticks(10),
            TestSource { id: 0 },
        );
        handler.add_source(
            "source_to_cancel",
            now + Duration::ticks(20),
            TestSource { id: 1 },
        );
        handler.add_source(
            "source_to_keep_2",
            now + Duration::ticks(30),
            TestSource { id: 2 },
        );
        handler.flush_pending();

        let cancelled_sources =
            handler.drain_cancel_scheduled(|_, entry| entry.name() == "source_to_cancel");

        assert_eq!(cancelled_sources.len(), 1);
        assert_eq!(cancelled_sources[0].1.name(), "source_to_cancel");

        // Verify remaining sources in ready queue
        let ready_at_10 = handler.drain_ready(now + Duration::ticks(10));
        assert_eq!(ready_at_10.len(), 1);
        assert_eq!(ready_at_10[0].name(), "source_to_keep_1");

        let ready_at_30 = handler.drain_ready(now + Duration::ticks(30));
        assert_eq!(ready_at_30.len(), 1);
        assert_eq!(ready_at_30[0].name(), "source_to_keep_2");
    }

    #[test]
    fn cancel_multiple_sources() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let now = SimTime::new(0);

        handler.add_source(
            "cancel_me_1",
            now + Duration::ticks(10),
            TestSource { id: 0 },
        );
        handler.add_source("keep_me", now + Duration::ticks(20), TestSource { id: 1 });
        handler.add_source(
            "cancel_me_2",
            now + Duration::ticks(30),
            TestSource { id: 2 },
        );
        handler.add_source(
            "cancel_me_3",
            now + Duration::ticks(40),
            TestSource { id: 3 },
        );
        handler.flush_pending();

        let cancelled_sources =
            handler.drain_cancel_scheduled(|_, entry| entry.name().contains("cancel_me"));

        assert_eq!(cancelled_sources.len(), 3);
        let names: Vec<_> = cancelled_sources
            .into_iter()
            .map(|(_, entry)| entry.name().to_string())
            .collect();
        assert!(names.contains(&"cancel_me_1".to_string()));
        assert!(names.contains(&"cancel_me_2".to_string()));
        assert!(names.contains(&"cancel_me_3".to_string()));

        let ready_at_20 = handler.drain_ready(now + Duration::ticks(20));
        assert_eq!(ready_at_20.len(), 1);
        assert_eq!(ready_at_20[0].name(), "keep_me");
    }

    #[test]
    fn cancel_no_sources() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let now = SimTime::new(0);

        handler.add_source("source_1", now + Duration::ticks(10), TestSource { id: 0 });
        handler.add_source("source_2", now + Duration::ticks(20), TestSource { id: 1 });
        handler.flush_pending();

        let cancelled_sources =
            handler.drain_cancel_scheduled(|_, entry| entry.name() == "non_existent_source");

        assert!(cancelled_sources.is_empty());

        // Verify all sources are still present
        let ready_at_10 = handler.drain_ready(now + Duration::ticks(10));
        assert_eq!(ready_at_10.len(), 1);
        assert_eq!(ready_at_10[0].name(), "source_1");

        let ready_at_20 = handler.drain_ready(now + Duration::ticks(20));
        assert_eq!(ready_at_20.len(), 1);
        assert_eq!(ready_at_20[0].name(), "source_2");
    }

    #[test]
    fn cancel_all_sources() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let now = SimTime::new(0);

        handler.add_source("source_1", now + Duration::ticks(10), TestSource { id: 0 });
        handler.add_source("source_2", now + Duration::ticks(20), TestSource { id: 1 });
        handler.flush_pending();

        let cancelled_sources = handler.drain_cancel_scheduled(|_, _| true); // Cancel all

        assert_eq!(cancelled_sources.len(), 2);
        let names: Vec<_> = cancelled_sources
            .into_iter()
            .map(|(_, entry)| entry.name().to_string())
            .collect();
        assert!(names.contains(&"source_1".to_string()));
        assert!(names.contains(&"source_2".to_string()));

        // Verify no sources left
        let ready = handler.drain_ready(now + Duration::ticks(10));
        assert!(ready.is_empty());
        let ready = handler.drain_ready(now + Duration::ticks(20));
        assert!(ready.is_empty());
    }

    #[test]
    fn cancel_source_with_mixed_queues() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let now = SimTime::new(0);

        // Events in pending_queue initially
        handler.add_source(
            "pending_keep_1",
            now + Duration::ticks(10),
            TestSource { id: 0 },
        );
        handler.add_source(
            "pending_cancel_1",
            now + Duration::ticks(20),
            TestSource { id: 1 },
        );

        handler.flush_pending(); // Move pending_keep_1 and pending_cancel_1 to ready_queue

        // Events now in pending_queue
        handler.add_source(
            "pending_cancel_2",
            now + Duration::ticks(15),
            TestSource { id: 2 },
        );
        handler.add_source(
            "pending_keep_2",
            now + Duration::ticks(25),
            TestSource { id: 3 },
        );

        let cancelled_sources =
            handler.drain_cancel_scheduled(|_, entry| entry.name().contains("cancel"));

        assert_eq!(cancelled_sources.len(), 2);
        let names: Vec<_> = cancelled_sources
            .into_iter()
            .map(|(_, entry)| entry.name().to_string())
            .collect();
        assert!(names.contains(&"pending_cancel_1".to_string()));
        assert!(names.contains(&"pending_cancel_2".to_string()));

        // Verify remaining sources
        handler.flush_pending(); // Flush the remaining pending sources

        let ready_at_10 = handler.drain_ready(now + Duration::ticks(10));
        assert_eq!(ready_at_10.len(), 1);
        assert_eq!(ready_at_10[0].name(), "pending_keep_1");

        let ready_at_25 = handler.drain_ready(now + Duration::ticks(25));
        assert_eq!(ready_at_25.len(), 1);
        assert_eq!(ready_at_25[0].name(), "pending_keep_2");
    }

    #[test]
    fn cancel_source_by_id() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let now = SimTime::new(0);

        handler.add_source("source_1", now + Duration::ticks(10), TestSource { id: 0 }); // id 0
        handler.add_source("source_2", now + Duration::ticks(20), TestSource { id: 1 }); // id 1
        handler.add_source("source_3", now + Duration::ticks(30), TestSource { id: 2 }); // id 2
        handler.flush_pending();

        let source_to_cancel_id = SourceId::new(1); // Cancel source_2

        let cancelled_sources =
            handler.drain_cancel_scheduled(|_, entry| entry.source_id() == source_to_cancel_id);

        assert_eq!(cancelled_sources.len(), 1);
        assert_eq!(cancelled_sources[0].1.name(), "source_2");
        assert_eq!(cancelled_sources[0].1.source_id(), source_to_cancel_id);

        // Verify remaining sources
        let ready_at_10 = handler.drain_ready(now + Duration::ticks(10));
        assert_eq!(ready_at_10.len(), 1);
        assert_eq!(ready_at_10[0].name(), "source_1");

        let ready_at_30 = handler.drain_ready(now + Duration::ticks(30));
        assert_eq!(ready_at_30.len(), 1);
        assert_eq!(ready_at_30[0].name(), "source_3");
    }
}
