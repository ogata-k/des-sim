//! The `interactive_step` module provides the `InteractiveStepHook`, a utility
//! for debugging and interactive control of a simulation.
//!
//! This hook pauses the simulation at the beginning of each tick, prompting the user
//! to press Enter to proceed. This allows for step-by-step inspection of the simulation
//! state, making it invaluable for understanding complex simulation dynamics.

use crate::modeling::event::Event;
use crate::modeling::hook::Hook;
use crate::modeling::model::Model;
use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::source_handler::{SourceReadyEntry, SourceView};
use std::io;
use std::io::Write;

/// A hook that pauses the simulation at each tick to allow for interactive inspection.
pub struct InteractiveStepHook;

impl<E, M: Model<E>> Hook<E, M> for InteractiveStepHook {
    fn before_simulation(&self, _model: &M) {
        // No-op
    }

    fn after_simulation(&self, _model: &M, _end_tick: SimTime) {
        println!("[Interactive Step Hook] Simulation halted: termination condition reached.");
    }

    fn before_tick(&self, _model: &M, current_tick: SimTime, skipped_duration: Duration) {
        println!("================ [Interactive Step Hook] ================");
        println!("  Current Tick          : {:?}", current_tick);
        println!("  Skipped Duration      : {} ticks", skipped_duration);
        println!("--------------------------------------------------------");

        print!(
            "[Interactive Step Hook] Press Enter to process this tick (source/event phases)... "
        );

        if cfg!(test) || cfg!(feature = "des_sim_test_mode") {
            // Skip waiting during tests to prevent blocking
            return;
        }

        let _ = io::stdout().flush(); // Ensure prompt is displayed

        // Block thread until user presses Enter
        let mut input = String::new();
        let _ = io::stdin().read_line(&mut input);
    }

    fn after_tick(&self, _model: &M, current_tick: SimTime, last_micro_step: MicroStep) {
        println!(
            "[Interactive Step Hook] Finished processing tick {}. (Count micro-steps: {})",
            current_tick,
            // Adjust 0-indexed micro-step to count
            last_micro_step.value() + 1
        );
        println!("========================================================\n");
    }

    fn before_micro_step(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
    ) {
        // No-op
    }

    fn after_micro_step(&self, _model: &M, _current_tick: SimTime, _current_micro_step: MicroStep) {
        // No-op
    }

    fn on_discard_remain_micro_step(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _first_discarded_micro_step: MicroStep,
        _discarded_sources: &[SourceReadyEntry],
        _discarded_events: &[Event<E>],
    ) {
        // No-op
    }

    fn before_register_source(&self, _model: &M, _name: &str) {
        // No-op
    }

    fn after_register_source(&self, _model: &M, _name: &str) {
        // No-op
    }

    fn before_source_phase(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
    ) {
        // No-op
    }

    fn before_source(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _source_view: &SourceView,
    ) {
        // No-op
    }

    fn after_source(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _source_view: &SourceView,
        _computed_next_fire: Option<SimTime>,
    ) {
        // No-op
    }

    fn cancel_source(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _scheduled_at: SimTime,
        _source_view: &SourceView,
    ) {
        // No-op
    }

    fn discard_source(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _source_view: &SourceView,
    ) {
        // No-op
    }

    fn after_source_phase(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
    ) {
        // No-op
    }

    fn before_event_phase(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
    ) {
        // No-op
    }

    fn before_event(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _event: &Event<E>,
    ) {
        // No-op
    }

    fn after_event(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _event: &Event<E>,
    ) {
        // No-op
    }

    fn cancel_event(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _scheduled_at: SimTime,
        _event: &Event<E>,
    ) {
        // No-op
    }

    fn discard_event(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _event: &Event<E>,
    ) {
        // No-op
    }

    fn after_event_phase(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
    ) {
        // No-op
    }
}
