//! The `time` module provides fundamental types and utilities for managing
//! time within the discrete event simulation.
//!
//! This includes `SimTime` for representing absolute simulation time, `Duration`
//! for time intervals, `MicroStep` for ordering events within the same `SimTime`,
//! and `TickStatus`/`MicroStepStatus` for tracking the current state of time
//! progression.

mod micro_step;
mod sim_time;

pub use micro_step::*;
pub use sim_time::*;
