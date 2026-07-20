//! The `limit_discard` module provides the `LimitDiscardStrategy`, an implementation
//! of the `ContinueStrategy` trait that discards remaining tasks when a micro-step
//! limit is reached.
//!
//! This strategy allows the simulation to continue without error, but with a controlled
//! loss of precision or completeness within a tick, by purging pending events and
//! sources once the micro-step count exceeds a defined threshold.

use crate::execution::phase::UncheckedActiveExecutor;
use crate::execution::strategy::{ContinueStrategy, ContinuousStrategyResult};
use crate::modeling::model::Model;

/// A strategy that discards remaining tasks when the micro-step limit is reached.
///
/// This strategy prioritizes the continuation of the simulation over absolute
/// execution accuracy. When the micro-step count exceeds the specified
/// `limit_micro_step_count`, the strategy purges all remaining micro-steps
/// and pending events for the current tick without triggering an error,
/// effectively forcing the simulation to proceed to the next tick.
#[derive(Clone)]
pub struct LimitDiscardStrategy {
    limit_micro_step_count: u64,
}

impl<E, M: Model<E>, RunnerError> ContinueStrategy<E, M, RunnerError> for LimitDiscardStrategy {
    type Err = RunnerError;

    /// Checks the micro-step limit and purges remaining tasks if exceeded.
    fn handle_micro_step_continue(
        &mut self,
        model: &M,
        unchecked: UncheckedActiveExecutor<E, M>,
    ) -> ContinuousStrategyResult<E, M, Self::Err> {
        let current_micro_step = unchecked.current_micro_step();

        // Continue without modification if within the threshold.
        if current_micro_step.value() < self.limit_micro_step_count {
            return Ok(unchecked.into_active_executor());
        }

        // Limit reached: Purge remaining tasks in the current execution context.
        let mut next_active = unchecked.into_active_executor();
        // Since it is still continuing, all remaining events to be processed on the current tick are discarded.
        next_active.discard_remain_micro_step(model);
        Ok(next_active)
    }
}

impl LimitDiscardStrategy {
    /// Creates a new `LimitDiscardStrategy`.
    ///
    /// # Arguments
    /// * `limit_micro_step_count` - The micro-step threshold at which discarding begins.
    pub fn new(limit_micro_step_count: u64) -> Self {
        LimitDiscardStrategy {
            limit_micro_step_count,
        }
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
    use std::convert::Infallible;

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    enum TestEvent {}

    struct TestModel;

    impl Model<TestEvent> for TestModel {
        fn handle_event(
            &mut self,
            _context: &mut EventContext<TestEvent, Self>,
            _event: &Event<TestEvent>,
        ) {
        }
    }

    /// Helper to create a test `UncheckedActiveExecutor`.
    fn create_unchecked_executor(
        micro_step_val: u64,
    ) -> UncheckedActiveExecutor<TestEvent, TestModel> {
        let active_context = ActiveExecutorContext {
            current_tick_status: TickStatus::initialize(),
            next_micro_step_status: MicroStepStatus::new(MicroStep::new(micro_step_val + 1)),
            hook_delegate: HookDelegate::new(),
            source_handler: SourceHandler::new(),
            event_scheduler: EventScheduler::new(),
        };
        UncheckedActiveExecutor::new(active_context, MicroStep::new(micro_step_val))
    }

    #[test]
    fn test_limit_discard_strategy_under_limit() {
        let model = TestModel;
        let mut strategy = LimitDiscardStrategy::new(5);

        // Within limit (4 < 5): continue without discarding.
        let unchecked = create_unchecked_executor(4);
        let result: ContinuousStrategyResult<TestEvent, TestModel, Infallible> =
            strategy.handle_micro_step_continue(&model, unchecked);

        // Since it is below the upper limit, discard does not run and is OK.
        assert!(
            result.is_ok(),
            "Strategy failed to continue when under limit."
        );
    }

    #[test]
    fn test_limit_discard_strategy_reach_limit() {
        let model = TestModel;
        let mut strategy = LimitDiscardStrategy::new(5);

        // Limit reached (5): should purge events and continue without error.
        let unchecked = create_unchecked_executor(5);
        let result: ContinuousStrategyResult<TestEvent, TestModel, Infallible> =
            strategy.handle_micro_step_continue(&model, unchecked);

        // Even if the upper limit is reached, it does not become Err,
        // but is internally discarded and then becomes Ok.
        assert!(
            result.is_ok(),
            "Strategy failed to continue after reaching limit."
        );
    }
}
