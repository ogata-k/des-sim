pub mod instance;

use crate::context::ExecutorStatus;
use crate::execution::SimulationResult;
use crate::execution::engine::Engine;
use crate::execution::strategy::ContinueStrategy;
use crate::modeling::model::Model;
use crate::primitive::time::{TickStatus, TimeTick};

/// A trait defining the execution policy for simulation engines.
///
/// The `Runner` orchestrates the interaction between a `Model` and an `Engine`.
/// It provides a standardized interface for different execution strategies—such
/// as synchronous execution, playback, or parallel batch processing—allowing
/// the user to control how the simulation proceeds over time.
pub trait Runner<E, M: Model<E>, CS: ContinueStrategy<E, M, Self::Err>> {
    /// Implementation-specific error type that may occur during simulation.
    type Err: std::fmt::Debug;

    /// The core, low-level method for driving the simulation.
    ///
    /// # Arguments
    /// * `engine` - The engine managing the event scheduler and context.
    /// * `model` - The initial model state.
    /// * `should_stop` - A closure invoked before each tick to evaluate termination
    ///   conditions. As an `FnMut`, it can track internal state like retry counts
    ///   or cumulative errors to trigger dynamic stops.
    fn run<F>(
        &mut self,
        engine: Engine<E, M>,
        model: M,
        should_stop: F,
    ) -> SimulationResult<M, CS::Err>
    where
        F: FnMut(&M, ExecutorStatus, TickStatus) -> bool;

    /// Advances the simulation while inserting a specified delay between ticks.
    ///
    /// Ideal for GUI/CUI visualizations or debugging where the simulation
    /// progress needs to be observable by a human.
    fn run_playback<F>(
        &mut self,
        engine: Engine<E, M>,
        model: M,
        duration: std::time::Duration,
        mut should_stop: F,
    ) -> SimulationResult<M, CS::Err>
    where
        F: FnMut(&M, ExecutorStatus, TickStatus) -> bool,
    {
        self.run(engine, model, |model, exec_status, tick_status| {
            if should_stop(model, exec_status, tick_status) {
                return true;
            }
            std::thread::sleep(duration);
            false
        })
    }

    /// Advances the simulation until a specific tick count is reached.
    ///
    /// If the `Runner` implementation optimizes by skipping time periods, this
    /// method ensures a safe exit at the first appropriate processing step
    /// exceeding the specified threshold.
    fn run_do_ticks(
        &mut self,
        engine: Engine<E, M>,
        model: M,
        tick_count: TimeTick,
        include_zero_tick: bool,
    ) -> SimulationResult<M, CS::Err> {
        self.run(
            engine,
            model,
            |_, _, next_handle_tick_status: TickStatus| {
                next_handle_tick_status.is_done_ticks(include_zero_tick, tick_count)
            },
        )
    }

    /// Automatically advances the simulation until the event queue is exhausted (idle).
    fn run_until_idle(&mut self, engine: Engine<E, M>, model: M) -> SimulationResult<M, CS::Err> {
        self.run(engine, model, |_, executor_status: ExecutorStatus, _| {
            executor_status == ExecutorStatus::NoMoreEvent
        })
    }

    /// Advances the simulation until the model's internal state satisfies a condition.
    ///
    /// Allows for domain-specific termination criteria, such as reaching a production
    /// target or a specific KPI threshold.
    fn run_until_model_condition<F>(
        &mut self,
        engine: Engine<E, M>,
        model: M,
        mut should_stop_model_condition: F,
    ) -> SimulationResult<M, CS::Err>
    where
        F: FnMut(&M) -> bool,
    {
        self.run(engine, model, |model: &M, _, _| {
            should_stop_model_condition(model)
        })
    }

    /// Executes multiple simulation trials in parallel across multiple CPU cores.
    ///
    /// Useful for Monte Carlo simulations or parametric studies. Each thread
    /// constructs its own independent instance using the provided builders.
    fn run_batch_parallel<MF, EF, F>(
        &self,
        count: usize,
        engine_builder: EF,
        model_builder: MF,
        should_stop: F,
    ) -> Vec<SimulationResult<M, CS::Err>>
    where
        Self: Clone + Sync,
        // Restricted to immutable Fn instead of FnMut so that it can be safely called many times
        // at the same time from parallel threads
        EF: Fn(usize) -> Engine<E, M> + Sync,
        M: Send,
        // Restricted to immutable Fn instead of FnMut so that it can be safely called many times
        // at the same time from parallel threads
        MF: Fn(usize) -> M + Sync,
        F: FnMut(&M, ExecutorStatus, TickStatus) -> bool + Clone + Sync,
        CS::Err: Send,
    {
        use rayon::prelude::*;

        (0..count)
            .into_par_iter()
            .map(|index| {
                let mut local_runner = self.clone();
                let local_engine = engine_builder(index);
                let local_model = model_builder(index);

                local_runner.run(local_engine, local_model, should_stop.clone())
            })
            .collect()
    }
}
