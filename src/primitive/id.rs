//! The `id` module provides unique identifier types for events and sources
//! within the simulation.
//!
//! These IDs (`EventId` and `SourceId`) are essential for tracking and
//! managing individual simulation components, ensuring that each entity
//! can be uniquely referenced.

mod event_id;
mod source_id;

pub use event_id::EventId;
pub use source_id::SourceId;
