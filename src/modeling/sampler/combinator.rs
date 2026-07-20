//! The `combinator` module provides a set of combinators for building complex
//! `DurationSampler` instances from simpler ones.
//!
//! These combinators allow for fluent, declarative construction of sampling logic,
//! including mapping, delaying, jittering, chaining, aggregating, clamping,
//! and ensuring non-negative durations.

mod aggregate;
mod chain;
mod clamp;
mod delay;
mod ensure_non_negative;
mod jitter;
mod map;

pub use aggregate::*;
pub use chain::*;
pub use clamp::*;
pub use delay::*;
pub use ensure_non_negative::*;
pub use jitter::*;
pub use map::*;
