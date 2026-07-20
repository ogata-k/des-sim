//! The `instance` module provides concrete implementations of the `Hook` trait
//! and related utilities.
//!
//! This includes `HookDelegate` for managing multiple hooks, `SharedHook` for
//! thread-safe hook access, `InteractiveStepHook` for debugging, and `TraceHook`
//! for logging simulation events.

mod delegate;
mod interactive_step;
mod shared;
mod trace;

pub(crate) use delegate::*;
pub use interactive_step::*;
pub use shared::*;
pub use trace::*;
