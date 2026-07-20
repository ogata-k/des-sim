//! The `phase` module defines the different phases within a simulation tick,
//! including micro-steps for event and source processing.
//!
//! It provides structures like `MicroStepHandler` to manage the execution flow
//! within a tick and `MicroStepResult` to determine the next action.

mod event;
mod handler;
mod source;

use crate::context::ActiveExecutorContext;
use crate::modeling::model::Model;
use crate::primitive::time::MicroStepStatus;
use crate::primitive::time::{MicroStep, SimTime};
pub use event::*;
pub use handler::*;
pub use source::*;

/// A wrapper for an execution context awaiting micro-step continuation validation.
///
/// This structure holds a "pending" context that has not yet been approved by
/// a `ContinueStrategy`. By wrapping the `ActiveExecutorContext`, it enforces
/// a pattern where the execution engine must explicitly call `into_active_executor()`
/// after validation, effectively confirming that the simulation state is authorized
/// to proceed to the next step.
pub struct UncheckedActiveExecutor<E, M: Model<E>> {
    active_executor: ActiveExecutorContext<E, M>,
    current_micro_step: MicroStep,
}

impl<E, M: Model<E>> UncheckedActiveExecutor<E, M> {
    pub(crate) fn new(
        executor: ActiveExecutorContext<E, M>,
        current_micro_step: MicroStep,
    ) -> Self {
        Self {
            active_executor: executor,
            current_micro_step,
        }
    }

    /// Returns the current simulation time.
    pub fn current_tick(&self) -> SimTime {
        self.active_executor.current_tick_status.current()
    }

    /// Returns the current micro-step index.
    pub fn current_micro_step(&self) -> MicroStep {
        self.current_micro_step
    }

    /// Consumes this wrapper and returns the validated `ActiveExecutorContext`.
    ///
    /// This method is intended to be called only after a strategy has verified
    /// that it is safe to continue the simulation.
    pub fn into_active_executor(self) -> ActiveExecutorContext<E, M> {
        self.active_executor
    }
}

/// The result of a micro-step execution.
///
/// This enum dictates the next phase of the simulation, differentiating between
/// whether execution should continue to the next micro-step (`Continue`) or
/// terminate the current tick (`Complete`).
pub enum MicroStepResult<E, M: Model<E>> {
    /// Indicates that the simulation can continue. The `UncheckedActiveExecutor`
    /// is passed forward to be validated by the assigned `ContinueStrategy`.
    Continue(UncheckedActiveExecutor<E, M>),
    /// Indicates that the current tick is finished. The `ActiveExecutorContext`
    /// and the final `MicroStepStatus` are returned to finalize the tick's state.
    Complete(ActiveExecutorContext<E, M>, MicroStepStatus),
}
