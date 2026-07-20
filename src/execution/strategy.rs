//! The `strategy` module defines various strategies that control the simulation's behavior,
//! particularly how it handles micro-step continuation.
//!
//! This allows for flexible control over simulation flow, such as enforcing limits
//! on micro-step iterations or always continuing.

mod continue_strategy;

pub use continue_strategy::*;
