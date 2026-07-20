//! The `context` module provides various context objects that allow interaction with the simulation environment.
//!
//! These contexts are passed to models, sources, and hooks, enabling them to schedule events,
//! access simulation time, and interact with other parts of the simulation.

mod event;
mod executor;
mod source;
mod user;

pub use event::*;
pub use executor::*;
pub use source::*;
pub use user::*;
