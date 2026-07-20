//! The `result` module defines the outcome types for a simulation run:
//! `SimulationResult`, `SimulationOutput`, and `SimulationError`.
//!
//! These types encapsulate the final state of the simulation, whether it completed
//! successfully or terminated due to an error, providing access to the final
//! simulation time and model state.

use crate::primitive::time::SimTime;

/// A type alias representing the final outcome of a simulation run.
pub type SimulationResult<M, Err> = Result<SimulationOutput<M>, SimulationError<M, Err>>;

/// The final output of a successfully completed simulation.
#[derive(Debug)]
pub struct SimulationOutput<M> {
    /// The simulation time at which the execution finished.
    time: SimTime,
    /// The state of the model at the end of the simulation.
    model: M,
}

impl<M> SimulationOutput<M> {
    /// Create `SimulationOutput` instance.
    pub(crate) fn new(time: SimTime, model: M) -> Self {
        Self { time, model }
    }

    /// Returns the last simulation tick (time) reached.
    pub fn last_tick(&self) -> SimTime {
        self.time
    }

    /// Returns a reference to the final model state.
    pub fn model(&self) -> &M {
        &self.model
    }

    /// Returns a mutable reference to the final model state.
    pub fn model_mut(&mut self) -> &mut M {
        &mut self.model
    }
}

/// The state of a simulation that terminated due to an error.
#[derive(Debug)]
pub struct SimulationError<M, Err> {
    /// The simulation time at which the error occurred.
    time: SimTime,
    /// The state of the model at the time of the error.
    model: M,
    /// The specific error that caused the simulation to halt.
    error: Err,
}

impl<M, Err> SimulationError<M, Err> {
    pub(crate) fn new(time: SimTime, model: M, error: Err) -> Self {
        Self { time, model, error }
    }

    /// Returns the simulation tick (time) at which the error occurred.
    pub fn last_tick(&self) -> SimTime {
        self.time
    }

    /// Returns a reference to the model state at the time of the error.
    pub fn model(&self) -> &M {
        &self.model
    }

    /// Returns a mutable reference to the model state at the time of the error.
    pub fn model_mut(&mut self) -> &mut M {
        &mut self.model
    }

    /// Returns a reference to the error that occurred.
    pub fn error(&self) -> &Err {
        &self.error
    }

    /// Returns a mutable reference to the error that occurred.
    pub fn error_mut(&mut self) -> &mut Err {
        &mut self.error
    }
}
