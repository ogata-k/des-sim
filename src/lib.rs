//! `des-sim` is a Rust library for Discrete Event System (DES) simulation.
//!
//! It provides a type-safe and high-performance environment for building classic time-driven simulations.
//! If you would like to know more about how to use it, please check the README.md file [in English](https://github.com/ogata-k/des-sim/blob/master/README.md) or [in Japanese](https://github.com/ogata-k/des-sim/blob/master/README-ja.md).
//!
//! The crate is organized into several modules:
//! - [`context`]: Provides context objects for event scheduling and interaction with the simulation environment.
//! - [`execution`]: Contains the core simulation engine and different runners for executing simulations.
//! - [`modeling`]: Offers traits and structures for defining simulation models, events, sources, and hooks.
//! - [`primitive`]: Defines fundamental data types used throughout the simulation, such as time.

pub mod context;
mod event_scheduler;
pub mod execution;
pub mod modeling;
pub mod primitive;
mod source_handler;
