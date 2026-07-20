//! The `source_handler` module manages the registration, scheduling, and
//! execution of simulation sources.
//!
//! It maintains a registry of `SourceEntry` instances and uses priority queues
//! (`ready_queue` and `pending_queue`) to efficiently manage sources scheduled
//! for future execution. This module ensures that sources are fired at the
//! correct simulation time and provides mechanisms for dynamic scheduling
//! and cancellation.

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

/// Represents a simulation source scheduled for execution at a specific simulation time.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub(crate) struct ScheduledSource {
    /// The simulation time at which the source is scheduled to execute.
    pub(crate) scheduled_at: SimTime,
    /// The unique identifier of the source to be executed.
    pub(crate) source_id: SourceId,
}

/// An entry in the source registry, containing the source's name and its implementation.
pub(crate) struct SourceEntry<E, M: Model<E>> {
    /// The human-readable name of the source.
    pub(crate) name: Arc<str>,
    /// The boxed source implementation.
    pub(crate) source: Box<dyn Source<E, M>>,
}

/// Manages the scheduling and lifecycle of simulation sources.
pub(crate) struct SourceHandler<E, M: Model<E>> {
    source_registry: Vec<SourceEntry<E, M>>,
    next_source_id: usize,
    // Rust is a max-heap; Reverse allows ordering by time ascending.
    ready_queue: BinaryHeap<Reverse<ScheduledSource>>,
    pending_queue: BinaryHeap<Reverse<ScheduledSource>>,
}

impl<E, M: Model<E>> SourceHandler<E, M> {
    /// Creates a new, empty `SourceHandler`.
    pub fn new() -> SourceHandler<E, M> {
        SourceHandler {
            source_registry: vec![],
            next_source_id: 0,
            ready_queue: BinaryHeap::new(),
            pending_queue: BinaryHeap::new(),
        }
    }

    /// Converts a [ScheduledSource] into a [SourceReadyEntry] by retrieving its associated name from the registry.
    fn to_ready_entry(&self, scheduled: ScheduledSource) -> SourceReadyEntry {
        SourceReadyEntry::new(
            scheduled.source_id,
            Arc::clone(&self.source_registry[scheduled.source_id.value()].name),
        )
    }

    /// Iterates through all registered sources and executes the provided `initializer` function.
    /// If an initializer returns [Some(Duration)], the source is scheduled for initial firing
    /// at `SimTime::zero() + duration`.
    pub(crate) fn initialize_sources<F>(&mut self, mut initializer: F)
    where
        F: FnMut(&mut SourceEntry<E, M>) -> Option<Duration>,
    {
        self.source_registry.iter_mut().for_each(|e| {
            let duration_opt = initializer(e);
            if let Some(duration) = duration_opt {
                self.pending_queue.push(Reverse(ScheduledSource {
                    scheduled_at: SimTime::zero() + duration,
                    source_id: SourceId::new(self.next_source_id),
                }));
                self.next_source_id += 1;
            }
        });
    }

    /// Registers a [Source] to be initialized before the simulation starts.
    pub fn add_source_for_before_simulation<S>(&mut self, name: &'static str, source: S)
    where
        S: Source<E, M> + 'static,
    {
        self.source_registry.push(SourceEntry {
            name: Arc::from(name),
            source: Box::new(source),
        });
    }

    /// Registers a [Source] to be executed after a specified delay.
    pub fn add_source_after_registered_action<S>(
        &mut self,
        name: &'static str,
        current_tick: SimTime,
        delay: Option<Duration>,
        source: S,
    ) where
        S: Source<E, M> + 'static,
    {
        self.source_registry.push(SourceEntry {
            name: Arc::from(name),
            source: Box::new(source),
        });
        if let Some(delay) = delay {
            self.pending_queue.push(Reverse(ScheduledSource {
                scheduled_at: current_tick + delay,
                source_id: SourceId::new(self.next_source_id),
            }));
            self.next_source_id += 1;
        }
    }

    /// Pops sources that should fire at the current tick.
    ///
    /// # Note
    ///
    /// If a source is scheduled with [Duration::zero()], it may fire within the same
    /// time step. Repeated calls to `drain_ready` are required until the queue is
    /// empty to ensure all zero-delay sources are processed.
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

            // Collect them first so that they will not be processed even if they are processed
            // by each source and registered in the next microstep of now.
            let scheduled = self.ready_queue.pop().unwrap().0;
            fired_source_indexes.push_back(self.to_ready_entry(scheduled));
        }

        fired_source_indexes
    }

    /// Removes and returns scheduled sources that match the provided predicate from both queues.
    pub(crate) fn drain_cancel_scheduled<F>(
        &mut self,
        mut pred: F,
    ) -> Vec<(SimTime, SourceReadyEntry)>
    where
        F: FnMut(SimTime, &SourceReadyEntry) -> bool,
    {
        let mut cancelled = Vec::new();

        if self.pending_queue.iter().any(|Reverse(scheduled)| {
            pred(scheduled.scheduled_at, &self.to_ready_entry(*scheduled))
        }) {
            // Decompose the heap and extract it as a Vec
            let items = std::mem::take(&mut self.pending_queue).into_vec();
            let mut to_keep = Vec::with_capacity(items.len());

            // Sort by whether it meets the conditions
            for Reverse(scheduled) in items {
                if pred(scheduled.scheduled_at, &self.to_ready_entry(scheduled)) {
                    cancelled.push(scheduled);
                } else {
                    to_keep.push(Reverse(scheduled));
                }
            }

            // Rebuild heap with remaining elements
            self.pending_queue = BinaryHeap::from(to_keep);
        }

        if self.ready_queue.iter().any(|Reverse(scheduled)| {
            pred(scheduled.scheduled_at, &self.to_ready_entry(*scheduled))
        }) {
            // Decompose the heap and extract it as a Vec
            let items = std::mem::take(&mut self.ready_queue).into_vec();
            let mut to_keep = Vec::with_capacity(items.len());

            // Sort by whether it meets the conditions
            for Reverse(scheduled) in items {
                if pred(scheduled.scheduled_at, &self.to_ready_entry(scheduled)) {
                    cancelled.push(scheduled);
                } else {
                    to_keep.push(Reverse(scheduled));
                }
            }

            // Rebuild heap with remaining elements
            self.ready_queue = BinaryHeap::from(to_keep);
        }

        // Arrange them in firing order for ease of handling
        cancelled.sort();
        cancelled
            .into_iter()
            .map(|scheduled| (scheduled.scheduled_at, self.to_ready_entry(scheduled)))
            .collect()
    }

    /// Returns a mutable reference to the `SourceEntry` identified by the given `source_id`.
    pub(crate) fn get_by_source_id(&mut self, source_id: SourceId) -> &mut SourceEntry<E, M> {
        &mut self.source_registry[source_id.value()]
    }

    /// Schedules a source for its next execution after a specific delay.
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

    /// Returns the scheduled time of the next source in the ready queue.
    pub fn peek_next_time(&self) -> Option<SimTime> {
        self.ready_queue.peek().map(|i| i.0.scheduled_at)
    }

    /// Peeks at the next scheduled source in the ready queue.
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn peek(&self) -> Option<(SimTime, &ScheduledSource)> {
        self.ready_queue.peek().map(|i| (i.0.scheduled_at, &i.0))
    }

    /// Returns the number of sources in the ready queue.
    #[cfg(test)]
    pub fn ready_queue_len(&self) -> usize {
        self.ready_queue.len()
    }

    /// Transfers all pending sources into the ready queue.
    pub fn flush_pending(&mut self) {
        self.ready_queue.append(&mut self.pending_queue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{EventContext, SourceContext, UserContext};
    use crate::modeling::event::{Event, EventPriority};
    use crate::modeling::model::Model;
    use crate::modeling::source::Source;
    use crate::primitive::time::{Duration, MicroStep, SimTime};

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
            // No-op for testing
        }
    }

    // Dummy UserContext for testing
    struct UserContextImpl;

    impl<E, M: Model<E>> UserContext<E, M> for UserContextImpl {
        fn current_tick(&self) -> SimTime {
            SimTime::from_ticks(0)
        }

        fn current_micro_step(&self) -> MicroStep {
            MicroStep::zero()
        }

        fn schedule_event(
            &mut self,
            _delay: Duration,
            _priority: EventPriority,
            _event_payload: E,
        ) {
            // No-op for testing
        }
    }

    // Dummy Source for testing
    struct TestSource {
        #[allow(unused)]
        id: usize,
        initial_delay: Duration,
    }

    impl Source<TestEvent, TestModel> for TestSource {
        fn on_registered(
            &mut self,
            _context: &mut dyn UserContext<TestEvent, TestModel>,
            _model: &TestModel,
        ) -> Option<Duration> {
            Some(self.initial_delay)
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
        assert!(handler.pending_queue.is_empty());
    }

    #[test]
    fn test_add_source_for_before_simulation() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let source1 = TestSource {
            id: 1,
            initial_delay: Duration::ticks(10),
        };
        let source2 = TestSource {
            id: 2,
            initial_delay: Duration::ticks(5),
        };

        handler.add_source_for_before_simulation("source1", source1);
        handler.add_source_for_before_simulation("source2", source2);

        assert_eq!(handler.source_registry.len(), 2);
        assert!(handler.ready_queue.is_empty());
        assert!(handler.pending_queue.is_empty());
    }

    #[test]
    fn test_initialize_sources() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        handler.add_source_for_before_simulation(
            "s1",
            TestSource {
                id: 1,
                initial_delay: Duration::ticks(10),
            },
        );
        handler.add_source_for_before_simulation(
            "s2",
            TestSource {
                id: 2,
                initial_delay: Duration::ticks(5),
            },
        );

        let mut dummy_context = UserContextImpl;
        let dummy_model = TestModel;
        handler.initialize_sources(|entry| {
            entry.source.on_registered(&mut dummy_context, &dummy_model)
        });

        assert_eq!(handler.source_registry.len(), 2);
        assert_eq!(handler.pending_queue.len(), 2);

        let mut pending_sources: Vec<ScheduledSource> =
            handler.pending_queue.iter().map(|s| s.0).collect();
        pending_sources.sort_by_key(|s| s.scheduled_at);

        assert_eq!(pending_sources.len(), 2);
        assert_eq!(pending_sources[0].scheduled_at, SimTime::from(5));
        assert_eq!(pending_sources[0].source_id, SourceId::new(1));
        assert_eq!(pending_sources[1].scheduled_at, SimTime::from(10));
        assert_eq!(pending_sources[1].source_id, SourceId::new(0));
    }

    #[test]
    fn test_add_source_after_registered_action_with_delay() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let current_tick = SimTime::from(100);
        let delay = Duration::from(50);
        let source = TestSource {
            id: 1,
            initial_delay: Duration::ticks(0),
        };

        handler.add_source_after_registered_action(
            "s_after_delay",
            current_tick,
            Some(delay),
            source,
        );

        assert_eq!(handler.source_registry.len(), 1);
        assert_eq!(handler.pending_queue.len(), 1);

        let scheduled = handler
            .pending_queue
            .peek()
            .expect("Queue should not be empty")
            .0;
        assert_eq!(scheduled.scheduled_at, current_tick + delay);
        assert_eq!(scheduled.source_id, SourceId::new(0));
    }

    #[test]
    fn test_add_source_after_registered_action_no_delay() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let current_tick = SimTime::from(100);
        let source = TestSource {
            id: 1,
            initial_delay: Duration::ticks(0),
        };

        handler.add_source_after_registered_action("s_no_delay", current_tick, None, source);

        assert_eq!(handler.source_registry.len(), 1);
        assert_eq!(handler.pending_queue.len(), 0);
    }

    #[test]
    fn test_add_source_after_registered_action_multiple() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let current_tick = SimTime::from(100);

        // SourceId 0, scheduled at 120
        handler.add_source_after_registered_action(
            "s_delay_1",
            current_tick,
            Some(Duration::ticks(20)),
            TestSource {
                id: 1,
                initial_delay: Duration::ticks(0),
            },
        );

        // SourceId 1, not scheduled
        handler.add_source_after_registered_action(
            "s_no_delay_2",
            current_tick,
            None,
            TestSource {
                id: 2,
                initial_delay: Duration::ticks(0),
            },
        );

        // SourceId 2, scheduled at 110
        handler.add_source_after_registered_action(
            "s_delay_3",
            current_tick,
            Some(Duration::ticks(10)),
            TestSource {
                id: 3,
                initial_delay: Duration::ticks(0),
            },
        );

        assert_eq!(handler.source_registry.len(), 3);
        assert_eq!(handler.pending_queue.len(), 2);

        let mut pending_sources: Vec<ScheduledSource> =
            handler.pending_queue.iter().map(|s| s.0).collect();
        pending_sources.sort_by_key(|s| s.scheduled_at);

        assert_eq!(pending_sources[0].scheduled_at, SimTime::from(110));
        assert_eq!(pending_sources[0].source_id, SourceId::new(1)); // Corresponds to s_delay_3

        assert_eq!(pending_sources[1].scheduled_at, SimTime::from(120));
        assert_eq!(pending_sources[1].source_id, SourceId::new(0)); // Corresponds to s_delay_1
    }

    #[test]
    fn test_add_source_after_registered_action_zero_delay() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let current_tick = SimTime::from(100);
        let delay = Duration::zero();
        let source = TestSource {
            id: 1,
            initial_delay: Duration::ticks(0),
        };

        handler.add_source_after_registered_action(
            "s_zero_delay",
            current_tick,
            Some(delay),
            source,
        );

        assert_eq!(handler.source_registry.len(), 1);
        assert_eq!(handler.next_source_id, 1);
        assert!(handler.ready_queue.is_empty());
        assert_eq!(handler.pending_queue.len(), 1);

        let scheduled = handler
            .pending_queue
            .peek()
            .expect("Queue should not be empty")
            .0;
        assert_eq!(scheduled.scheduled_at, current_tick + delay);
        assert_eq!(scheduled.source_id, SourceId::new(0));
    }

    #[test]
    fn test_flush_pending() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        handler.add_source_for_before_simulation(
            "s1",
            TestSource {
                id: 1,
                initial_delay: Duration::ticks(10),
            },
        );
        handler.add_source_for_before_simulation(
            "s2",
            TestSource {
                id: 2,
                initial_delay: Duration::ticks(5),
            },
        );

        let mut dummy_context = UserContextImpl;
        let dummy_model = TestModel;
        handler.initialize_sources(|entry| {
            entry.source.on_registered(&mut dummy_context, &dummy_model)
        });

        assert!(handler.ready_queue.is_empty());
        assert_eq!(handler.pending_queue.len(), 2);

        handler.flush_pending();

        assert_eq!(handler.ready_queue.len(), 2);
        assert!(handler.pending_queue.is_empty());

        // Check order in ready_queue (smallest scheduled_at first)
        let s1 = handler
            .ready_queue
            .pop()
            .expect("Queue should not be empty")
            .0;
        let s2 = handler
            .ready_queue
            .pop()
            .expect("Queue should not be empty")
            .0;

        assert_eq!(s1.scheduled_at, SimTime::from(5));
        assert_eq!(s1.source_id, SourceId::new(1));
        assert_eq!(s2.scheduled_at, SimTime::from(10));
        assert_eq!(s2.source_id, SourceId::new(0));
    }

    #[test]
    fn test_drain_ready() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        handler.add_source_for_before_simulation(
            "s1",
            TestSource {
                id: 1,
                initial_delay: Duration::ticks(10),
            },
        );
        handler.add_source_for_before_simulation(
            "s2",
            TestSource {
                id: 2,
                initial_delay: Duration::ticks(5),
            },
        );
        handler.add_source_for_before_simulation(
            "s3",
            TestSource {
                id: 3,
                initial_delay: Duration::ticks(10),
            },
        );
        handler.add_source_for_before_simulation(
            "s4",
            TestSource {
                id: 4,
                initial_delay: Duration::ticks(15),
            },
        );

        let mut dummy_context = UserContextImpl;
        let dummy_model = TestModel;
        handler.initialize_sources(|entry| {
            entry.source.on_registered(&mut dummy_context, &dummy_model)
        });
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
        handler.add_source_for_before_simulation(
            "s1",
            TestSource {
                id: 1,
                initial_delay: Duration::ticks(10),
            },
        );
        handler.add_source_for_before_simulation(
            "s2",
            TestSource {
                id: 2,
                initial_delay: Duration::ticks(10),
            },
        );

        let mut dummy_context = UserContextImpl;
        let dummy_model = TestModel;
        handler.initialize_sources(|entry| {
            entry.source.on_registered(&mut dummy_context, &dummy_model)
        });
        handler.flush_pending();

        // Drain at SimTime 10
        let ready_at_10 = handler.drain_ready(SimTime::from(10));
        assert_eq!(ready_at_10.len(), 2);
        // Order might vary for same scheduled_at, but both s1 and s2 should be there
        let mut ids: Vec<SourceId> = ready_at_10.iter().map(|e| e.source_id()).collect();
        ids.sort_by_key(|id| id.value());
        assert_eq!(ids[0], SourceId::new(0)); // s1
        assert_eq!(ids[1], SourceId::new(1)); // s2

        handler.add_source_after_registered_action(
            "s3",
            SimTime::from(10),
            Some(Duration::zero()),
            TestSource {
                id: 3,
                initial_delay: Duration::ticks(0),
            },
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
        handler.add_source_for_before_simulation(
            "s1",
            TestSource {
                id: 1,
                initial_delay: Duration::ticks(5),
            },
        );

        let mut dummy_context = UserContextImpl;
        let dummy_model = TestModel;
        handler.initialize_sources(|entry| {
            entry.source.on_registered(&mut dummy_context, &dummy_model)
        });
        handler.flush_pending();

        // Try to drain at a time later than the scheduled source
        handler.drain_ready(SimTime::from(10));
    }

    #[test]
    fn test_get_by_source_id() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        handler.add_source_for_before_simulation(
            "s1",
            TestSource {
                id: 1,
                initial_delay: Duration::ticks(10),
            },
        );
        handler.add_source_for_before_simulation(
            "s2",
            TestSource {
                id: 2,
                initial_delay: Duration::ticks(5),
            },
        );

        let mut dummy_context = UserContextImpl;
        let dummy_model = TestModel;
        handler.initialize_sources(|entry| {
            entry.source.on_registered(&mut dummy_context, &dummy_model)
        });
        handler.flush_pending();

        let entry1 = handler.get_by_source_id(SourceId::new(0));
        assert_eq!(entry1.name.as_ref(), "s1");

        let entry2 = handler.get_by_source_id(SourceId::new(1));
        assert_eq!(entry2.name.as_ref(), "s2");
    }

    #[test]
    fn test_schedule_next() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        handler.add_source_for_before_simulation(
            "s1",
            TestSource {
                id: 1,
                initial_delay: Duration::ticks(10),
            },
        );

        let mut dummy_context = UserContextImpl;
        let dummy_model = TestModel;
        handler.initialize_sources(|entry| {
            entry.source.on_registered(&mut dummy_context, &dummy_model)
        });
        handler.flush_pending();

        let source_id = SourceId::new(0);
        let current_tick = SimTime::from(10);
        let next_delay = Duration::from(20);

        handler.schedule_next(current_tick, next_delay, source_id);

        assert_eq!(handler.pending_queue.len(), 1);
        let scheduled = handler
            .pending_queue
            .peek()
            .expect("Queue should not be empty")
            .0;
        assert_eq!(scheduled.scheduled_at, current_tick + next_delay);
        assert_eq!(scheduled.source_id, source_id);
    }

    #[test]
    fn test_peek_next_time() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        assert_eq!(handler.peek_next_time(), None);

        handler.add_source_for_before_simulation(
            "s1",
            TestSource {
                id: 1,
                initial_delay: Duration::ticks(10),
            },
        );
        handler.add_source_for_before_simulation(
            "s2",
            TestSource {
                id: 2,
                initial_delay: Duration::ticks(5),
            },
        );

        let mut dummy_context = UserContextImpl;
        let dummy_model = TestModel;
        handler.initialize_sources(|entry| {
            entry.source.on_registered(&mut dummy_context, &dummy_model)
        });

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
        assert!(handler.peek().is_none());

        handler.add_source_for_before_simulation(
            "s1",
            TestSource {
                id: 1,
                initial_delay: Duration::ticks(10),
            },
        );
        handler.add_source_for_before_simulation(
            "s2",
            TestSource {
                id: 2,
                initial_delay: Duration::ticks(5),
            },
        );

        let mut dummy_context = UserContextImpl;
        let dummy_model = TestModel;
        handler.initialize_sources(|entry| {
            entry.source.on_registered(&mut dummy_context, &dummy_model)
        });

        assert_eq!(handler.peek(), None);

        handler.flush_pending();

        let (time, scheduled_source) = handler.peek().expect("Peek should return value");
        assert_eq!(time, SimTime::from(5));
        assert_eq!(scheduled_source.scheduled_at, SimTime::from(5));
        assert_eq!(scheduled_source.source_id, SourceId::new(1));

        handler.drain_ready(SimTime::from(5));
        let (time, scheduled_source) = handler.peek().expect("Peek should return value");
        assert_eq!(time, SimTime::from(10));
        assert_eq!(scheduled_source.scheduled_at, SimTime::from(10));
        assert_eq!(scheduled_source.source_id, SourceId::new(0));

        handler.drain_ready(SimTime::from(10));
        assert_eq!(handler.peek(), None);
    }

    #[test]
    fn cancel_single_source_from_pending_queue() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let now = SimTime::from_ticks(0);

        handler.add_source_for_before_simulation(
            "source_to_cancel",
            TestSource {
                id: 0,
                initial_delay: Duration::ticks(10),
            },
        );
        handler.add_source_for_before_simulation(
            "source_to_keep_1",
            TestSource {
                id: 1,
                initial_delay: Duration::ticks(20),
            },
        );
        handler.add_source_for_before_simulation(
            "source_to_keep_2",
            TestSource {
                id: 2,
                initial_delay: Duration::ticks(30),
            },
        );

        let mut dummy_context = UserContextImpl;
        let dummy_model = TestModel;
        handler.initialize_sources(|entry| {
            entry.source.on_registered(&mut dummy_context, &dummy_model)
        });
        // Note: flush_pending not called here to test pending queue cancellation

        let cancelled_sources =
            handler.drain_cancel_scheduled(|_, entry| entry.name() == "source_to_cancel");
        assert_eq!(cancelled_sources.len(), 1);
        assert_eq!(cancelled_sources[0].1.name(), "source_to_cancel");
        assert_eq!(cancelled_sources[0].1.source_id(), SourceId::new(0));

        // Verify remaining sources in pending queue after flush
        handler.flush_pending();
        let ready_at_20 = handler.drain_ready(now + Duration::ticks(20));
        assert_eq!(ready_at_20.len(), 1);
        assert_eq!(ready_at_20[0].name(), "source_to_keep_1");
        assert_eq!(ready_at_20[0].source_id(), SourceId::new(1));

        let ready_at_30 = handler.drain_ready(now + Duration::ticks(30));
        assert_eq!(ready_at_30.len(), 1);
        assert_eq!(ready_at_30[0].name(), "source_to_keep_2");
        assert_eq!(ready_at_30[0].source_id(), SourceId::new(2));
    }

    #[test]
    fn cancel_single_source_from_ready_queue() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let now = SimTime::from_ticks(0);

        handler.add_source_for_before_simulation(
            "source_to_keep_1",
            TestSource {
                id: 0,
                initial_delay: Duration::ticks(10),
            },
        );
        handler.add_source_for_before_simulation(
            "source_to_cancel",
            TestSource {
                id: 1,
                initial_delay: Duration::ticks(20),
            },
        );
        handler.add_source_for_before_simulation(
            "source_to_keep_2",
            TestSource {
                id: 2,
                initial_delay: Duration::ticks(30),
            },
        );
        let mut dummy_context = UserContextImpl;
        let dummy_model = TestModel;
        handler.initialize_sources(|entry| {
            entry.source.on_registered(&mut dummy_context, &dummy_model)
        });
        handler.flush_pending();

        let cancelled_sources =
            handler.drain_cancel_scheduled(|_, entry| entry.name() == "source_to_cancel");
        assert_eq!(cancelled_sources.len(), 1);
        assert_eq!(cancelled_sources[0].1.name(), "source_to_cancel");
        assert_eq!(cancelled_sources[0].1.source_id(), SourceId::new(1));

        // Verify remaining sources in ready queue
        let ready_at_10 = handler.drain_ready(now + Duration::ticks(10));
        assert_eq!(ready_at_10.len(), 1);
        assert_eq!(ready_at_10[0].name(), "source_to_keep_1");
        assert_eq!(ready_at_10[0].source_id(), SourceId::new(0));

        let ready_at_30 = handler.drain_ready(now + Duration::ticks(30));
        assert_eq!(ready_at_30.len(), 1);
        assert_eq!(ready_at_30[0].name(), "source_to_keep_2");
        assert_eq!(ready_at_30[0].source_id(), SourceId::new(2));
    }

    #[test]
    fn cancel_multiple_sources() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let now = SimTime::from_ticks(0);

        handler.add_source_for_before_simulation(
            "cancel_me_1",
            TestSource {
                id: 0,
                initial_delay: Duration::ticks(10),
            },
        );
        handler.add_source_for_before_simulation(
            "keep_me",
            TestSource {
                id: 1,
                initial_delay: Duration::ticks(20),
            },
        );
        handler.add_source_for_before_simulation(
            "cancel_me_2",
            TestSource {
                id: 2,
                initial_delay: Duration::ticks(30),
            },
        );
        handler.add_source_for_before_simulation(
            "cancel_me_3",
            TestSource {
                id: 3,
                initial_delay: Duration::ticks(40),
            },
        );
        let mut dummy_context = UserContextImpl;
        let dummy_model = TestModel;
        handler.initialize_sources(|entry| {
            entry.source.on_registered(&mut dummy_context, &dummy_model)
        });
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
        assert_eq!(ready_at_20[0].source_id(), SourceId::new(1));
    }

    #[test]
    fn cancel_no_sources() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let now = SimTime::from_ticks(0);

        handler.add_source_for_before_simulation(
            "source_1",
            TestSource {
                id: 0,
                initial_delay: Duration::ticks(10),
            },
        );
        handler.add_source_for_before_simulation(
            "source_2",
            TestSource {
                id: 1,
                initial_delay: Duration::ticks(20),
            },
        );

        let mut dummy_context = UserContextImpl;
        let dummy_model = TestModel;
        handler.initialize_sources(|entry| {
            entry.source.on_registered(&mut dummy_context, &dummy_model)
        });
        handler.flush_pending();

        let cancelled_sources =
            handler.drain_cancel_scheduled(|_, entry| entry.name() == "non_existent_source");
        assert!(cancelled_sources.is_empty());

        // Verify all sources are still present
        let ready_at_10 = handler.drain_ready(now + Duration::ticks(10));
        assert_eq!(ready_at_10.len(), 1);
        assert_eq!(ready_at_10[0].name(), "source_1");
        assert_eq!(ready_at_10[0].source_id(), SourceId::new(0));

        let ready_at_20 = handler.drain_ready(now + Duration::ticks(20));
        assert_eq!(ready_at_20.len(), 1);
        assert_eq!(ready_at_20[0].name(), "source_2");
        assert_eq!(ready_at_20[0].source_id(), SourceId::new(1));
    }

    #[test]
    fn cancel_all_sources() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let now = SimTime::from_ticks(0);

        handler.add_source_for_before_simulation(
            "source_1",
            TestSource {
                id: 0,
                initial_delay: Duration::ticks(10),
            },
        );
        handler.add_source_for_before_simulation(
            "source_2",
            TestSource {
                id: 1,
                initial_delay: Duration::ticks(20),
            },
        );

        let mut dummy_context = UserContextImpl;
        let dummy_model = TestModel;
        handler.initialize_sources(|entry| {
            entry.source.on_registered(&mut dummy_context, &dummy_model)
        });
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
        let now = SimTime::from_ticks(0);

        handler.add_source_for_before_simulation(
            "pending_keep_1",
            TestSource {
                id: 0,
                initial_delay: Duration::ticks(10),
            },
        );
        handler.add_source_for_before_simulation(
            "pending_cancel_1",
            TestSource {
                id: 1,
                initial_delay: Duration::ticks(20),
            },
        );

        let mut dummy_context = UserContextImpl;
        let dummy_model = TestModel;
        handler.initialize_sources(|entry| {
            entry.source.on_registered(&mut dummy_context, &dummy_model)
        });
        handler.flush_pending(); // Move pending_keep_1 and pending_cancel_1 to ready_queue

        // Events now in pending_queue
        handler.add_source_after_registered_action(
            "pending_cancel_2",
            now,
            Some(Duration::ticks(15)),
            TestSource {
                id: 2,
                initial_delay: Duration::ticks(15),
            },
        );
        handler.add_source_after_registered_action(
            "pending_keep_2",
            now,
            Some(Duration::ticks(25)),
            TestSource {
                id: 3,
                initial_delay: Duration::ticks(25),
            },
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
        handler.flush_pending();

        let ready_at_10 = handler.drain_ready(now + Duration::ticks(10));
        assert_eq!(ready_at_10.len(), 1);
        assert_eq!(ready_at_10[0].name(), "pending_keep_1");
        assert_eq!(ready_at_10[0].source_id(), SourceId::new(0));

        let ready_at_25 = handler.drain_ready(now + Duration::ticks(25));
        assert_eq!(ready_at_25.len(), 1);
        assert_eq!(ready_at_25[0].name(), "pending_keep_2");
        assert_eq!(ready_at_25[0].source_id(), SourceId::new(3));
    }

    #[test]
    fn cancel_source_by_id() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let now = SimTime::from_ticks(0);

        handler.add_source_for_before_simulation(
            "source_1",
            TestSource {
                id: 0,
                initial_delay: Duration::ticks(10),
            },
        );
        handler.add_source_for_before_simulation(
            "source_2",
            TestSource {
                id: 1,
                initial_delay: Duration::ticks(20),
            },
        );
        handler.add_source_for_before_simulation(
            "source_3",
            TestSource {
                id: 2,
                initial_delay: Duration::ticks(30),
            },
        );

        let mut dummy_context = UserContextImpl;
        let dummy_model = TestModel;
        handler.initialize_sources(|entry| {
            entry.source.on_registered(&mut dummy_context, &dummy_model)
        });
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
        assert_eq!(ready_at_10[0].source_id(), SourceId::new(0));

        let ready_at_30 = handler.drain_ready(now + Duration::ticks(30));
        assert_eq!(ready_at_30.len(), 1);
        assert_eq!(ready_at_30[0].name(), "source_3");
        assert_eq!(ready_at_30[0].source_id(), SourceId::new(2));
    }

    #[test]
    fn test_deterministic_ordering_for_same_sim_time() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();

        // Register sources scheduled at the same time (SimTime: 10)
        // with different registration orders and different SourceIds
        handler.add_source_for_before_simulation(
            "source_0",
            TestSource {
                id: 0,
                initial_delay: Duration::ticks(10),
            },
        );
        handler.add_source_for_before_simulation(
            "source_1",
            TestSource {
                id: 1,
                initial_delay: Duration::ticks(10),
            },
        );

        let mut dummy_context = UserContextImpl;
        let dummy_model = TestModel;
        handler.initialize_sources(|entry| {
            entry.source.on_registered(&mut dummy_context, &dummy_model)
        });

        // Flush from pending_queue to ready_queue
        handler.flush_pending();

        // Get elements at the same time
        let mut ready_sources = handler.drain_ready(SimTime::from_ticks(10));
        assert_eq!(ready_sources.len(), 2);

        // By implementing BinaryHeap and Reverse(ScheduledSource), if the times are the same,
        // the one with the smaller `SourceId` will always be popped first (guaranteeing determinism)
        let first = ready_sources
            .pop_front()
            .expect("Should have first element");
        let second = ready_sources
            .pop_front()
            .expect("Should have second element");

        assert_eq!(first.source_id(), SourceId::new(0));
        assert_eq!(first.name(), "source_0");

        assert_eq!(second.source_id(), SourceId::new(1));
        assert_eq!(second.name(), "source_1");
    }

    #[test]
    fn test_cascading_registration_during_simulation_flow() {
        let mut handler: SourceHandler<TestEvent, TestModel> = SourceHandler::new();
        let now = SimTime::from_ticks(0);

        handler.add_source_for_before_simulation(
            "trigger_source",
            TestSource {
                id: 0,
                initial_delay: Duration::ticks(10),
            },
        );

        let mut dummy_context = UserContextImpl;
        let dummy_model = TestModel;
        handler.initialize_sources(|entry| {
            entry.source.on_registered(&mut dummy_context, &dummy_model)
        });
        handler.flush_pending();

        // Simulating a case where a new source is dynamically registered in a chain
        // while processing the event "current time 10"
        let ready_at_10 = handler.drain_ready(now + Duration::ticks(10));
        assert_eq!(ready_at_10.len(), 1);

        // Dynamic addition processing (add_source_after_registered_action)
        // At this point, it is accumulated only in pending_queue and does not pollute the processing order of ready_queue (buffering)
        handler.add_source_after_registered_action(
            "cascaded_source_immediate",
            now + Duration::ticks(10),
            Some(Duration::zero()),
            TestSource {
                id: 1,
                initial_delay: Duration::zero(),
            },
        );
        handler.add_source_after_registered_action(
            "cascaded_source_delayed",
            now + Duration::ticks(10),
            Some(Duration::ticks(5)),
            TestSource {
                id: 2,
                initial_delay: Duration::zero(),
            },
        );

        // Since it has not been flushed yet, even if you drain it again at the same time (10),
        // the dynamically added items cannot be removed.
        let ready_at_10_retry = handler.drain_ready(now + Duration::ticks(10));
        assert!(ready_at_10_retry.is_empty());

        handler.flush_pending();

        // You can get the instant chain source scheduled at the same time (10) here
        let mut ready_at_10_post_flush = handler.drain_ready(now + Duration::ticks(10));
        assert_eq!(ready_at_10_post_flush.len(), 1);
        assert_eq!(
            ready_at_10_post_flush
                .pop_front()
                .expect("Should exist")
                .name(),
            "cascaded_source_immediate"
        );

        // The delayed version can be retrieved correctly at time 15.
        let mut ready_at_15 = handler.drain_ready(now + Duration::ticks(15));
        assert_eq!(ready_at_15.len(), 1);
        assert_eq!(
            ready_at_15.pop_front().expect("Should exist").name(),
            "cascaded_source_delayed"
        );
    }
}
