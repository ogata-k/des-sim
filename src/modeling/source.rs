//! The `source` module defines the `Source` trait, which is used to create
//! event generators within the simulation.
//!
//! Sources are responsible for scheduling events, either periodically or based
//! on specific conditions, and can be configured to fire once or repeatedly.
//! This module also re-exports `SourceReadyEntry` and `SourceView` for
//! interacting with sources.

use crate::context::{SourceContext, UserContext};
use crate::modeling::model::Model;
use crate::primitive::time::Duration;

// Re-exporting these types to keep the internal `source_handler` module private
// while exposing a clean API for modeling.
pub use crate::source_handler::{SourceReadyEntry, SourceView};

/// Defines the interface for a simulation source.
///
/// Sources are responsible for scheduling events or influencing the simulation
/// based on their internal logic.
pub trait Source<E, M: Model<E>>: Send {
    /// Callback executed when the source is registered.
    ///
    /// This is invoked regardless of whether the simulation has started or is currently running.
    /// The `context` provides access to either [SourceContext] (at startup) or
    /// [EventContext](crate::context::EventContext) (during runtime).
    ///
    /// # Warning
    ///
    /// If this method schedules an event with [Duration::zero()] that re-registers this
    /// source, it may result in an infinite micro-step loop.
    fn on_registered(&mut self, context: &mut dyn UserContext<E, M>, model: &M)
    -> Option<Duration>;

    /// Executes the source's primary logic when triggered.
    ///
    /// # Returns
    ///
    /// The [Duration] until the next scheduled fire event, or `None` if no
    /// further events should be scheduled.
    fn fire(&mut self, context: &mut SourceContext<E, M>, model: &M) -> Option<Duration>;
}
