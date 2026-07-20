//! The `event` module defines the `EventPhase` struct, which manages the execution
//! of events within a micro-step.
//!
//! It provides mechanisms for processing events, taking them from the queue,
//! and interacting with the simulation model and hooks.

use crate::context::{EventContext, UserContext};
use crate::execution::phase::MicroStepHandler;
use crate::modeling::event::Event;
use crate::modeling::hook::Hook;
use crate::modeling::model::Model;
use std::collections::VecDeque;

/// Manages the event execution phase in the simulation.
///
/// This structure holds the queue of events to be processed in the current micro-step,
/// manages the execution of event handling by the model, handles event discarding,
/// and performs phase-end procedures.
pub struct EventPhase<E, M: Model<E>> {
    context: EventContext<E, M>,
    ready_events: VecDeque<Event<E>>,
}

impl<E, M: Model<E>> EventPhase<E, M> {
    /// Creates a new event phase.
    pub(crate) fn new(context: EventContext<E, M>, ready_events: VecDeque<Event<E>>) -> Self {
        EventPhase {
            context,
            ready_events,
        }
    }

    /// Returns a mutable reference to the event context used in the current phase.
    pub fn get_context(&mut self) -> &mut EventContext<E, M> {
        &mut self.context
    }

    /// Completes the event phase and transitions to the next micro-step handler.
    ///
    /// This invokes the `after_event_phase` hook to update the simulation state.
    pub fn complete_event_phase(self, model: &M) -> MicroStepHandler<EventContext<E, M>> {
        self.context.hook().after_event_phase(
            model,
            self.context.current_tick(),
            self.context.current_micro_step(),
        );

        MicroStepHandler::new(self.context)
    }

    /// Pops one event from the front of the queue.
    pub fn take_one(&mut self) -> Option<Event<E>> {
        self.ready_events.pop_front()
    }

    /// Searches for and pops the first event in the queue that satisfies the given predicate.
    pub fn take_one_if<F>(&mut self, predicate: F) -> Option<Event<E>>
    where
        F: FnOnce(&Event<E>) -> bool,
    {
        // Note: VecDeque::pop_front_if is nightly. If using stable,
        // a custom implementation or drain_filter equivalent may be needed.
        self.ready_events.pop_front_if(|e| predicate(e))
    }

    /// Pops an event from the front of the queue only if it satisfies the given predicate.
    pub fn take_front_if<F>(&mut self, predicate: F) -> Option<Event<E>>
    where
        F: FnOnce(&Event<E>) -> bool,
    {
        if self.ready_events.front().is_some_and(predicate) {
            self.ready_events.pop_front()
        } else {
            None
        }
    }

    /// Takes all events currently in the queue.
    pub fn take_all(&mut self) -> VecDeque<Event<E>> {
        std::mem::take(&mut self.ready_events)
    }

    /// Extracts and returns all events from the queue that satisfy the given predicate.
    ///
    /// Events that do not satisfy the predicate remain in the queue.
    pub fn take_all_if<F>(&mut self, predicate: F) -> VecDeque<Event<E>>
    where
        F: FnMut(&Event<E>) -> bool,
    {
        let all_events = std::mem::take(&mut self.ready_events);

        let (taken, remaining): (VecDeque<_>, VecDeque<_>) =
            all_events.into_iter().partition(predicate);

        self.ready_events = remaining;

        taken
    }

    /// Processes the specified event using the model.
    ///
    /// This invokes the `before_event` and `after_event` hooks surrounding the event processing.
    pub fn handle_event(&mut self, model: &mut M, event: Event<E>) {
        self.context.hook().before_event(
            model,
            self.context.current_tick(),
            self.context.current_micro_step(),
            &event,
        );
        model.handle_event(self.get_context(), &event);
        self.context.hook().after_event(
            model,
            self.context.current_tick(),
            self.context.current_micro_step(),
            &event,
        );
    }

    /// Discards the specified event.
    ///
    /// This invokes the `discard_event` hook during the discard process.
    pub fn discard(&mut self, model: &M, event: Event<E>) {
        self.context.hook().discard_event(
            model,
            self.context.current_tick(),
            self.context.current_micro_step(),
            &event,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{EventContext, UserContext};
    use crate::event_scheduler::EventScheduler;
    use crate::modeling::event::{Event, EventPriority};
    use crate::modeling::hook::instance::{HookDelegate, SharedHook};
    use crate::modeling::model::Model;
    use crate::primitive::id::EventId;
    use crate::primitive::time::{Duration, MicroStep, MicroStepStatus, SimTime, TickStatus};
    use crate::source_handler::{SourceHandler, SourceReadyEntry, SourceView};
    use std::collections::VecDeque;
    use std::rc::Rc;
    use std::sync::Mutex;

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    enum TestEvent {
        A,
        B,
        C,
    }

    struct TestModel {
        handled_events: Vec<TestEvent>,
    }

    impl Model<TestEvent> for TestModel {
        fn handle_event(
            &mut self,
            _context: &mut EventContext<TestEvent, Self>,
            event: &Event<TestEvent>,
        ) {
            self.handled_events.push(event.payload);
        }
    }

    /// A hook implementation that tracks discarded events.
    struct DiscardHook {
        discarded_events: Rc<Mutex<Vec<TestEvent>>>,
    }

    impl Hook<TestEvent, TestModel> for DiscardHook {
        fn before_simulation(&self, _model: &TestModel) {
            // none
        }

        fn after_simulation(&self, _model: &TestModel, _end_tick: SimTime) {
            // none
        }

        fn before_tick(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _skipped_duration: Duration,
        ) {
            // none
        }

        fn after_tick(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _last_micro_step: MicroStep,
        ) {
            // none
        }

        fn before_micro_step(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            // none
        }

        fn after_micro_step(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            // none
        }

        fn on_discard_remain_micro_step(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _first_discarded_micro_step: MicroStep,
            _discarded_sources: &[SourceReadyEntry],
            _discarded_events: &[Event<TestEvent>],
        ) {
            // none
        }

        fn before_register_source(&self, _model: &TestModel, _name: &str) {
            // none
        }

        fn after_register_source(&self, _model: &TestModel, _name: &str) {
            // none
        }

        fn before_source_phase(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            // none
        }

        fn before_source(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _source_view: &SourceView,
        ) {
            // none
        }

        fn after_source(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _source_view: &SourceView,
            _computed_next_fire: Option<SimTime>,
        ) {
            // none
        }

        fn cancel_source(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _scheduled_at: SimTime,
            _source_view: &SourceView,
        ) {
            // none
        }

        fn discard_source(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _source_view: &SourceView,
        ) {
            // none
        }

        fn after_source_phase(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            // none
        }

        fn before_event_phase(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            // none
        }

        fn before_event(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _event: &Event<TestEvent>,
        ) {
            // none
        }

        fn after_event(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _event: &Event<TestEvent>,
        ) {
            // none
        }

        fn cancel_event(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _scheduled_at: SimTime,
            _event: &Event<TestEvent>,
        ) {
            // none
        }

        fn discard_event(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            event: &Event<TestEvent>,
        ) {
            self.discarded_events.lock().unwrap().push(event.payload);
        }

        fn after_event_phase(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            // none
        }
    }

    /// Sets up the initial state for an event phase test.
    fn setup_event_phase() -> (EventPhase<TestEvent, TestModel>, TestModel) {
        let model = TestModel {
            handled_events: Vec::new(),
        };
        let event_context = EventContext {
            current_tick_status: TickStatus::initialize(),
            current_micro_step_status: MicroStepStatus::initialize(),
            hook_delegate: HookDelegate::new(),
            source_handler: SourceHandler::new(),
            event_scheduler: EventScheduler::new(),
        };

        let mut ready_events = VecDeque::new();
        ready_events.push_back(Event::new(
            EventId::new(0),
            EventPriority::minimum(),
            TestEvent::A,
        ));
        ready_events.push_back(Event::new(
            EventId::new(1),
            EventPriority::minimum(),
            TestEvent::B,
        ));
        ready_events.push_back(Event::new(
            EventId::new(2),
            EventPriority::minimum(),
            TestEvent::C,
        ));

        (EventPhase::new(event_context, ready_events), model)
    }

    #[test]
    fn test_new() {
        let (event_phase, _) = setup_event_phase();
        assert_eq!(event_phase.ready_events.len(), 3);
    }

    #[test]
    fn test_get_context() {
        let (mut event_phase, _) = setup_event_phase();
        let context = event_phase.get_context();
        assert_eq!(context.current_tick(), SimTime::zero());
    }

    #[test]
    fn test_take_one() {
        let (mut event_phase, _) = setup_event_phase();
        let event = event_phase.take_one().unwrap();
        assert_eq!(event.payload, TestEvent::A);
        assert_eq!(event_phase.ready_events.len(), 2);
    }

    #[test]
    fn test_take_one_if() {
        // Test successful predicate match
        let (mut event_phase_a, _) = setup_event_phase();
        let event_a = event_phase_a
            .take_one_if(|e| e.payload == TestEvent::A)
            .unwrap();
        assert_eq!(event_a.payload, TestEvent::A);
        assert_eq!(event_phase_a.ready_events.len(), 2);
        assert_eq!(
            event_phase_a.ready_events.front().unwrap().payload,
            TestEvent::B
        );

        // Test failed predicate match
        let (mut event_phase_b, _) = setup_event_phase();
        let event_b = event_phase_b.take_one_if(|_| false);
        assert!(event_b.is_none());
        assert_eq!(event_phase_b.ready_events.len(), 3);
    }

    #[test]
    fn test_take_front_if() {
        let (mut event_phase, _) = setup_event_phase();
        let event_a = event_phase
            .take_front_if(|e| e.payload == TestEvent::A)
            .unwrap();
        assert_eq!(event_a.payload, TestEvent::A);
        assert_eq!(event_phase.ready_events.len(), 2);

        let event_b = event_phase.take_front_if(|e| e.payload == TestEvent::A);
        assert!(event_b.is_none());
        assert_eq!(event_phase.ready_events.len(), 2);
    }

    #[test]
    fn test_take_all() {
        let (mut event_phase, _) = setup_event_phase();
        let all_events = event_phase.take_all();
        assert_eq!(all_events.len(), 3);
        assert_eq!(event_phase.ready_events.len(), 0);
    }

    #[test]
    fn test_take_all_if() {
        let (mut event_phase, _) = setup_event_phase();
        event_phase.ready_events.push_back(Event::new(
            EventId::new(3),
            EventPriority::minimum(),
            TestEvent::A,
        ));

        let taken_events = event_phase.take_all_if(|e| e.payload == TestEvent::A);
        assert_eq!(taken_events.len(), 2); // Original A + added A
        assert_eq!(taken_events.front().unwrap().payload, TestEvent::A);
        assert_eq!(taken_events.get(1).unwrap().payload, TestEvent::A);

        assert_eq!(event_phase.ready_events.len(), 2);
        assert_eq!(
            event_phase.ready_events.front().unwrap().payload,
            TestEvent::B
        );
        assert_eq!(
            event_phase.ready_events.get(1).unwrap().payload,
            TestEvent::C
        );
    }

    #[test]
    fn test_handle_event() {
        let (mut event_phase, mut model) = setup_event_phase();
        let event = Event::new(EventId::new(3), EventPriority::minimum(), TestEvent::A);
        event_phase.handle_event(&mut model, event);
        assert_eq!(model.handled_events.len(), 1);
        assert_eq!(model.handled_events[0], TestEvent::A);
    }

    #[test]
    fn test_discard() {
        let (mut event_phase, model) = setup_event_phase();
        let hook = SharedHook::new(DiscardHook {
            discarded_events: Rc::new(Mutex::new(Vec::new())),
        });
        event_phase
            .get_context()
            .hook_delegate
            .add_shared_hook(hook.clone());

        let event = Event::new(EventId::new(3), EventPriority::minimum(), TestEvent::A);
        event_phase.discard(&model, event);
        assert_eq!(hook.get_ref().discarded_events.lock().unwrap().len(), 1);
        assert_eq!(
            hook.get_ref().discarded_events.lock().unwrap()[0],
            TestEvent::A
        );
    }

    #[test]
    fn test_complete_event_phase() {
        let (event_phase, model) = setup_event_phase();
        let micro_step_handler = event_phase.complete_event_phase(&model);
        assert_eq!(
            micro_step_handler.ref_context().current_tick(),
            SimTime::zero()
        );
    }
}
