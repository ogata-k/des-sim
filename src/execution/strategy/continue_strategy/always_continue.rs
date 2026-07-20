//! The `always_continue` module provides the `AlwaysContinueStrategy`, a simple implementation
//! of the `ContinueStrategy` trait.
//!
//! This strategy unconditionally allows the simulation to proceed to the next micro-step,
//! effectively delegating termination control to the main simulation loop's `should_stop`
//! condition or the natural exhaustion of events.

use crate::execution::phase::UncheckedActiveExecutor;
use crate::execution::strategy::{ContinueStrategy, ContinuousStrategyResult};
use crate::modeling::model::Model;

/// A default strategy that unconditionally allows simulation continuation.
///
/// Since this strategy imposes no termination criteria, the simulation proceeds
/// until the global stop condition (`should_stop`) is met or the model's
/// internal logic completes naturally.
#[derive(Clone)]
pub struct AlwaysContinueStrategy;

impl Default for AlwaysContinueStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl<E, M: Model<E>, RunnerError> ContinueStrategy<E, M, RunnerError> for AlwaysContinueStrategy {
    type Err = RunnerError;

    /// Always returns `Ok`, authorizing the execution engine to proceed to the
    /// next micro-step.
    fn handle_micro_step_continue(
        &mut self,
        _model: &M,
        unchecked: UncheckedActiveExecutor<E, M>,
    ) -> ContinuousStrategyResult<E, M, Self::Err> {
        Ok(unchecked.into_active_executor())
    }
}

impl AlwaysContinueStrategy {
    /// Creates a new `AlwaysContinueStrategy`.
    pub fn new() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{ActiveExecutorContext, EventContext};
    use crate::event_scheduler::EventScheduler;
    use crate::modeling::event::Event;
    use crate::modeling::hook::instance::HookDelegate;
    use crate::primitive::time::{MicroStep, MicroStepStatus, TickStatus};
    use crate::source_handler::SourceHandler;

    /// Dummy event for testing.
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    enum TestEvent {}

    /// Dummy model for testing.
    #[derive(Debug, Clone)]
    struct TestModel;

    impl Model<TestEvent> for TestModel {
        fn handle_event(
            &mut self,
            _context: &mut EventContext<TestEvent, Self>,
            _event: &Event<TestEvent>,
        ) {
        }
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn test_always_continue_strategy_default_and_new() {
        let _strategy_new = AlwaysContinueStrategy::new();
        let _strategy_default = AlwaysContinueStrategy::default();
    }

    #[test]
    fn test_always_continue_strategy_handle_continue() {
        let model = TestModel;

        // Prepare the simulation context.
        let active_context = ActiveExecutorContext {
            current_tick_status: TickStatus::initialize(),
            next_micro_step_status: MicroStepStatus::initialize(),
            hook_delegate: HookDelegate::new(),
            source_handler: SourceHandler::new(),
            event_scheduler: EventScheduler::new(),
        };

        // Create the execution state.
        let current_micro_step = MicroStep::zero();
        let unchecked_executor = UncheckedActiveExecutor::new(active_context, current_micro_step);

        let mut strategy = AlwaysContinueStrategy::new();

        // Validate strategy execution.
        let result: ContinuousStrategyResult<TestEvent, TestModel, ()> =
            strategy.handle_micro_step_continue(&model, unchecked_executor);

        assert!(result.is_ok(), "Continuation strategy failed unexpectedly.");

        let returned_context = result.map_err(|e| e.1).unwrap();

        // Verify that the context information is correctly preserved.
        assert_eq!(
            returned_context.current_tick_status.current(),
            crate::primitive::time::SimTime::zero()
        );
    }
}
