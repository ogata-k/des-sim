//! The `handler` module defines `MicroStepHandler`, a central component for managing
//! the execution flow within a simulation tick.
//!
//! It orchestrates the transitions between different micro-step phases (source and event processing)
//! and ensures type-safe progression of the simulation state.

use crate::context::{ActiveExecutorContext, EventContext, SourceContext};
use crate::execution::phase::{EventPhase, MicroStepResult, SourcePhase, UncheckedActiveExecutor};
use crate::modeling::hook::Hook;
use crate::modeling::model::Model;
use crate::primitive::time::MicroStepStatus;

/// Controls the micro-step execution phases of the simulation.
///
/// This structure owns the simulation state (context) and guarantees phase transitions
/// in a one-way (disposable) manner. By not implementing `Clone`, it ensures the safety
/// of the execution flow at the type-system level.
pub struct MicroStepHandler<CTX> {
    context: CTX,
}

impl<CTX> MicroStepHandler<CTX> {
    /// Creates a new handler.
    pub(crate) fn new(context: CTX) -> MicroStepHandler<CTX> {
        MicroStepHandler { context }
    }

    /// Returns an immutable reference to the context.
    pub fn ref_context(&self) -> &CTX {
        &self.context
    }

    /// Returns a mutable reference to the context.
    pub fn ref_mut_context(&mut self) -> &mut CTX {
        &mut self.context
    }
}

impl<E, M: Model<E>> MicroStepHandler<ActiveExecutorContext<E, M>> {
    /// Starts the source execution phase.
    ///
    /// Generates a `SourcePhase` from the current context and invokes the `before_source_phase` hook.
    pub fn start_source_phase(self, model: &M) -> SourcePhase<E, M> {
        let mut context = self.context;
        context.hook().before_source_phase(
            model,
            context.current_tick_status.current(),
            context.next_micro_step_status.current(),
        );

        let ready_sources = context
            .source_handler
            .drain_ready(context.current_tick_status.current());

        SourcePhase::new(
            SourceContext {
                current_tick_status: context.current_tick_status,
                current_micro_step_status: context.next_micro_step_status,
                hook_delegate: context.hook_delegate,
                source_handler: None,
                event_scheduler: context.event_scheduler,
            },
            context.source_handler,
            ready_sources,
        )
    }
}

impl<E, M: Model<E>> MicroStepHandler<SourceContext<E, M>> {
    /// Transitions to the event execution phase.
    ///
    /// Invokes the `before_event_phase` hook, restores the pending `source_handler`,
    /// and generates an `EventPhase`.
    pub fn to_event_phase(self, model: &M) -> EventPhase<E, M> {
        let mut context = self.context;
        context.hook().before_event_phase(
            model,
            context.current_tick_status.current(),
            context.current_micro_step_status.current(),
        );

        let ready_events = context
            .event_scheduler
            .drain_ready(context.current_tick_status.current());

        EventPhase::new(
            EventContext {
                current_tick_status: context.current_tick_status,
                current_micro_step_status: context.current_micro_step_status,
                hook_delegate: context.hook_delegate,
                source_handler: context
                    .source_handler
                    .expect("Failed to retrieve SourceHandler from SourcePhase."),
                event_scheduler: context.event_scheduler,
            },
            ready_events,
        )
    }
}

impl<E, M: Model<E>> MicroStepHandler<EventContext<E, M>> {
    /// Ends the current micro-step and returns the result (continue or complete).
    ///
    /// Checks for remaining executable events or sources within the current tick and
    /// constructs the context for the next micro-step if necessary.
    pub fn end_micro_step(mut self, model: &M) -> MicroStepResult<E, M> {
        // Flush pending handlers to accurately peek at the next scheduled events/sources.
        self.ref_mut_context().source_handler.flush_pending();
        self.ref_mut_context().event_scheduler.flush_pending();
        let current_tick = self.ref_context().current_tick_status.current();

        // Check the next scheduled time for both events and sources.
        let next_event_at = self.ref_context().event_scheduler.peek_next_time();
        let next_source_at = self.ref_context().source_handler.peek_next_time();

        let has_next_in_current_tick = matches!(next_event_at, Some(t) if t == current_tick)
            || matches!(next_source_at, Some(t) if t == current_tick);

        if has_next_in_current_tick {
            // Still have work to do in the current tick; proceed to the next micro-step.
            let current_micro_step = self.ref_context().current_micro_step_status.current();
            let next_micro_step = current_micro_step.next();

            self.context.hook().after_micro_step(
                model,
                current_tick,
                self.ref_context().current_micro_step_status.current(),
            );

            let active_context = ActiveExecutorContext {
                current_tick_status: self.ref_context().current_tick_status,
                next_micro_step_status: MicroStepStatus::new(next_micro_step),
                hook_delegate: self.context.hook_delegate,
                source_handler: self.context.source_handler,
                event_scheduler: self.context.event_scheduler,
            };

            MicroStepResult::Continue(UncheckedActiveExecutor::new(
                active_context,
                current_micro_step,
            ))
        } else {
            // No more work in the current tick; terminate the micro-step.
            let last_micro_step_status = self.ref_context().current_micro_step_status;

            self.context.hook().after_micro_step(
                model,
                current_tick,
                self.ref_context().current_micro_step_status.current(),
            );

            let active_context = ActiveExecutorContext {
                current_tick_status: self.ref_context().current_tick_status,
                next_micro_step_status: last_micro_step_status,
                hook_delegate: self.context.hook_delegate,
                source_handler: self.context.source_handler,
                event_scheduler: self.context.event_scheduler,
            };

            MicroStepResult::Complete(active_context, last_micro_step_status)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{ActiveExecutorContext, EventContext, SourceContext};
    use crate::event_scheduler::EventScheduler;
    use crate::modeling::event::{Event, EventPriority};
    use crate::modeling::hook::instance::{HookDelegate, SharedHook};
    use crate::modeling::model::Model;
    use crate::primitive::time::{Duration, MicroStep, MicroStepStatus, SimTime, TickStatus};
    use crate::source_handler::{SourceHandler, SourceReadyEntry, SourceView};
    use std::rc::Rc;
    use std::sync::Mutex;

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    enum TestEvent {
        A,
    }

    struct TestModel;

    impl Model<TestEvent> for TestModel {
        fn handle_event(
            &mut self,
            _context: &mut EventContext<TestEvent, Self>,
            _event: &Event<TestEvent>,
        ) {
            // No-op
        }
    }

    /// Hook used to track phase transition calls during tests.
    struct TransitionTrackerHook {
        called_before_source_phase: Rc<Mutex<bool>>,
        called_before_event_phase: Rc<Mutex<bool>>,
        called_after_micro_step: Rc<Mutex<bool>>,
    }

    impl Hook<TestEvent, TestModel> for TransitionTrackerHook {
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
            *self.called_after_micro_step.lock().unwrap() = true;
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
            *self.called_before_source_phase.lock().unwrap() = true;
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
            *self.called_before_event_phase.lock().unwrap() = true;
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
            _event: &Event<TestEvent>,
        ) {
            // none
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

    fn setup_tracker_hook() -> SharedHook<TestEvent, TestModel, TransitionTrackerHook> {
        SharedHook::new(TransitionTrackerHook {
            called_before_source_phase: Rc::new(Mutex::new(false)),
            called_before_event_phase: Rc::new(Mutex::new(false)),
            called_after_micro_step: Rc::new(Mutex::new(false)),
        })
    }

    #[test]
    fn test_ref_context_and_mut() {
        let mut handler = MicroStepHandler::new(42);
        assert_eq!(*handler.ref_context(), 42);

        *handler.ref_mut_context() = 100;
        assert_eq!(*handler.ref_context(), 100);
    }

    #[test]
    fn test_start_source_phase() {
        let model = TestModel;
        let hook = setup_tracker_hook();
        let mut hook_delegate = HookDelegate::new();
        hook_delegate.add_shared_hook(hook.clone());

        let active_context = ActiveExecutorContext {
            current_tick_status: TickStatus::initialize(),
            next_micro_step_status: MicroStepStatus::initialize(),
            hook_delegate,
            source_handler: SourceHandler::new(),
            event_scheduler: EventScheduler::new(),
        };

        let handler = MicroStepHandler::new(active_context);
        let mut source_phase = handler.start_source_phase(&model);

        assert!(source_phase.source_handler.is_some());
        assert!(source_phase.get_context().source_handler.is_none());
        assert!(*hook.get_ref().called_before_source_phase.lock().unwrap());
    }

    #[test]
    fn test_to_event_phase() {
        let model = TestModel;
        let hook = setup_tracker_hook();
        let mut hook_delegate = HookDelegate::new();
        hook_delegate.add_shared_hook(hook.clone());

        let source_context = SourceContext {
            current_tick_status: TickStatus::initialize(),
            current_micro_step_status: MicroStepStatus::initialize(),
            hook_delegate,
            source_handler: Some(SourceHandler::new()),
            event_scheduler: EventScheduler::new(),
        };

        let handler = MicroStepHandler::new(source_context);
        let _event_phase = handler.to_event_phase(&model);

        assert!(*hook.get_ref().called_before_event_phase.lock().unwrap());
    }

    #[test]
    fn test_end_micro_step_continue() {
        let model = TestModel;
        let hook = setup_tracker_hook();
        let mut hook_delegate = HookDelegate::new();
        hook_delegate.add_shared_hook(hook.clone());

        let tick_status = TickStatus::initialize();
        let current_time = tick_status.current();

        let mut event_scheduler = EventScheduler::new();
        event_scheduler.schedule(
            current_time,
            Duration::zero(),
            EventPriority::minimum(),
            TestEvent::A,
        );
        event_scheduler.flush_pending();

        assert_eq!(event_scheduler.peek_next_time(), Some(current_time));

        let event_context = EventContext {
            current_tick_status: tick_status,
            current_micro_step_status: MicroStepStatus::initialize(),
            hook_delegate,
            source_handler: SourceHandler::new(),
            event_scheduler,
        };

        let handler = MicroStepHandler::new(event_context);
        let result = handler.end_micro_step(&model);

        match result {
            MicroStepResult::Continue(_) => {
                // pass
            }
            MicroStepResult::Complete(_, _) => {
                panic!("Expected MicroStepResult::Continue, but got Complete");
            }
        }
        assert!(*hook.get_ref().called_after_micro_step.lock().unwrap());
    }

    #[test]
    fn test_end_micro_step_complete() {
        let model = TestModel;
        let hook = setup_tracker_hook();
        let mut hook_delegate = HookDelegate::new();
        hook_delegate.add_shared_hook(hook.clone());

        let event_context = EventContext {
            current_tick_status: TickStatus::initialize(),
            current_micro_step_status: MicroStepStatus::initialize(),
            hook_delegate,
            source_handler: SourceHandler::new(),
            event_scheduler: EventScheduler::new(),
        };

        let handler = MicroStepHandler::new(event_context);
        let result = handler.end_micro_step(&model);

        match result {
            MicroStepResult::Complete(_, _) => {
                // pass
            }
            MicroStepResult::Continue(_) => {
                panic!("Expected MicroStepResult::Complete, but got Continue");
            }
        }
        assert!(*hook.get_ref().called_after_micro_step.lock().unwrap());
    }
}
