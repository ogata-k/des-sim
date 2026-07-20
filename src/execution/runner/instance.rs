//! The `instance` module provides concrete implementations of the `Runner` trait.
//!
//! These include `StandardRunner` for efficient, time-skipping simulations,
//! `RealtimeRunner` for simulations synchronized with real-world time,
//! and `ParallelBatchRunner` for executing multiple simulations concurrently.

mod parallel;
mod realtime;
mod standard;

pub use parallel::*;
pub use realtime::*;
pub use standard::*;
