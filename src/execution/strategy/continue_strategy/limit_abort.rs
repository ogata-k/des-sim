use crate::execution::phase::UncheckedActiveExecutor;
use crate::execution::strategy::{ContinueStrategy, ContinuousStrategyResult};
use crate::modeling::model::Model;
use std::fmt::{Display, Formatter};

/// Errors associated with the `LimitAbortStrategy`.
#[derive(Debug)]
pub enum LimitAbortStrategyError<RunnerError> {
    /// An error originating from the execution engine.
    Runner(RunnerError),
    /// Simulation aborted due to exceeding the maximum micro-step limit.
    LimitExceeded {
        limit_micro_step_count: u64,
        limit_micro_step_exceeded_count: usize,
    },
}

impl<RunnerError: Display> Display for LimitAbortStrategyError<RunnerError> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LimitAbortStrategyError::Runner(e) => {
                write!(f, "{}", e)
            }
            LimitAbortStrategyError::LimitExceeded {
                limit_micro_step_count,
                limit_micro_step_exceeded_count,
            } => {
                write!(
                    f,
                    "The maximum limit of {} micro-steps has been exceeded {} times.",
                    limit_micro_step_count, limit_micro_step_exceeded_count
                )
            }
        }
    }
}

impl<RunnerError: std::error::Error> std::error::Error for LimitAbortStrategyError<RunnerError> {}

/// A continuation strategy that monitors and limits the number of micro-steps.
///
/// This strategy tracks the micro-step count within each tick. If the count exceeds
/// `limit_micro_step_count`, the strategy allows for a grace period defined by
/// `allow_error_count`. Within this grace period, the strategy attempts to recover
/// by discarding remaining tasks in the current tick. Once the limit is exceeded
/// beyond the allowed count, the simulation is forcibly terminated with an error.
#[derive(Clone)]
pub struct LimitAbortStrategy {
    limit_micro_step_count: u64,
    allow_error_count: usize,
    error_count: usize,
}

impl<E, M: Model<E>, RunnerError> ContinueStrategy<E, M, RunnerError> for LimitAbortStrategy {
    type Err = LimitAbortStrategyError<RunnerError>;

    fn handle_micro_step_continue(
        &mut self,
        model: &M,
        unchecked: UncheckedActiveExecutor<E, M>,
    ) -> ContinuousStrategyResult<E, M, Self::Err> {
        let current_micro_step = unchecked.current_micro_step();

        // Continue unconditionally if within the limit.
        if current_micro_step.value() < self.limit_micro_step_count {
            return Ok(unchecked.into_active_executor());
        }

        self.error_count += 1;
        let mut next_active = unchecked.into_active_executor();

        // Check if the error is within the allowed threshold.
        if self.error_count <= self.allow_error_count {
            // Within limit: Purge remaining tasks in the current tick to attempt recovery.
            next_active.discard_remain_micro_step(model);
            Ok(next_active)
        } else {
            // Exceeded limit: Clean up the current tick and terminate with an error.
            next_active.discard_remain_micro_step(model);
            Err((
                next_active,
                LimitAbortStrategyError::LimitExceeded {
                    limit_micro_step_count: self.limit_micro_step_count,
                    limit_micro_step_exceeded_count: self.error_count,
                },
            ))
        }
    }
}

impl LimitAbortStrategy {
    /// Creates a new `LimitAbortStrategy`.
    ///
    /// # Arguments
    /// * `limit_micro_step_count` - Maximum micro-steps permitted per tick.
    /// * `allow_error_count` - Number of times exceeding the limit is permitted.
    pub fn new(limit_micro_step_count: u64, allow_error_count: usize) -> Self {
        LimitAbortStrategy {
            limit_micro_step_count,
            allow_error_count,
            error_count: 0,
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

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    enum TestEvent {}

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

    /// Helper function to generate an `UncheckedActiveExecutor` for testing.
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
    fn test_limit_abort_strategy_under_limit() {
        let model = TestModel;
        // Limit: 10, Allow Error Count: 1
        let mut strategy = LimitAbortStrategy::new(10, 1);

        // Current micro-step: 9 (Below limit of 10)
        let unchecked = create_unchecked_executor(9);
        let result: ContinuousStrategyResult<
            TestEvent,
            TestModel,
            LimitAbortStrategyError<String>,
        > = strategy.handle_micro_step_continue(&model, unchecked);

        // Should continue unconditionally when below limit.
        assert!(result.is_ok());
        assert_eq!(strategy.error_count, 0);
    }

    #[test]
    fn test_limit_abort_strategy_reach_limit_within_allowance() {
        let model = TestModel;
        // Limit: 10, Allow Error Count: 1
        let mut strategy = LimitAbortStrategy::new(10, 1);

        // 1st time reaching limit (Micro-step 10)
        let unchecked = create_unchecked_executor(10);
        let result: ContinuousStrategyResult<
            TestEvent,
            TestModel,
            LimitAbortStrategyError<String>,
        > = strategy.handle_micro_step_continue(&model, unchecked);

        // Within grace period (1), so should return Ok.
        assert!(result.is_ok());
        assert_eq!(strategy.error_count, 1);
    }

    #[test]
    fn test_limit_abort_strategy_exceed_allowance_aborts() {
        let model = TestModel;
        // Limit: 10, Allow Error Count: 1
        let mut strategy = LimitAbortStrategy::new(10, 1);

        // 1st reach (Within allowance)
        let unchecked_1 = create_unchecked_executor(10);
        let unchecked_1_result: ContinuousStrategyResult<
            TestEvent,
            TestModel,
            LimitAbortStrategyError<String>,
        > = strategy.handle_micro_step_continue(&model, unchecked_1);
        assert!(unchecked_1_result.is_ok());

        // 2nd reach (Exceeds allowance)
        let unchecked_2 = create_unchecked_executor(10);
        let unchecked_2_result: ContinuousStrategyResult<
            TestEvent,
            TestModel,
            LimitAbortStrategyError<String>,
        > = strategy.handle_micro_step_continue(&model, unchecked_2);

        // Limit exceeded, should return Err.
        assert!(unchecked_2_result.is_err());
        assert_eq!(strategy.error_count, 2);

        // Verify error details.
        let (_context, error) = unchecked_2_result.err().unwrap();
        match error {
            LimitAbortStrategyError::LimitExceeded {
                limit_micro_step_count,
                limit_micro_step_exceeded_count,
            } => {
                assert_eq!(limit_micro_step_count, 10);
                assert_eq!(limit_micro_step_exceeded_count, 2);
            }
            _ => panic!("Expected LimitAbortStrategyError::LimitExceeded variant"),
        }
    }

    #[test]
    fn test_error_display() {
        // Validate Display implementation for the custom error.
        let error: LimitAbortStrategyError<String> = LimitAbortStrategyError::LimitExceeded {
            limit_micro_step_count: 5,
            limit_micro_step_exceeded_count: 3,
        };
        let message = format!("{}", error);
        assert_eq!(
            message,
            "The maximum limit of 5 micro-steps has been exceeded 3 times."
        );

        let runner_error = LimitAbortStrategyError::Runner("internal error".to_string());
        assert_eq!(format!("{}", runner_error), "internal error");
    }
}
