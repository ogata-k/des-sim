use crate::event_scheduler::EventScheduler;
use crate::execution::phase::MicroStepHandler;
use crate::execution::{SimulationError, SimulationOutput, SimulationResult};
use crate::modeling::event::Event;
use crate::modeling::hook::Hook;
use crate::modeling::hook::instance::HookDelegate;
use crate::modeling::model::Model;
use crate::primitive::time::{Duration, SimTime};
use crate::primitive::time::{MicroStepStatus, TickStatus};
use crate::source_handler::{SourceHandler, SourceReadyEntry};
use std::cmp::min;
use std::collections::VecDeque;

/// Represents the current status of the execution engine.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum ExecutorStatus {
    /// No further events are scheduled.
    NoMoreEvent,
    /// More events are scheduled for execution.
    ExistsMoreEvent,
}

/// Manages the simulation state at the execution engine level.
///
/// This structure is used by the runner to control the simulation's progression.
/// Unlike the user-facing context, this provides a low-level interface for
/// controlling internal simulation steps and hook processing.
pub struct ExecutorContext<E, M: Model<E>> {
    pub(crate) next_tick_status: TickStatus,
    pub(crate) current_tick: SimTime,
    pub(crate) hook_delegate: HookDelegate<E, M>,
    pub(crate) source_handler: SourceHandler<E, M>,
    pub(crate) event_scheduler: EventScheduler<E>,
}

impl<E, M: Model<E>> ExecutorContext<E, M> {
    /// Retrieves the hooks associated with the current process.
    pub(crate) fn hook(&self) -> &impl Hook<E, M> {
        &self.hook_delegate
    }

    /// Previews the status of the next tick.
    ///
    /// # Returns
    /// An `ExecutorStatus` indicating if events exist, and the next tick information.
    pub fn peek_next_tick(&self) -> (ExecutorStatus, TickStatus) {
        let next_event_fired_at = self.event_scheduler.peek_next_time();
        let executor_status = if next_event_fired_at.is_some() {
            ExecutorStatus::ExistsMoreEvent
        } else {
            ExecutorStatus::NoMoreEvent
        };

        (executor_status, self.next_tick_status)
    }

    /// Initiates processing for the current tick and transitions to an active context.
    ///
    /// Invokes the `before_tick` hook and updates the simulation state.
    pub fn begin_tick(self, model: &M) -> ActiveExecutorContext<E, M> {
        self.hook_delegate.before_tick(
            model,
            self.next_tick_status.current(),
            self.next_tick_status.skipped(),
        );

        ActiveExecutorContext {
            // From here, the current TickStatus
            current_tick_status: self.next_tick_status,
            next_micro_step_status: MicroStepStatus::initialize(),
            hook_delegate: self.hook_delegate,
            source_handler: self.source_handler,
            event_scheduler: self.event_scheduler,
        }
    }

    /// Terminates the simulation successfully and returns the result.
    pub fn end_simulation_as_ok<Err>(self, model: M) -> SimulationResult<M, Err> {
        self.hook().after_simulation(&model, self.current_tick);
        Ok(SimulationOutput::new(self.current_tick, model))
    }

    /// Terminates the simulation with an error and returns the result.
    pub fn end_simulation_as_error<Err>(self, model: M, error: Err) -> SimulationResult<M, Err> {
        self.hook().after_simulation(&model, self.current_tick);
        Err(SimulationError::new(self.current_tick, model, error))
    }
}

/// Context for tick processing where micro-steps can be executed.
pub struct ActiveExecutorContext<E, M: Model<E>> {
    pub(crate) current_tick_status: TickStatus,
    /// Holds the state of the future micro-step, as none have started yet.
    pub(crate) next_micro_step_status: MicroStepStatus,
    pub(crate) hook_delegate: HookDelegate<E, M>,
    pub(crate) source_handler: SourceHandler<E, M>,
    pub(crate) event_scheduler: EventScheduler<E>,
}

impl<E, M: Model<E>> ActiveExecutorContext<E, M> {
    pub(crate) fn hook(&self) -> &impl Hook<E, M> {
        &self.hook_delegate
    }

    /// Initiates micro-step processing.
    pub fn begin_micro_step(self, model: &M) -> MicroStepHandler<ActiveExecutorContext<E, M>> {
        self.hook().before_micro_step(
            model,
            self.current_tick_status.current(),
            self.next_micro_step_status.current(),
        );

        MicroStepHandler::new(self)
    }

    /// Ends the current tick and advances to the next tick by incrementing by 1.
    pub fn end_tick_with_increment_tick(self, model: &M) -> ExecutorContext<E, M> {
        let current_tick = self.current_tick_status.current();
        self.hook()
            .after_tick(model, current_tick, self.next_micro_step_status.current());

        let next_tick = current_tick + Duration::one();
        let next_tick_status = TickStatus::new(next_tick, Duration::zero());

        ExecutorContext {
            next_tick_status,
            current_tick,
            hook_delegate: self.hook_delegate,
            source_handler: self.source_handler,
            event_scheduler: self.event_scheduler,
        }
    }

    /// Ends the current tick and jumps to the next tick based on scheduled events or sources.
    /// If no further scheduled times are found, defaults to incrementing by 1.
    pub fn end_tick_with_jump_to_next_tick(self, model: &M) -> ExecutorContext<E, M> {
        let current_tick = self.current_tick_status.current();
        self.hook()
            .after_tick(model, current_tick, self.next_micro_step_status.current());

        // Calculate the next event time and the duration of the skipped period.
        let (skipped, next_tick) = match (
            self.source_handler.peek_next_time(),
            self.event_scheduler.peek_next_time(),
        ) {
            (Some(next_scheduled_at), None) | (None, Some(next_scheduled_at)) => (
                next_scheduled_at - current_tick - Duration::one(),
                next_scheduled_at,
            ),
            (Some(source_next_scheduled_at), Some(event_next_scheduled_at)) => {
                let next_scheduled_at = min(source_next_scheduled_at, event_next_scheduled_at);
                (
                    next_scheduled_at - current_tick - Duration::one(),
                    next_scheduled_at,
                )
            }
            (_, _) => {
                // Nothing else is scheduled; proceed to the immediate next tick.
                (Duration::zero(), current_tick + Duration::one())
            }
        };
        let next_tick_status = TickStatus::new(next_tick, skipped);

        ExecutorContext {
            next_tick_status,
            current_tick,
            hook_delegate: self.hook_delegate,
            source_handler: self.source_handler,
            event_scheduler: self.event_scheduler,
        }
    }

    /// Discards any remaining micro-steps in the current tick.
    ///
    /// # Returns
    /// A pair containing the discarded sources and events.
    pub fn discard_remain_micro_step(
        &mut self,
        model: &M,
    ) -> (VecDeque<SourceReadyEntry>, VecDeque<Event<E>>) {
        let current_tick = self.current_tick_status.current();
        let mut ready_sources = self.source_handler.drain_ready(current_tick);
        let mut ready_events = self.event_scheduler.drain_ready(current_tick);

        self.hook().on_discard_remain_micro_step(
            model,
            current_tick,
            self.next_micro_step_status.current(),
            ready_sources.make_contiguous(),
            ready_events.make_contiguous(),
        );

        (ready_sources, ready_events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{EventContext, SourceContext, UserContext};
    use crate::modeling::event::{Event, EventPriority};
    use crate::modeling::hook::instance::SharedHook;
    use crate::modeling::source::Source;
    use crate::primitive::time::{Duration, MicroStep, SimTime};
    use crate::source_handler::SourceView;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Represents a simple event payload for testing.
    #[derive(Debug, PartialEq, Eq, Copy, Clone)]
    struct TestEvent;

    /// Represents a source that triggers at a fixed tick duration.
    #[derive(Debug, PartialEq, Eq, Copy, Clone)]
    struct TestSource {
        tick: Duration,
    }

    impl Source<TestEvent, TestModel> for TestSource {
        fn on_registered(
            &mut self,
            _context: &mut dyn UserContext<TestEvent, TestModel>,
            _model: &TestModel,
        ) -> Option<Duration> {
            Some(self.tick)
        }

        fn fire(
            &mut self,
            _context: &mut SourceContext<TestEvent, TestModel>,
            _model: &TestModel,
        ) -> Option<Duration> {
            Some(self.tick)
        }
    }

    /// A simple model implementation for testing context interactions.
    #[derive(Debug)]
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

    /// A mock hook that tracks method invocations for verification.
    #[derive(Default)]
    struct MockHook {
        before_tick_called: Rc<RefCell<Vec<(SimTime, Duration)>>>,
        after_tick_called: Rc<RefCell<Vec<(SimTime, MicroStep)>>>,
        before_micro_step_called: Rc<RefCell<Vec<(SimTime, MicroStep)>>>,
        #[allow(clippy::type_complexity)]
        on_discard_remain_micro_step_called: Rc<RefCell<Vec<(SimTime, MicroStep, usize, usize)>>>,
        after_simulation_called: Rc<RefCell<Vec<SimTime>>>,
    }

    impl MockHook {
        fn new() -> Self {
            Default::default()
        }
    }

    impl<E, M: Model<E>> Hook<E, M> for MockHook {
        fn before_simulation(&self, _model: &M) {
            unreachable!();
        }

        fn after_simulation(&self, _model: &M, last_tick: SimTime) {
            self.after_simulation_called.borrow_mut().push(last_tick);
        }

        fn before_tick(&self, _model: &M, now: SimTime, skipped: Duration) {
            self.before_tick_called.borrow_mut().push((now, skipped));
        }

        fn after_tick(&self, _model: &M, now: SimTime, micro_step: MicroStep) {
            self.after_tick_called.borrow_mut().push((now, micro_step));
        }

        fn before_micro_step(&self, _model: &M, now: SimTime, micro_step: MicroStep) {
            self.before_micro_step_called
                .borrow_mut()
                .push((now, micro_step));
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
            now: SimTime,
            micro_step: MicroStep,
            ready_sources: &[SourceReadyEntry],
            ready_events: &[Event<E>],
        ) {
            self.on_discard_remain_micro_step_called.borrow_mut().push((
                now,
                micro_step,
                ready_sources.len(),
                ready_events.len(),
            ));
        }

        fn before_register_source(&self, _model: &M, _name: &str) {
            unreachable!();
        }

        fn after_register_source(&self, _model: &M, _name: &str) {
            unreachable!();
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
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _scheduled_at: SimTime,
            _source_view: &SourceView,
        ) {
            unreachable!();
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
            // 呼ばれないことを確認する
            unreachable!();
        }

        fn after_event(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _event: &Event<E>,
        ) {
            // 呼ばれないことを確認する
            unreachable!();
        }

        fn cancel_event(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _scheduled_at: SimTime,
            _event: &Event<E>,
        ) {
            // 呼ばれないことを確認する
            unreachable!();
        }

        fn discard_event(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _event: &Event<E>,
        ) {
            // 呼ばれないことを確認する
            unreachable!();
        }

        fn after_event_phase(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            // 呼ばれないことを確認する
            unreachable!();
        }
    }

    fn create_mock_executor_context(
        current_tick: SimTime,
        next_tick_status: TickStatus,
    ) -> (
        ExecutorContext<TestEvent, TestModel>,
        SharedHook<TestEvent, TestModel, MockHook>,
    ) {
        let test_hook = MockHook::new();
        let shared_hook = SharedHook::new(test_hook);
        let mut hook_delegate = HookDelegate::new();
        hook_delegate.add_shared_hook(shared_hook.clone());
        let context = ExecutorContext {
            next_tick_status,
            current_tick,
            hook_delegate,
            source_handler: SourceHandler::new(),
            event_scheduler: EventScheduler::new(),
        };
        (context, shared_hook)
    }

    #[test]
    fn test_executor_context_peek_next_tick_no_event() {
        let current_tick = SimTime::from_ticks(0);
        let next_tick_status = TickStatus::new(SimTime::from_ticks(1), Duration::zero());

        let (context, _) = create_mock_executor_context(current_tick, next_tick_status);

        let (status, tick_status) = context.peek_next_tick();
        assert_eq!(status, ExecutorStatus::NoMoreEvent);
        assert_eq!(tick_status, next_tick_status);
    }

    #[test]
    fn test_executor_context_peek_next_tick_with_event() {
        let current_tick = SimTime::from_ticks(0);
        let next_tick_status = TickStatus::new(SimTime::from_ticks(1), Duration::zero());
        let (mut context, _) = create_mock_executor_context(current_tick, next_tick_status);

        context.event_scheduler.schedule(
            SimTime::from_ticks(5),
            Duration::ticks(0),
            EventPriority::minimum(),
            TestEvent,
        );
        context.event_scheduler.flush_pending();

        assert_eq!(
            context.event_scheduler.peek_next_time(),
            Some(SimTime::from_ticks(5))
        );

        let (status, tick_status) = context.peek_next_tick();
        assert_eq!(status, ExecutorStatus::ExistsMoreEvent);
        assert_eq!(tick_status, next_tick_status);
    }

    #[test]
    fn test_executor_context_begin_tick() {
        let current_tick = SimTime::from_ticks(0);
        let next_tick_status = TickStatus::new(SimTime::from_ticks(1), Duration::zero());
        let (context, shared_hook) = create_mock_executor_context(current_tick, next_tick_status);

        let model = TestModel;
        let active_context = context.begin_tick(&model);

        assert_eq!(active_context.current_tick_status, next_tick_status);
        assert_eq!(
            active_context.next_micro_step_status,
            MicroStepStatus::initialize()
        );
        assert_eq!(shared_hook.get_ref().before_tick_called.borrow().len(), 1);
        assert_eq!(
            shared_hook.get_ref().before_tick_called.borrow()[0],
            (SimTime::from_ticks(1), Duration::zero())
        );
    }

    #[test]
    fn test_executor_context_end_simulation_as_ok() {
        let current_tick = SimTime::from_ticks(10);
        let next_tick_status = TickStatus::new(SimTime::from_ticks(11), Duration::zero());
        let (context, shared_hook) = create_mock_executor_context(current_tick, next_tick_status);

        let model = TestModel;
        let result: SimulationResult<TestModel, &'static str> = context.end_simulation_as_ok(model);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.last_tick(), current_tick);
        assert_eq!(
            shared_hook.get_ref().after_simulation_called.borrow().len(),
            1
        );
        assert_eq!(
            shared_hook.get_ref().after_simulation_called.borrow()[0],
            current_tick
        );
    }

    #[test]
    fn test_executor_context_end_simulation_as_error() {
        let current_tick = SimTime::from_ticks(10);
        let next_tick_status = TickStatus::new(SimTime::from_ticks(11), Duration::zero());
        let (context, shared_hook) = create_mock_executor_context(current_tick, next_tick_status);

        let model = TestModel;
        let error_msg = "Simulation failed";
        let result = context.end_simulation_as_error(model, error_msg);

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.last_tick(), current_tick);
        assert_eq!(error.error(), &error_msg);
        assert_eq!(
            shared_hook.get_ref().after_simulation_called.borrow().len(),
            1
        );
        assert_eq!(
            shared_hook.get_ref().after_simulation_called.borrow()[0],
            current_tick
        );
    }

    // ActiveExecutorContext Tests
    fn create_mock_active_executor_context(
        current_tick_status: TickStatus,
        next_micro_step_status: MicroStepStatus,
    ) -> (
        ActiveExecutorContext<TestEvent, TestModel>,
        SharedHook<TestEvent, TestModel, MockHook>,
    ) {
        let test_hook = MockHook::new();
        let shared_hook = SharedHook::new(test_hook);
        let mut hook_delegate = HookDelegate::new();
        hook_delegate.add_shared_hook(shared_hook.clone());
        let context = ActiveExecutorContext {
            current_tick_status,
            next_micro_step_status,
            hook_delegate,
            source_handler: SourceHandler::new(),
            event_scheduler: EventScheduler::new(),
        };
        (context, shared_hook)
    }

    #[test]
    fn test_active_executor_context_begin_micro_step() {
        let current_tick_status = TickStatus::new(SimTime::from_ticks(5), Duration::zero());
        let next_micro_step_status = MicroStepStatus::new(MicroStep::zero());
        let (context, shared_hook) =
            create_mock_active_executor_context(current_tick_status, next_micro_step_status);

        let model = TestModel;
        let micro_step_handler = context.begin_micro_step(&model);

        assert_eq!(
            micro_step_handler.ref_context().current_tick_status,
            current_tick_status
        );
        assert_eq!(
            shared_hook
                .get_ref()
                .before_micro_step_called
                .borrow()
                .len(),
            1
        );
        assert_eq!(
            shared_hook.get_ref().before_micro_step_called.borrow()[0],
            (SimTime::from_ticks(5), MicroStep::zero())
        );
    }

    #[test]
    fn test_active_executor_context_end_tick_with_increment_tick() {
        let current_tick_status = TickStatus::new(SimTime::from_ticks(5), Duration::zero());
        let mut micro_step_ten = MicroStep::zero();
        for _ in 0..10 {
            micro_step_ten = micro_step_ten.next();
        }
        let next_micro_step_status = MicroStepStatus::new(micro_step_ten);
        let (context, shared_hook) =
            create_mock_active_executor_context(current_tick_status, next_micro_step_status);

        let model = TestModel;
        let next_context = context.end_tick_with_increment_tick(&model);

        assert_eq!(next_context.current_tick, SimTime::from_ticks(5));
        assert_eq!(
            next_context.next_tick_status.current(),
            SimTime::from_ticks(6)
        );
        assert_eq!(next_context.next_tick_status.skipped(), Duration::zero());
        assert_eq!(shared_hook.get_ref().after_tick_called.borrow().len(), 1);
        assert_eq!(
            shared_hook.get_ref().after_tick_called.borrow()[0],
            (SimTime::from_ticks(5), micro_step_ten)
        );
    }

    #[test]
    fn test_active_executor_context_end_tick_with_jump_to_next_tick_only_source() {
        let current_tick_status = TickStatus::new(SimTime::from_ticks(5), Duration::zero());
        let mut micro_step_ten = MicroStep::zero();
        for _ in 0..10 {
            micro_step_ten = micro_step_ten.next();
        }
        let next_micro_step_status = MicroStepStatus::new(micro_step_ten);
        let (mut context, shared_hook) =
            create_mock_active_executor_context(current_tick_status, next_micro_step_status);

        context.source_handler.add_source_after_registered_action(
            "test source",
            current_tick_status.current(),
            Some(Duration::ticks(7)),
            TestSource {
                tick: Duration::ticks(3),
            },
        );
        context.source_handler.flush_pending();

        let model = TestModel;
        let next_context = context.end_tick_with_jump_to_next_tick(&model);

        assert_eq!(next_context.current_tick, SimTime::from_ticks(5));
        assert_eq!(
            next_context.next_tick_status.current(),
            SimTime::from_ticks(12)
        );
        assert_eq!(next_context.next_tick_status.skipped(), Duration::ticks(6));
        assert_eq!(shared_hook.get_ref().after_tick_called.borrow().len(), 1);
        assert_eq!(
            shared_hook.get_ref().after_tick_called.borrow()[0],
            (SimTime::from_ticks(5), micro_step_ten)
        );
    }

    #[test]
    fn test_active_executor_context_end_tick_with_jump_to_next_tick_only_event() {
        let current_tick_status = TickStatus::new(SimTime::from_ticks(5), Duration::zero());
        let mut micro_step_ten = MicroStep::zero();
        for _ in 0..10 {
            micro_step_ten = micro_step_ten.next();
        }
        let next_micro_step_status = MicroStepStatus::new(micro_step_ten);
        let (mut context, shared_hook) =
            create_mock_active_executor_context(current_tick_status, next_micro_step_status);
        context.event_scheduler.schedule(
            SimTime::from_ticks(5),
            Duration::ticks(5),
            EventPriority::minimum(),
            TestEvent,
        );
        context.event_scheduler.flush_pending();

        let model = TestModel;
        let next_context = context.end_tick_with_jump_to_next_tick(&model);

        assert_eq!(next_context.current_tick, SimTime::from_ticks(5));
        assert_eq!(
            next_context.next_tick_status.current(),
            SimTime::from_ticks(10)
        );
        assert_eq!(next_context.next_tick_status.skipped(), Duration::ticks(4));
        assert_eq!(shared_hook.get_ref().after_tick_called.borrow().len(), 1);
        assert_eq!(
            shared_hook.get_ref().after_tick_called.borrow()[0],
            (SimTime::from_ticks(5), micro_step_ten)
        );
    }

    #[test]
    fn test_active_executor_context_end_tick_with_jump_to_next_tick_both_event_and_source() {
        let current_tick_status = TickStatus::new(SimTime::from_ticks(5), Duration::zero());
        let mut micro_step_ten = MicroStep::zero();
        for _ in 0..10 {
            micro_step_ten = micro_step_ten.next();
        }
        let next_micro_step_status = MicroStepStatus::new(micro_step_ten);
        let (mut context, shared_hook) =
            create_mock_active_executor_context(current_tick_status, next_micro_step_status);
        context.source_handler.add_source_after_registered_action(
            "test source",
            current_tick_status.current(),
            Some(Duration::ticks(7)),
            TestSource {
                tick: Duration::ticks(3),
            },
        );
        context.event_scheduler.schedule(
            SimTime::from_ticks(5),
            Duration::ticks(5),
            EventPriority::minimum(),
            TestEvent,
        );
        context.source_handler.flush_pending();
        context.event_scheduler.flush_pending();

        let model = TestModel;
        let next_context = context.end_tick_with_jump_to_next_tick(&model);

        assert_eq!(next_context.current_tick, SimTime::from_ticks(5));
        assert_eq!(
            next_context.next_tick_status.current(),
            SimTime::from_ticks(10)
        ); // min(10, 12) = 10
        assert_eq!(next_context.next_tick_status.skipped(), Duration::ticks(4));
        assert_eq!(shared_hook.get_ref().after_tick_called.borrow().len(), 1);
        assert_eq!(
            shared_hook.get_ref().after_tick_called.borrow()[0],
            (SimTime::from_ticks(5), micro_step_ten)
        );
    }

    #[test]
    fn test_active_executor_context_end_tick_with_jump_to_next_tick_no_next() {
        let current_tick_status = TickStatus::new(SimTime::from_ticks(5), Duration::zero());
        let mut micro_step_ten = MicroStep::zero();
        for _ in 0..10 {
            micro_step_ten = micro_step_ten.next();
        }
        let next_micro_step_status = MicroStepStatus::new(micro_step_ten);
        let (context, shared_hook) =
            create_mock_active_executor_context(current_tick_status, next_micro_step_status);

        let model = TestModel;
        let next_context = context.end_tick_with_jump_to_next_tick(&model);

        assert_eq!(next_context.current_tick, SimTime::from_ticks(5));
        assert_eq!(
            next_context.next_tick_status.current(),
            SimTime::from_ticks(6)
        );
        assert_eq!(next_context.next_tick_status.skipped(), Duration::zero());
        assert_eq!(shared_hook.get_ref().after_tick_called.borrow().len(), 1);
        assert_eq!(
            shared_hook.get_ref().after_tick_called.borrow()[0],
            (SimTime::from_ticks(5), micro_step_ten)
        );
    }

    #[test]
    fn test_active_executor_context_discard_remain_micro_step() {
        let current_tick_status = TickStatus::new(SimTime::from_ticks(5), Duration::zero());
        let mut micro_step_ten = MicroStep::zero();
        for _ in 0..10 {
            micro_step_ten = micro_step_ten.next();
        }
        let next_micro_step_status = MicroStepStatus::new(micro_step_ten);
        let (mut context, shared_hook) =
            create_mock_active_executor_context(current_tick_status, next_micro_step_status);

        context.source_handler.add_source_after_registered_action(
            "test source",
            current_tick_status.current(),
            Some(Duration::ticks(0)),
            TestSource {
                tick: Duration::ticks(3),
            },
        );
        context.event_scheduler.schedule(
            SimTime::from_ticks(5),
            Duration::ticks(0),
            EventPriority::minimum(),
            TestEvent,
        );
        context.event_scheduler.schedule(
            SimTime::from_ticks(5),
            Duration::ticks(0),
            EventPriority::minimum(),
            TestEvent,
        );
        context.source_handler.flush_pending();
        context.event_scheduler.flush_pending();

        let model = TestModel;
        let (discarded_sources, discarded_events) = context.discard_remain_micro_step(&model);

        assert_eq!(discarded_sources.len(), 1);
        assert_eq!(discarded_sources[0].name(), "test source");

        assert_eq!(discarded_events.len(), 2);
        assert_eq!(discarded_events[0].payload, TestEvent);
        assert_eq!(discarded_events[1].payload, TestEvent);

        assert_eq!(
            shared_hook
                .get_ref()
                .on_discard_remain_micro_step_called
                .borrow()
                .len(),
            1
        );
        assert_eq!(
            shared_hook
                .get_ref()
                .on_discard_remain_micro_step_called
                .borrow()[0],
            (SimTime::from_ticks(5), micro_step_ten, 1, 2)
        );
    }
}
