//! The `continue_strategy` module defines the `ContinueStrategy` trait and its associated types.
//!
//! This trait allows for custom logic to determine whether the simulation should continue
//! after each micro-step, providing a flexible way to implement termination conditions
//! or error handling.

mod always_continue;
mod limit_abort;
mod limit_discard;

pub use always_continue::*;
pub use limit_abort::*;
pub use limit_discard::*;

use crate::context::ActiveExecutorContext;
use crate::execution::phase::UncheckedActiveExecutor;
use crate::modeling::model::Model;

/// The result type for simulation continuation strategies.
///
/// On success (`Ok`), it returns an `ActiveExecutorContext` to proceed to the
/// next simulation step. On failure (`Err`), it returns the current context
/// along with a strategy-specific error, allowing the simulation to terminate
/// gracefully or be debugged.
pub type ContinuousStrategyResult<E, M, Err> =
    Result<ActiveExecutorContext<E, M>, (ActiveExecutorContext<E, M>, Err)>;

/// A trait for defining strategies that decide whether to continue simulation
/// execution after each micro-step.
///
/// By implementing this trait, you can flexibly customize simulation termination
/// conditions, such as:
/// - Limiting the total number of micro-steps.
/// - Aborting execution when specific criteria are met.
/// - Enforcing continuous execution regardless of state.
pub trait ContinueStrategy<E, M: Model<E>, RunnerError> {
    /// The error type associated with this strategy.
    type Err;

    /// Invoked at the end of a micro-step to determine whether to continue.
    ///
    /// # Arguments
    /// * `model` - A reference to the current model state.
    /// * `unchecked_executor` - The execution engine awaiting validation to
    ///   proceed to the next step.
    ///
    /// # Returns
    /// * `Ok` - The simulation is authorized to proceed to the next step.
    /// * `Err` - The simulation must be terminated based on the strategy's logic.
    #[allow(clippy::result_large_err)]
    fn handle_micro_step_continue(
        &mut self,
        model: &M,
        unchecked_executor: UncheckedActiveExecutor<E, M>,
    ) -> ContinuousStrategyResult<E, M, Self::Err>;
}
