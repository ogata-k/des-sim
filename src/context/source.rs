//! The `source` module provides the `SourceContext`, which is used by sources to interact with the simulation environment
//! when they are fired.
//!
//! It allows sources to schedule new events and access the current simulation time.

use crate::context::UserContext;
use crate::event_scheduler::EventScheduler;
use crate::modeling::event::EventPriority;
use crate::modeling::hook::Hook;
use crate::modeling::hook::instance::HookDelegate;
use crate::modeling::model::Model;
use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::primitive::time::{MicroStepStatus, TickStatus};
use crate::source_handler::SourceHandler;

/// Context information provided during the source execution phase.
///
/// This context provides access to the current simulation time and micro-step status
/// during model execution, as well as functionality for scheduling events.
pub struct SourceContext<E, M: Model<E>> {
    pub(crate) current_tick_status: TickStatus,
    pub(crate) current_micro_step_status: MicroStepStatus,
    pub(crate) hook_delegate: HookDelegate<E, M>,
    // ### Internal Design Note
    // The `source_handler` is stored as an `Option`. This is to allow temporary extraction of the
    // `SourceHandler` when re-registering (re-queuing) sources after they are fired. This design
    // pattern avoids lifetime issues during the holding period and ensures safe transfer of
    // ownership to the `MicroStepHandler`.
    pub(crate) source_handler: Option<SourceHandler<E, M>>,
    pub(crate) event_scheduler: EventScheduler<E>,
}

impl<E, M: Model<E>> UserContext<E, M> for SourceContext<E, M> {
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

impl<E, M: Model<E>> SourceContext<E, M> {
    /// Returns the hook associated with the current execution phase.
    pub(crate) fn hook(&self) -> &impl Hook<E, M> {
        &self.hook_delegate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::EventContext;
    use crate::event_scheduler::EventScheduler;
    use crate::modeling::event::{Event, EventPriority};
    use crate::modeling::hook::instance::HookDelegate;
    use crate::modeling::model::Model;
    use crate::primitive::time::{Duration, MicroStep, SimTime};
    use crate::primitive::time::{MicroStepStatus, TickStatus};

    #[derive(Debug, PartialEq)]
    enum TestEvent {
        EventA,
    }

    struct TestModel;

    impl Model<TestEvent> for TestModel {
        fn handle_event(
            &mut self,
            _context: &mut EventContext<TestEvent, Self>,
            _event: &Event<TestEvent>,
        ) {
            // No-op for testing
        }
    }

    /// Helper to create a default SourceContext for testing.
    fn create_test_source_context() -> SourceContext<TestEvent, TestModel> {
        SourceContext {
            current_tick_status: TickStatus::new(SimTime::from_ticks(0), Duration::zero()),
            current_micro_step_status: MicroStepStatus::new(MicroStep::zero()),
            hook_delegate: HookDelegate::new(),
            source_handler: None,
            event_scheduler: EventScheduler::new(),
        }
    }

    #[test]
    fn test_current_tick() {
        let context = create_test_source_context();
        assert_eq!(context.current_tick(), SimTime::from_ticks(0));
    }

    #[test]
    fn test_current_micro_step() {
        let context = create_test_source_context();
        assert_eq!(context.current_micro_step(), MicroStep::zero());
    }

    #[test]
    fn test_schedule_event() {
        let mut context = create_test_source_context();

        // Register a few events in advance
        context.event_scheduler.schedule(
            SimTime::from_ticks(0),
            Duration::one(),
            EventPriority::minimum(),
            TestEvent::EventA,
        );
        context.event_scheduler.schedule(
            SimTime::from_ticks(0),
            Duration::one(),
            EventPriority::minimum(),
            TestEvent::EventA,
        );
        context.event_scheduler.schedule(
            SimTime::from_ticks(0),
            Duration::one(),
            EventPriority::minimum(),
            TestEvent::EventA,
        );

        // Move events into the ready queue
        context.event_scheduler.flush_pending();
        let initial_event_count = context.event_scheduler.ready_queue_len();

        // Schedule a new event and verify the count
        context.schedule_event(Duration::one(), EventPriority::minimum(), TestEvent::EventA);
        context.event_scheduler.flush_pending();

        assert_eq!(
            context.event_scheduler.ready_queue_len(),
            initial_event_count + 1
        );
    }
}
