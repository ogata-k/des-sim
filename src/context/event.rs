use crate::context::UserContext;
use crate::event_scheduler::EventScheduler;
use crate::modeling::event::{Event, EventPriority};
use crate::modeling::hook::Hook;
use crate::modeling::hook::instance::HookDelegate;
use crate::modeling::model::Model;
use crate::modeling::source::Source;
use crate::primitive::time::{Duration, MicroStep, MicroStepStatus, SimTime, TickStatus};
use crate::source_handler::{SourceHandler, SourceReadyEntry, SourceView};

/// Holds and manages context information during event processing.
///
/// This provides access to the current simulation time, micro-step status,
/// hook management, source handling, and the event scheduler during model execution.
pub struct EventContext<E, M: Model<E>> {
    pub(crate) current_tick_status: TickStatus,
    pub(crate) current_micro_step_status: MicroStepStatus,
    pub(crate) hook_delegate: HookDelegate<E, M>,
    pub(crate) source_handler: SourceHandler<E, M>,
    pub(crate) event_scheduler: EventScheduler<E>,
}

impl<E, M: Model<E>> UserContext<E, M> for EventContext<E, M> {
    fn current_tick(&self) -> SimTime {
        self.current_tick_status.current()
    }

    fn current_micro_step(&self) -> MicroStep {
        self.current_micro_step_status.current()
    }

    fn schedule_event(&mut self, delay: Duration, priority: EventPriority, event_payload: E) {
        self.event_scheduler
            .schedule(self.current_tick(), delay, priority, event_payload);
    }
}

impl<E, M: Model<E>> EventContext<E, M> {
    /// Retrieves the hooks associated with the current event processing.
    pub(crate) fn hook(&self) -> &impl Hook<E, M> {
        &self.hook_delegate
    }

    /// Registers a new source and adds it to the event processing loop.
    ///
    /// # Arguments
    /// * `model` - The model associated with the source.
    /// * `name` - A unique identifier for the source.
    /// * `source` - The source implementation to register.
    pub fn add_source<S>(&mut self, model: &M, name: &'static str, mut source: S)
    where
        S: Source<E, M> + 'static,
    {
        self.hook().before_register_source(model, name);
        let first_fire_delay = source.on_registered(self, model);
        self.source_handler.add_source_after_registered_action(
            name,
            self.current_tick(),
            first_fire_delay,
            source,
        );
        self.hook().after_register_source(model, name);
    }

    /// Cancels scheduled sources that satisfy the provided condition.
    ///
    /// # Arguments
    /// * `model` - The associated model.
    /// * `pred` - A predicate function to determine if a source should be canceled.
    ///
    /// # Returns
    /// A `Vec` containing information about the canceled sources.
    pub fn cancel_scheduled_sources<S, F>(
        &mut self,
        model: &M,
        pred: F,
    ) -> Vec<(SimTime, SourceReadyEntry)>
    where
        S: Source<E, M> + 'static,
        F: FnMut(SimTime, &SourceReadyEntry) -> bool,
    {
        let mut result = Vec::new();
        let now = self.current_tick();
        let micro_step = self.current_micro_step();
        let canceled = self.source_handler.drain_cancel_scheduled(pred);

        canceled.into_iter().for_each(|(scheduled_at, entry)| {
            self.hook().cancel_source(
                model,
                now,
                micro_step,
                scheduled_at,
                &SourceView::new(entry.source_id(), entry.clone_name_arc()),
            );
            result.push((scheduled_at, entry));
        });

        result
    }

    /// Cancels scheduled events that satisfy the provided condition.
    ///
    /// # Arguments
    /// * `model` - The associated model.
    /// * `pred` - A predicate function to determine if an event should be canceled.
    ///
    /// # Returns
    /// A `Vec` containing information about the canceled events.
    pub fn cancel_scheduled_events<F>(&mut self, model: &M, pred: F) -> Vec<(SimTime, Event<E>)>
    where
        F: FnMut(SimTime, &Event<E>) -> bool,
    {
        let mut result = Vec::new();
        let now = self.current_tick();
        let micro_step = self.current_micro_step();
        let canceled = self.event_scheduler.drain_cancel_scheduled(pred);

        canceled.into_iter().for_each(|(scheduled_at, event)| {
            self.hook()
                .cancel_event(model, now, micro_step, scheduled_at, &event);
            result.push((scheduled_at, event));
        });

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::SourceContext;
    use crate::modeling::hook::instance::SharedHook;
    use std::cell::RefCell;
    use std::fmt::Debug;
    use std::rc::Rc;

    /// Event structure for testing purposes.
    #[derive(Debug, Clone, PartialEq)]
    struct TestEvent;

    /// Model definition for testing purposes.
    struct TestModel;

    impl Model<TestEvent> for TestModel {
        fn handle_event(
            &mut self,
            _event_context: &mut EventContext<TestEvent, Self>,
            _event: &Event<TestEvent>,
        ) {
            // No-op for test purposes.
        }
    }

    /// Source implementation for testing purposes.
    struct TestSource {
        initial_delay: Duration,
    }

    impl Source<TestEvent, TestModel> for TestSource {
        fn on_registered(
            &mut self,
            context: &mut dyn UserContext<TestEvent, TestModel>,
            _model: &TestModel,
        ) -> Option<Duration> {
            context.schedule_event(Duration::ticks(5), EventPriority::minimum(), TestEvent);
            Some(self.initial_delay)
        }

        fn fire(
            &mut self,
            context: &mut SourceContext<TestEvent, TestModel>,
            _model: &TestModel,
        ) -> Option<Duration> {
            context.schedule_event(Duration::ticks(5), EventPriority::minimum(), TestEvent);
            Some(Duration::ticks(5))
        }
    }

    /// Mock implementation for tracking hook invocation history.
    ///
    /// Used to verify that hook methods are called as expected during tests.
    #[derive(Default)]
    struct MockHook {
        before_register_source_called: Rc<RefCell<Vec<String>>>,
        after_register_source_called: Rc<RefCell<Vec<String>>>,
        cancel_source_called: Rc<RefCell<Vec<String>>>,
        cancel_event_called: Rc<RefCell<Vec<String>>>,
    }

    impl MockHook {
        /// Creates a new mock hook.
        fn new() -> Self {
            Default::default()
        }
    }

    impl<E: Debug, M: Model<E>> Hook<E, M> for MockHook {
        fn before_simulation(&self, _model: &M) {
            unreachable!();
        }
        fn after_simulation(&self, _model: &M, _end_tick: SimTime) {
            unreachable!();
        }
        fn before_tick(&self, _model: &M, _current_tick: SimTime, _skipped_duration: Duration) {
            unreachable!();
        }
        fn after_tick(&self, _model: &M, _current_tick: SimTime, _last_micro_step: MicroStep) {
            unreachable!();
        }
        fn before_micro_step(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            unreachable!();
        }
        fn after_micro_step(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            unreachable!();
        }
        fn on_discard_remain_micro_step(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _first_discarded_micro_step: MicroStep,
            _discarded_sources: &[SourceReadyEntry],
            _discarded_events: &[Event<E>],
        ) {
            unreachable!();
        }

        fn before_register_source(&self, _model: &M, name: &str) {
            self.before_register_source_called
                .borrow_mut()
                .push(name.to_string());
        }

        fn after_register_source(&self, _model: &M, name: &str) {
            self.after_register_source_called
                .borrow_mut()
                .push(name.to_string());
        }

        fn before_source_phase(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            unreachable!();
        }

        fn before_source(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _source_view: &SourceView,
        ) {
            unreachable!();
        }
        fn after_source(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _source_view: &SourceView,
            _computed_next_fire: Option<SimTime>,
        ) {
            unreachable!();
        }

        fn cancel_source(
            &self,
            _model: &M,
            _now: SimTime,
            _micro_step: MicroStep,
            _scheduled_at: SimTime,
            source_view: &SourceView,
        ) {
            self.cancel_source_called
                .borrow_mut()
                .push(source_view.name().to_string());
        }

        fn discard_source(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _source_view: &SourceView,
        ) {
            unreachable!();
        }

        fn after_source_phase(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            unreachable!();
        }
        fn before_event_phase(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            unreachable!();
        }
        fn before_event(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _event: &Event<E>,
        ) {
            unreachable!();
        }
        fn after_event(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _event: &Event<E>,
        ) {
            unreachable!();
        }

        fn cancel_event(
            &self,
            _model: &M,
            _now: SimTime,
            _micro_step: MicroStep,
            _scheduled_at: SimTime,
            event: &Event<E>,
        ) {
            self.cancel_event_called
                .borrow_mut()
                .push(format!("{:?}", event.payload));
        }

        fn discard_event(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _event: &Event<E>,
        ) {
            unreachable!();
        }
        fn after_event_phase(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            unreachable!();
        }
    }

    /// Sets up the test environment, returning an EventContext and a SharedHook.
    fn setup() -> (
        EventContext<TestEvent, TestModel>,
        SharedHook<TestEvent, TestModel, MockHook>,
    ) {
        let test_hook = MockHook::new();
        let shared_hook = SharedHook::new(test_hook);
        let mut hook_delegate = HookDelegate::new();
        hook_delegate.add_shared_hook(shared_hook.clone());

        let context = EventContext {
            current_tick_status: TickStatus::initialize(),
            current_micro_step_status: MicroStepStatus::new(MicroStep::zero()),
            hook_delegate,
            source_handler: SourceHandler::new(),
            event_scheduler: EventScheduler::new(),
        };

        (context, shared_hook)
    }

    #[test]
    fn test_current_tick() {
        let (context, _) = setup();
        assert_eq!(context.current_tick(), SimTime::from_ticks(0));
    }

    #[test]
    fn test_current_micro_step() {
        let (context, _) = setup();
        assert_eq!(context.current_micro_step(), MicroStep::zero());
    }

    #[test]
    fn test_add_source_after() {
        let model = TestModel;
        let (mut context, shared_hook) = setup();
        let initial_sources_count = context.source_handler.ready_queue_len();
        let delay = Duration::ticks(10);

        context.add_source(
            &model,
            "test_source",
            TestSource {
                initial_delay: delay,
            },
        );

        context.source_handler.flush_pending();
        context.event_scheduler.flush_pending(); // Ensure event scheduler is also flushed if it was used by source initialization

        assert_eq!(
            context.source_handler.ready_queue_len(),
            initial_sources_count + 1
        );

        let (scheduled_at, scheduled_source) = context.source_handler.peek().unwrap();
        assert_eq!(scheduled_at, SimTime::from_ticks(0) + delay);
        assert_eq!(scheduled_source.source_id.value(), 0); // Assuming it's the first source added

        assert_eq!(
            shared_hook
                .get_ref()
                .before_register_source_called
                .borrow()
                .len(),
            1
        );
        assert_eq!(
            shared_hook.get_ref().before_register_source_called.borrow()[0],
            "test_source"
        );
        assert_eq!(
            shared_hook
                .get_ref()
                .after_register_source_called
                .borrow()
                .len(),
            1
        );
        assert_eq!(
            shared_hook.get_ref().after_register_source_called.borrow()[0],
            "test_source"
        );
    }

    #[test]
    fn test_add_source_at_now() {
        let model = TestModel;
        let (mut context, shared_hook) = setup();
        let initial_sources_count = context.source_handler.ready_queue_len();

        context.add_source(
            &model,
            "test_source_now",
            TestSource {
                initial_delay: Duration::zero(),
            },
        );

        context.source_handler.flush_pending();
        context.event_scheduler.flush_pending(); // Ensure event scheduler is also flushed if it was used by source initialization

        assert_eq!(
            context.source_handler.ready_queue_len(),
            initial_sources_count + 1
        );

        let (scheduled_at, scheduled_source) = context.source_handler.peek().unwrap();
        assert_eq!(scheduled_at, SimTime::from_ticks(0));
        assert_eq!(scheduled_source.source_id.value(), 0); // Assuming it's the first source added

        assert_eq!(
            shared_hook
                .get_ref()
                .before_register_source_called
                .borrow()
                .len(),
            1
        );
        assert_eq!(
            shared_hook.get_ref().before_register_source_called.borrow()[0],
            "test_source_now"
        );
        assert_eq!(
            shared_hook
                .get_ref()
                .after_register_source_called
                .borrow()
                .len(),
            1
        );
        assert_eq!(
            shared_hook.get_ref().after_register_source_called.borrow()[0],
            "test_source_now"
        );
    }

    #[test]
    fn test_schedule_event() {
        let (mut context, _) = setup();

        // Populate existing events
        context.event_scheduler.schedule(
            SimTime::from_ticks(0),
            Duration::one(),
            EventPriority::minimum(),
            TestEvent,
        );
        context.event_scheduler.schedule(
            SimTime::from_ticks(0),
            Duration::one(),
            EventPriority::minimum(),
            TestEvent,
        );
        context.event_scheduler.schedule(
            SimTime::from_ticks(0),
            Duration::one(),
            EventPriority::minimum(),
            TestEvent,
        );
        context.event_scheduler.flush_pending();

        let initial_scheduled_events_count = context.event_scheduler.ready_queue_len();
        let delay = Duration::ticks(5);
        let priority = EventPriority::new(10);
        let event_payload = TestEvent;

        context.schedule_event(delay, priority, event_payload.clone());
        context.source_handler.flush_pending(); // Ensure source handler is also flushed
        context.event_scheduler.flush_pending();

        assert_eq!(
            context.event_scheduler.ready_queue_len(),
            initial_scheduled_events_count + 1
        );

        let (scheduled_at, event) = context.event_scheduler.peek().unwrap();
        assert_eq!(scheduled_at, SimTime::from_ticks(1));
        assert_eq!(event.priority, EventPriority::minimum());
        assert_eq!(event.payload, TestEvent);
    }

    #[test]
    fn test_cancel_scheduled_events() {
        let (mut context, shared_hook) = setup();
        let model = TestModel;

        context.schedule_event(Duration::ticks(5), EventPriority::minimum(), TestEvent);
        context.schedule_event(Duration::ticks(10), EventPriority::minimum(), TestEvent);
        context.source_handler.flush_pending();
        context.event_scheduler.flush_pending();

        let canceled_events = context.cancel_scheduled_events(&model, |_, _| true);
        assert_eq!(canceled_events.len(), 2);
        assert_eq!(context.event_scheduler.ready_queue_len(), 0);
        assert_eq!(shared_hook.get_ref().cancel_event_called.borrow().len(), 2);
        assert_eq!(
            shared_hook.get_ref().cancel_event_called.borrow()[0],
            "TestEvent"
        );
        assert_eq!(
            shared_hook.get_ref().cancel_event_called.borrow()[1],
            "TestEvent"
        );

        // Re-schedule
        context.schedule_event(Duration::ticks(5), EventPriority::minimum(), TestEvent);
        context.schedule_event(Duration::ticks(10), EventPriority::minimum(), TestEvent);

        context.source_handler.flush_pending();
        context.event_scheduler.flush_pending();

        let canceled_events_filtered = context
            .cancel_scheduled_events(&model, |scheduled_at, _| {
                scheduled_at == SimTime::from_ticks(5)
            });
        assert_eq!(canceled_events_filtered.len(), 1);
        assert_eq!(canceled_events_filtered[0].0, SimTime::from_ticks(5));
        assert_eq!(context.event_scheduler.ready_queue_len(), 1);
        let (remaining_scheduled_at, _) = context.event_scheduler.peek().unwrap();
        assert_eq!(remaining_scheduled_at, SimTime::from_ticks(10));
        assert_eq!(shared_hook.get_ref().cancel_event_called.borrow().len(), 3);
        assert_eq!(
            shared_hook.get_ref().cancel_event_called.borrow()[2],
            "TestEvent"
        );
    }

    #[test]
    fn test_cancel_scheduled_sources() {
        let (mut context, shared_hook) = setup();
        let model = TestModel;

        context.add_source(
            &model,
            "source1",
            TestSource {
                initial_delay: Duration::ticks(5),
            },
        );
        context.add_source(
            &model,
            "source2",
            TestSource {
                initial_delay: Duration::ticks(10),
            },
        );
        context.source_handler.flush_pending();
        context.event_scheduler.flush_pending();

        let canceled_sources =
            context.cancel_scheduled_sources::<TestSource, _>(&model, |_, _| true);
        assert_eq!(canceled_sources.len(), 2);
        assert_eq!(context.source_handler.ready_queue_len(), 0);
        assert_eq!(shared_hook.get_ref().cancel_source_called.borrow().len(), 2);
        assert_eq!(
            shared_hook.get_ref().cancel_source_called.borrow()[0],
            "source1"
        );
        assert_eq!(
            shared_hook.get_ref().cancel_source_called.borrow()[1],
            "source2"
        );

        // Re-schedule
        context.add_source(
            &model,
            "source3",
            TestSource {
                initial_delay: Duration::ticks(5),
            },
        );
        context.add_source(
            &model,
            "source4",
            TestSource {
                initial_delay: Duration::ticks(10),
            },
        );
        context.source_handler.flush_pending();
        context.event_scheduler.flush_pending();

        let canceled_sources_filtered = context
            .cancel_scheduled_sources::<TestSource, _>(&model, |scheduled_at, _| {
                scheduled_at == SimTime::from_ticks(5)
            });
        assert_eq!(canceled_sources_filtered.len(), 1);
        assert_eq!(canceled_sources_filtered[0].0, SimTime::from_ticks(5));
        assert_eq!(context.source_handler.ready_queue_len(), 1);
        let (remaining_scheduled_at, _) = context.source_handler.peek().unwrap();
        assert_eq!(remaining_scheduled_at, SimTime::from_ticks(10));
        assert_eq!(shared_hook.get_ref().cancel_source_called.borrow().len(), 3);
        assert_eq!(
            shared_hook.get_ref().cancel_source_called.borrow()[2],
            "source3"
        );
    }
}
