//! The `modeling` module provides the fundamental building blocks for defining discrete event simulation models.
//!
//! It includes traits and structures for:
//! - [`agent`]: Representing autonomous entities within the simulation.
//! - [`event`]: Defining events that drive the simulation.
//! - [`hook`]: Observing and reacting to simulation events and state changes.
//! - [`model`]: Implementing the core simulation logic.
//! - [`sampler`]: Generating random data for stochastic simulations.
//! - [`source`]: Generating events periodically or based on specific conditions.

pub mod agent;
pub mod event;
pub mod hook;
pub mod model;
pub mod sampler;
pub mod source;
