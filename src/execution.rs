//! The `execution` module contains the core components for running a discrete event simulation.
//!
//! It defines the `Engine` which manages the simulation state, various `Runner` strategies
//! for advancing the simulation, and the `SimulationResult` for capturing outcomes.

mod engine;
pub mod phase;
mod result;
pub mod runner;
pub mod strategy;

pub use engine::*;
pub use result::*;
