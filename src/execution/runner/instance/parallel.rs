//! The `parallel` module provides the `ParallelRunner`, an implementation of the `Runner` trait
//! that enables parallel execution of events.
//!
//! This runner processes events concurrently, with an option to synchronize high-priority
//! events on the main thread to prevent data races. It requires models to implement
//! the `ParallelModel` trait for safe state management across threads.

use crate::context::{EventContext, ExecutorStatus};
use crate::execution::SimulationResult;
use crate::execution::engine::Engine;
use crate::execution::phase::MicroStepResult;
use crate::execution::runner::Runner;
use crate::execution::strategy::{AlwaysContinueStrategy, ContinueStrategy};
use crate::modeling::event::{Event, EventPriority};
use crate::modeling::model::Model;
use crate::primitive::time::TickStatus;
use std::marker::PhantomData;
use std::sync::mpsc::Sender;

/// An extension trait for models supporting parallel execution.
///
/// This trait extracts model state changes into commands that are aggregated
/// on the main thread, preventing data races in concurrent environments.
pub trait ParallelModel<E, Command>: Model<E> {
    /// An event handler invoked from asynchronous (parallel) threads.
    /// Sends a `Command` representing a state change request through the provided sender.
    fn handle_event_parallel(&self, event: Event<E>, sender: Sender<Command>);

    /// Applies the result of parallel computation (`Command`) to the model on the main thread.
    ///
    /// Since `&mut self` is provided here, state updates can be performed safely.
    fn apply_command(&mut self, context: &mut EventContext<E, Self>, command: Command)
    where
        Self: Sized;
}

/// A runner that enables parallel event execution.
///
/// Events with a priority exceeding `sync_priority_threshold` are processed
/// synchronously on the main thread, while others are processed in parallel.
#[derive(Clone)]
pub struct ParallelRunner<Command, CS> {
    skippable: bool,
    continue_strategy: CS,
    /// Events with a priority greater than or equal to this threshold are processed synchronously.
    sync_priority_threshold: EventPriority,
    _command: PhantomData<Command>,
}

impl<E, Command, M: ParallelModel<E, Command>, CS: ContinueStrategy<E, M, ()>> Runner<E, M, CS>
    for ParallelRunner<Command, CS>
where
    E: Send + 'static,
    M: Send + Sync + 'static,
    Command: Send,
{
    type Err = ();

    fn run<F>(
        &mut self,
        engine: Engine<E, M>,
        mut model: M,
        mut should_stop: F,
    ) -> SimulationResult<M, CS::Err>
    where
        F: FnMut(&M, ExecutorStatus, TickStatus) -> bool,
    {
        let mut runner_error: Option<CS::Err> = None;
        let mut executor = engine.begin_simulation(&model);

        loop {
            let (executor_status, tick_status) = executor.peek_next_tick();
            if should_stop(&model, executor_status, tick_status) {
                break;
            }

            let mut active_executor = executor.begin_tick(&model);

            loop {
                // 1. Begin micro-step
                let micro_step_handler = active_executor.begin_micro_step(&model);

                // 2. Source phase (Sources are typically processed synchronously as they depend on state)
                let mut source_phase = micro_step_handler.start_source_phase(&model);
                while let Some(source_ready) = source_phase.take_one() {
                    source_phase.fire_and_schedule(&model, source_ready);
                }
                let micro_step_handler = source_phase.complete_source_phase(&model);

                // 3. Event phase
                let mut event_phase = micro_step_handler.to_event_phase(&model);

                loop {
                    // Check if the front event is eligible for synchronous processing
                    if let Some(event_ready) =
                        event_phase.take_front_if(|e| e.priority >= self.sync_priority_threshold)
                    {
                        // [Synchronous Execution] Process immediately on the main thread
                        event_phase.handle_event(&mut model, event_ready);
                    } else {
                        // Extract remaining events for parallel processing
                        let parallel_events = event_phase.take_all();
                        let (sender, receiver) = std::sync::mpsc::channel();

                        // Utilize std::thread::scope to safely share &model with the Rayon thread pool
                        std::thread::scope(|_scope| {
                            use rayon::prelude::*;

                            let sender = sender;
                            parallel_events.into_par_iter().for_each_with(
                                sender,
                                |sender_worker, event_ready| {
                                    let model_ref = &model;
                                    model_ref
                                        .handle_event_parallel(event_ready, sender_worker.clone());
                                },
                            );
                        });

                        // Apply accumulated commands to the model on the main thread
                        while let Ok(command) = receiver.try_recv() {
                            model.apply_command(event_phase.get_context(), command);
                        }

                        break;
                    }
                }

                let micro_step_handler = event_phase.complete_event_phase(&model);

                // 4. End micro-step
                match micro_step_handler.end_micro_step(&model) {
                    MicroStepResult::Continue(unchecked) => {
                        match self
                            .continue_strategy
                            .handle_micro_step_continue(&model, unchecked)
                        {
                            Ok(new_active_executor) => {
                                active_executor = new_active_executor;
                                continue;
                            }
                            Err((new_active_executor, error)) => {
                                active_executor = new_active_executor;
                                runner_error = Some(error);
                                break;
                            }
                        }
                    }
                    MicroStepResult::Complete(new_active_executor, _) => {
                        active_executor = new_active_executor;
                        break;
                    }
                }
            }

            executor = if self.skippable {
                active_executor.end_tick_with_jump_to_next_tick(&model)
            } else {
                active_executor.end_tick_with_increment_tick(&model)
            };

            if runner_error.is_some() {
                break;
            }
        }

        if let Some(error) = runner_error.take() {
            executor.end_simulation_as_error(model, error)
        } else {
            executor.end_simulation_as_ok(model)
        }
    }
}

impl<Command> ParallelRunner<Command, AlwaysContinueStrategy> {
    /// Creates a new `ParallelRunner` using the `AlwaysContinueStrategy`.
    ///
    /// # Arguments
    /// * `skippable` - If `true`, the runner allows skipping empty ticks to optimize simulation time.
    /// * `sync_priority_threshold` - Events with a priority greater than or equal to this
    ///   threshold will be processed synchronously on the main thread.
    pub fn new(skippable: bool, sync_priority_threshold: EventPriority) -> Self {
        ParallelRunner {
            skippable,
            continue_strategy: AlwaysContinueStrategy::new(),
            sync_priority_threshold,
            _command: PhantomData,
        }
    }
}

impl<Command, CS> ParallelRunner<Command, CS> {
    /// Creates a new `ParallelRunner` with a custom `ContinueStrategy`.
    ///
    /// # Arguments
    /// * `skippable` - If `true`, the runner allows skipping empty ticks to optimize simulation time.
    /// * `sync_priority_threshold` - Events with a priority greater than or equal to this
    ///   threshold will be processed synchronously on the main thread.
    /// * `continue_strategy` - The specific strategy used to determine how the simulation
    ///   handles micro-step transitions.
    pub fn new_with_continue_strategy(
        skippable: bool,
        sync_priority_threshold: EventPriority,
        continue_strategy: CS,
    ) -> Self {
        ParallelRunner {
            skippable,
            continue_strategy,
            sync_priority_threshold,
            _command: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{EventContext, ExecutorStatus, SourceContext, UserContext};
    use crate::execution::engine::Engine;
    use crate::execution::strategy::{AlwaysContinueStrategy, LimitAbortStrategy};
    use crate::modeling::event::{Event, EventPriority};
    use crate::modeling::hook::Hook;
    use crate::modeling::model::Model;
    use crate::modeling::source::Source;
    use crate::primitive::time::{Duration, MicroStep, SimTime, TickStatus};
    use crate::source_handler::{SourceReadyEntry, SourceView};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::Sender;
    use std::sync::{Arc, Mutex};

    /// Common commands for test simulation.
    #[allow(unused)]
    #[derive(Debug)]
    enum TestCommand {
        IncrementSync,
        IncrementParallel,
    }

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    enum TestEvent {
        SyncTarget,
        ParallelTarget,
    }

    /// Model used for testing parallel and synchronous execution paths.
    struct TestParallelModel {
        sync_event_count: Arc<AtomicUsize>,
        parallel_command_count: Arc<AtomicUsize>,
    }

    impl Model<TestEvent> for TestParallelModel {
        fn handle_event(
            &mut self,
            _context: &mut EventContext<TestEvent, Self>,
            event: &Event<TestEvent>,
        ) {
            // Synchronous execution path
            if let TestEvent::SyncTarget = event.payload {
                self.sync_event_count.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    impl ParallelModel<TestEvent, TestCommand> for TestParallelModel {
        fn handle_event_parallel(&self, event: Event<TestEvent>, sender: Sender<TestCommand>) {
            // Asynchronous execution path; forwards request to main thread
            if let TestEvent::ParallelTarget = event.payload {
                let _ = sender.send(TestCommand::IncrementParallel);
            }
        }

        fn apply_command(
            &mut self,
            _context: &mut EventContext<TestEvent, Self>,
            command: TestCommand,
        ) {
            // Apply result on main thread
            if let TestCommand::IncrementParallel = command {
                self.parallel_command_count.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    #[test]
    fn test_parallel_runner_new_and_args() {
        let runner = ParallelRunner::<TestCommand, AlwaysContinueStrategy>::new(
            true,
            EventPriority::new(100),
        );
        assert!(runner.skippable);
        assert_eq!(runner.sync_priority_threshold, EventPriority::new(100));
    }

    #[test]
    fn test_parallel_runner_sync_and_parallel_execution() {
        let sync_counter = Arc::new(AtomicUsize::new(0));
        let parallel_counter = Arc::new(AtomicUsize::new(0));

        let model = TestParallelModel {
            sync_event_count: sync_counter.clone(),
            parallel_command_count: parallel_counter.clone(),
        };

        let mut engine = Engine::new();

        // 1. High-priority event (synchronous)
        engine.schedule_event_at(
            SimTime::zero(),
            EventPriority::new(10),
            TestEvent::SyncTarget,
        );
        // 2. Low-priority event (parallel)
        engine.schedule_event_at(
            SimTime::zero(),
            EventPriority::new(0),
            TestEvent::ParallelTarget,
        );

        // Set the threshold to "5". 1 should be assigned to synchronous, 2 should be assigned to parallel
        let mut runner = ParallelRunner::new(true, EventPriority::new(5));

        let mut tick_counter = 0;
        let should_stop = |_m: &TestParallelModel, _status: ExecutorStatus, _tick: TickStatus| {
            if tick_counter >= 1 {
                true
            } else {
                tick_counter += 1;
                false
            }
        };

        let result = runner.run(engine, model, should_stop);
        assert!(result.is_ok());

        // Verify whether the count-up ran according to the expected route.
        assert_eq!(sync_counter.load(Ordering::SeqCst), 1);
        assert_eq!(parallel_counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_parallel_runner_boundary_priority() {
        let sync_counter = Arc::new(AtomicUsize::new(0));
        let parallel_counter = Arc::new(AtomicUsize::new(0));

        let model = TestParallelModel {
            sync_event_count: sync_counter.clone(),
            parallel_command_count: parallel_counter.clone(),
        };

        let mut engine = Engine::new();

        // Events with “exactly the same” priority as the threshold
        engine.schedule_event_at(
            SimTime::zero(),
            EventPriority::new(5),
            TestEvent::SyncTarget,
        );

        // Check the specifications for events that are higher than the threshold (5) to be processed synchronously (>= judged)
        let mut runner = ParallelRunner::new(true, EventPriority::new(5));
        let mut tick_counter = 0;
        let should_stop = |_m: &TestParallelModel, _status: ExecutorStatus, _tick: TickStatus| {
            if tick_counter >= 1 {
                true
            } else {
                tick_counter += 1;
                false
            }
        };

        let result = runner.run(engine, model, should_stop);
        assert!(result.is_ok());

        // Boundary (>= threshold) is processed synchronously
        assert_eq!(sync_counter.load(Ordering::SeqCst), 1);
        assert_eq!(parallel_counter.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_parallel_runner_massive_parallel_events() {
        let sync_counter = Arc::new(AtomicUsize::new(0));
        let parallel_counter = Arc::new(AtomicUsize::new(0));
        let model = TestParallelModel {
            sync_event_count: sync_counter.clone(),
            parallel_command_count: parallel_counter.clone(),
        };

        let mut engine = Engine::new();

        // Input a large number of parallel events at once
        let event_count = 100;
        for _ in 0..event_count {
            engine.schedule_event_at(
                SimTime::zero(),
                EventPriority::new(0), //Below Threshold
                TestEvent::ParallelTarget,
            );
        }

        let mut runner = ParallelRunner::new(true, EventPriority::new(10));
        let mut tick_counter = 0;
        let should_stop = |_m: &TestParallelModel, _status: ExecutorStatus, _tick: TickStatus| {
            if tick_counter >= 1 {
                true
            } else {
                tick_counter += 1;
                false
            }
        };

        let result = runner.run(engine, model, should_stop);
        assert!(result.is_ok());
        assert_eq!(parallel_counter.load(Ordering::SeqCst), event_count);
    }

    #[test]
    fn test_parallel_runner_aborts_on_strategy_error() {
        let mut engine = Engine::new();

        // Set the first trigger event at time 0
        engine.schedule_event_at(
            SimTime::zero(),
            // Since the threshold is less than 5, it will be processed in parallel.
            EventPriority::new(0),
            TestEvent::ParallelTarget,
        );

        struct TestAbortModel;
        impl Model<TestEvent> for TestAbortModel {
            fn handle_event(
                &mut self,
                _context: &mut EventContext<TestEvent, Self>,
                _event: &Event<TestEvent>,
            ) {
                // none
            }
        }
        impl ParallelModel<TestEvent, TestCommand> for TestAbortModel {
            fn handle_event_parallel(&self, event: Event<TestEvent>, sender: Sender<TestCommand>) {
                if let TestEvent::ParallelTarget = event.payload {
                    let _ = sender.send(TestCommand::IncrementParallel);
                }
            }
            fn apply_command(
                &mut self,
                context: &mut EventContext<TestEvent, Self>,
                _command: TestCommand,
            ) {
                // Re-schedule in the same tick to trigger Continue strategy
                context.schedule_event(
                    Duration::zero(),
                    EventPriority::new(0),
                    TestEvent::ParallelTarget,
                );
            }
        }

        let abort_model = TestAbortModel;

        // Combine ParallelRunner with a continuation strategy with a 0 limit setting that immediately fails.
        let strategy = LimitAbortStrategy::new(0, 0);
        let mut runner =
            ParallelRunner::new_with_continue_strategy(true, EventPriority::new(5), strategy);

        let mut loop_count = 0;
        let should_stop = |_m: &TestAbortModel, _status: ExecutorStatus, _tick: TickStatus| {
            loop_count += 1;
            loop_count > 5
        };

        // When executed, immediately after the first event is processed, the event is re-registered to the same Tick in apply_command,
        // and MicroStepResult::Continue occurs at the first microstep end determination (end_micro_step).
        // Immediately after, LimitAbortStrategy detects that the limit has been exceeded and returns Err.
        let result = runner.run(engine, abort_model, should_stop);

        // Verify that the process did not terminate normally and detected an error (Err) in the continuation strategy to safely terminate the process.
        assert!(result.is_err());
    }

    #[test]
    fn test_parallel_runner_without_aborts_on_strategy_error() {
        let model = TestParallelModel {
            sync_event_count: Arc::new(AtomicUsize::new(0)),
            parallel_command_count: Arc::new(AtomicUsize::new(0)),
        };

        let mut engine = Engine::new();

        // An event at time 0 registered in the engine is registered to be processed as an event at 0 tick when the simulation starts,
        // so even if the microstep upper limit is 0, it will not be affected by the upper limit.
        engine.schedule_event_at(
            SimTime::zero(),
            EventPriority::new(0),
            TestEvent::ParallelTarget,
        );

        // Combine ParallelRunner with a continuation strategy with a 0 limit setting that immediately fails.
        let strategy = LimitAbortStrategy::new(0, 0);
        let mut runner =
            ParallelRunner::new_with_continue_strategy(true, EventPriority::new(5), strategy);

        let mut loop_count = 0;
        let should_stop = |_m: &TestParallelModel, _status: ExecutorStatus, _tick: TickStatus| {
            loop_count += 1;
            loop_count > 5
        };

        let result = runner.run(engine, model, should_stop);

        // Verify that the process completed without any errors in the continuation strategy.
        assert!(result.is_ok());
    }

    // Lifecycle event definition to track call order
    #[derive(Debug, PartialEq, Eq, Clone)]
    enum LifecycleEvent {
        BeforeSimulation,
        BeforeTick(SimTime),
        BeforeFireSource(SimTime),
        BeforeScheduleEvent,
        AfterScheduleEvent,
        AfterFireSource(SimTime),
        AfterTick(SimTime),
        AfterSimulation,
    }

    // A dummy source that shares and records trace logs for each event for testing purposes.
    struct TraceSource {
        trace: Arc<Mutex<Vec<LifecycleEvent>>>,
        initial_delay: Duration,
        interval_delay: Option<Duration>,
    }

    impl Source<TestEvent, TestParallelModel> for TraceSource {
        fn on_registered(
            &mut self,
            _context: &mut dyn UserContext<TestEvent, TestParallelModel>,
            _model: &TestParallelModel,
        ) -> Option<Duration> {
            self.trace
                .lock()
                .unwrap()
                .push(LifecycleEvent::BeforeSimulation);
            Some(self.initial_delay)
        }

        fn fire(
            &mut self,
            context: &mut SourceContext<TestEvent, TestParallelModel>,
            _model: &TestParallelModel,
        ) -> Option<Duration> {
            let mut t = self.trace.lock().unwrap();
            t.push(LifecycleEvent::BeforeFireSource(context.current_tick()));
            t.push(LifecycleEvent::BeforeScheduleEvent);

            // Fire an event
            context.schedule_event(
                Duration::zero(),
                EventPriority::new(0),
                TestEvent::ParallelTarget,
            );

            t.push(LifecycleEvent::AfterScheduleEvent);
            t.push(LifecycleEvent::AfterFireSource(context.current_tick()));
            self.interval_delay
        }
    }

    #[test]
    fn test_parallel_runner_lifecycle_execution_order_scenario() {
        let trace = Arc::new(Mutex::new(Vec::new()));
        let model = TestParallelModel {
            sync_event_count: Arc::new(AtomicUsize::new(0)),
            parallel_command_count: Arc::new(AtomicUsize::new(0)),
        };
        let mut engine = Engine::new();

        //Register the source that fires once at 1 tick
        engine.add_source(
            "trace_source",
            TraceSource {
                trace: Arc::clone(&trace),
                initial_delay: Duration::ticks(1),
                interval_delay: None,
            },
        );

        let mut runner = ParallelRunner::new(true, EventPriority::new(5));

        // Stop condition: Stop when 2 tick is reached
        let should_stop =
            move |_m: &TestParallelModel, _status: ExecutorStatus, tick: TickStatus| {
                tick.is_done_ticks(false, 2)
            };

        let result = runner.run(engine, model, should_stop);
        assert!(result.is_ok());

        // After exiting the loop, the simulation is safely finished,
        // so we manually record it at the end.
        trace.lock().unwrap().push(LifecycleEvent::AfterSimulation);

        let final_trace = trace.lock().unwrap();

        let expected = vec![
            LifecycleEvent::BeforeSimulation,
            // iteration at 1 tick
            LifecycleEvent::BeforeFireSource(SimTime::from_ticks(1)),
            LifecycleEvent::BeforeScheduleEvent,
            LifecycleEvent::AfterScheduleEvent,
            LifecycleEvent::AfterFireSource(SimTime::from_ticks(1)),
            // Termination processing after exiting the loop
            LifecycleEvent::AfterSimulation,
        ];

        assert_eq!(*final_trace, expected);
    }

    #[test]
    fn test_parallel_runner_lifecycle_interruption_on_strategy_error() {
        let trace = Arc::new(Mutex::new(Vec::new()));
        let model = TestParallelModel {
            sync_event_count: Arc::new(AtomicUsize::new(0)),
            parallel_command_count: Arc::new(AtomicUsize::new(0)),
        };
        let mut engine = Engine::new();

        // Prepare a source that will definitely fire infinitely at the first tick (time 0)
        engine.add_source(
            "loop_source",
            TraceSource {
                trace: Arc::clone(&trace),
                initial_delay: Duration::zero(),
                interval_delay: Some(Duration::zero()),
            },
        );

        // Strategies to instantly generate upper bound errors for microsteps
        let strategy = LimitAbortStrategy::new(0, 0);
        let mut runner =
            ParallelRunner::new_with_continue_strategy(true, EventPriority::new(5), strategy);

        let trace_for_stop = Arc::clone(&trace);
        let should_stop =
            move |_m: &TestParallelModel, _status: ExecutorStatus, tick: TickStatus| {
                trace_for_stop
                    .lock()
                    .unwrap()
                    .push(LifecycleEvent::BeforeTick(tick.current()));
                false
            };

        let result = runner.run(engine, model, should_stop);

        // Confirmed abnormal termination due to strategy error
        assert!(result.is_err());

        let final_trace = trace.lock().unwrap();

        // Verify that even when interrupted due to an error, the process that is paired with the started hook detects an abnormality and is interrupted.
        assert!(final_trace.contains(&LifecycleEvent::BeforeSimulation));
        assert!(final_trace.contains(&LifecycleEvent::BeforeTick(SimTime::zero())));
        assert!(!final_trace.contains(&LifecycleEvent::BeforeTick(SimTime::from_ticks(1))));

        // Verify that the normal lifecycle event (such as AfterTick) after the microstep where the error occurred is not executed
        // and the loop is safely exited.
        let last_event = final_trace.last().unwrap();
        assert_ne!(last_event, &LifecycleEvent::AfterTick(SimTime::zero()));
    }

    // An enum that records that the hook was called, along with detailed parameters.
    #[derive(Debug, PartialEq, Eq, Clone)]
    enum HookCall {
        BeforeSimulation,
        AfterSimulation(SimTime),
        BeforeTick {
            current: SimTime,
            skipped: Duration,
        },
        AfterTick {
            current: SimTime,
            last_micro: MicroStep,
        },
        BeforeMicroStep {
            current: SimTime,
            micro: MicroStep,
        },
        AfterMicroStep {
            current: SimTime,
            micro: MicroStep,
        },
        BeforeSourcePhase {
            current: SimTime,
            micro: MicroStep,
        },
        AfterSourcePhase {
            current: SimTime,
            micro: MicroStep,
        },
        BeforeEventPhase {
            current: SimTime,
            micro: MicroStep,
        },
        AfterEventPhase {
            current: SimTime,
            micro: MicroStep,
        },
    }

    // テスト用の Hook 実装体
    struct MockHook {
        calls: Arc<Mutex<Vec<HookCall>>>,
    }

    impl<E, M: Model<E>> Hook<E, M> for MockHook {
        fn before_simulation(&self, _model: &M) {
            self.calls.lock().unwrap().push(HookCall::BeforeSimulation);
        }
        fn after_simulation(&self, _model: &M, end_tick: SimTime) {
            self.calls
                .lock()
                .unwrap()
                .push(HookCall::AfterSimulation(end_tick));
        }
        fn before_tick(&self, _model: &M, current_tick: SimTime, skipped_duration: Duration) {
            self.calls.lock().unwrap().push(HookCall::BeforeTick {
                current: current_tick,
                skipped: skipped_duration,
            });
        }
        fn after_tick(&self, _model: &M, current_tick: SimTime, last_micro_step: MicroStep) {
            self.calls.lock().unwrap().push(HookCall::AfterTick {
                current: current_tick,
                last_micro: last_micro_step,
            });
        }
        fn before_micro_step(
            &self,
            _model: &M,
            current_tick: SimTime,
            current_micro_step: MicroStep,
        ) {
            self.calls.lock().unwrap().push(HookCall::BeforeMicroStep {
                current: current_tick,
                micro: current_micro_step,
            });
        }
        fn after_micro_step(
            &self,
            _model: &M,
            current_tick: SimTime,
            current_micro_step: MicroStep,
        ) {
            self.calls.lock().unwrap().push(HookCall::AfterMicroStep {
                current: current_tick,
                micro: current_micro_step,
            });
        }
        fn on_discard_remain_micro_step(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _first_discarded_micro_step: MicroStep,
            _discarded_sources: &[SourceReadyEntry],
            _discarded_events: &[Event<E>],
        ) {
        }
        fn before_register_source(&self, _model: &M, _name: &str) {}
        fn after_register_source(&self, _model: &M, _name: &str) {}
        fn before_source_phase(
            &self,
            _model: &M,
            current_tick: SimTime,
            current_micro_step: MicroStep,
        ) {
            self.calls
                .lock()
                .unwrap()
                .push(HookCall::BeforeSourcePhase {
                    current: current_tick,
                    micro: current_micro_step,
                });
        }
        fn before_source(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _source_view: &SourceView,
        ) {
        }
        fn after_source(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _source_view: &SourceView,
            _computed_next_fire: Option<SimTime>,
        ) {
        }
        fn cancel_source(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _scheduled_at: SimTime,
            _source_view: &SourceView,
        ) {
        }
        fn discard_source(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _source_view: &SourceView,
        ) {
        }
        fn after_source_phase(
            &self,
            _model: &M,
            current_tick: SimTime,
            current_micro_step: MicroStep,
        ) {
            self.calls.lock().unwrap().push(HookCall::AfterSourcePhase {
                current: current_tick,
                micro: current_micro_step,
            });
        }
        fn before_event_phase(
            &self,
            _model: &M,
            current_tick: SimTime,
            current_micro_step: MicroStep,
        ) {
            self.calls.lock().unwrap().push(HookCall::BeforeEventPhase {
                current: current_tick,
                micro: current_micro_step,
            });
        }
        fn before_event(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _event: &Event<E>,
        ) {
        }
        fn after_event(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _event: &Event<E>,
        ) {
        }
        fn cancel_event(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _scheduled_at: SimTime,
            _event: &Event<E>,
        ) {
        }
        fn discard_event(
            &self,
            _model: &M,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _event: &Event<E>,
        ) {
        }
        fn after_event_phase(
            &self,
            _model: &M,
            current_tick: SimTime,
            current_micro_step: MicroStep,
        ) {
            self.calls.lock().unwrap().push(HookCall::AfterEventPhase {
                current: current_tick,
                micro: current_micro_step,
            });
        }
    }

    #[test]
    fn test_parallel_runner_hook_lifecycle_flow_with_include_zero_tick() {
        use crate::modeling::hook::instance::SharedHook;

        let hook = MockHook {
            calls: Arc::new(Mutex::new(Vec::new())),
        };
        let shared_hook = SharedHook::new(hook);

        let model = TestParallelModel {
            sync_event_count: Arc::new(AtomicUsize::new(0)),
            parallel_command_count: Arc::new(AtomicUsize::new(0)),
        };
        let mut engine = Engine::new();
        engine.add_shared_hook(shared_hook.clone());

        // Place only one parallel event (priority 0) at 1 tick
        engine.schedule_event_at(
            SimTime::from_ticks(1),
            EventPriority::new(0),
            TestEvent::ParallelTarget,
        );

        let mut runner = ParallelRunner::new(true, EventPriority::new(5));

        // Stop condition: End when processing for two times has been completed.
        let should_stop = |_m: &TestParallelModel, _status: ExecutorStatus, tick: TickStatus| {
            tick.is_done_ticks(true, 2)
        };

        let _result = runner.run(engine, model, should_stop);

        let final_calls = shared_hook.get_ref().calls.lock().unwrap();

        let expected = vec![
            HookCall::BeforeSimulation,
            // --- process of 0 tick ---
            HookCall::BeforeTick {
                current: SimTime::from_ticks(0),
                skipped: Duration::zero(),
            },
            HookCall::BeforeMicroStep {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::BeforeSourcePhase {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::AfterSourcePhase {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::BeforeEventPhase {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::AfterEventPhase {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::AfterMicroStep {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::AfterTick {
                current: SimTime::from_ticks(0),
                last_micro: MicroStep::zero(),
            },
            // --- process of 1 tick ---
            HookCall::BeforeTick {
                current: SimTime::from_ticks(1),
                skipped: Duration::ticks(0),
            },
            HookCall::BeforeMicroStep {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::BeforeSourcePhase {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::AfterSourcePhase {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::BeforeEventPhase {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::AfterEventPhase {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::AfterMicroStep {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::AfterTick {
                current: SimTime::from_ticks(1),
                last_micro: MicroStep::zero(),
            },
            HookCall::AfterSimulation(SimTime::from_ticks(1)),
        ];

        assert_eq!(*final_calls, expected);
    }

    #[test]
    fn test_parallel_runner_hook_lifecycle_flow_without_include_zero_tick() {
        use crate::modeling::hook::instance::SharedHook;

        let hook = MockHook {
            calls: Arc::new(Mutex::new(Vec::new())),
        };
        let shared_hook = SharedHook::new(hook);

        let model = TestParallelModel {
            sync_event_count: Arc::new(AtomicUsize::new(0)),
            parallel_command_count: Arc::new(AtomicUsize::new(0)),
        };
        let mut engine = Engine::new();
        engine.add_shared_hook(shared_hook.clone());

        // Place only one parallel event (priority 0) at 1 tick
        engine.schedule_event_at(
            SimTime::from_ticks(1),
            EventPriority::new(0),
            TestEvent::ParallelTarget,
        );

        let mut runner = ParallelRunner::new(true, EventPriority::new(5));

        // Stop condition: End when processing for 2 tick has been completed.
        let should_stop = |_m: &TestParallelModel, _status: ExecutorStatus, tick: TickStatus| {
            tick.is_done_ticks(false, 2)
        };

        let _result = runner.run(engine, model, should_stop);

        let final_calls = shared_hook.get_ref().calls.lock().unwrap();

        let expected = vec![
            HookCall::BeforeSimulation,
            // --- process of 0 tick ---
            HookCall::BeforeTick {
                current: SimTime::from_ticks(0),
                skipped: Duration::zero(),
            },
            HookCall::BeforeMicroStep {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::BeforeSourcePhase {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::AfterSourcePhase {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::BeforeEventPhase {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::AfterEventPhase {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::AfterMicroStep {
                current: SimTime::from_ticks(0),
                micro: MicroStep::zero(),
            },
            HookCall::AfterTick {
                current: SimTime::from_ticks(0),
                last_micro: MicroStep::zero(),
            },
            // --- process of 1 tick ---
            HookCall::BeforeTick {
                current: SimTime::from_ticks(1),
                skipped: Duration::ticks(0),
            },
            HookCall::BeforeMicroStep {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::BeforeSourcePhase {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::AfterSourcePhase {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::BeforeEventPhase {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::AfterEventPhase {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::AfterMicroStep {
                current: SimTime::from_ticks(1),
                micro: MicroStep::zero(),
            },
            HookCall::AfterTick {
                current: SimTime::from_ticks(1),
                last_micro: MicroStep::zero(),
            },
            // --- process of 2 tick ---
            HookCall::BeforeTick {
                current: SimTime::from_ticks(2),
                skipped: Duration::zero(),
            },
            HookCall::BeforeMicroStep {
                current: SimTime::from_ticks(2),
                micro: MicroStep::zero(),
            },
            HookCall::BeforeSourcePhase {
                current: SimTime::from_ticks(2),
                micro: MicroStep::zero(),
            },
            HookCall::AfterSourcePhase {
                current: SimTime::from_ticks(2),
                micro: MicroStep::zero(),
            },
            HookCall::BeforeEventPhase {
                current: SimTime::from_ticks(2),
                micro: MicroStep::zero(),
            },
            HookCall::AfterEventPhase {
                current: SimTime::from_ticks(2),
                micro: MicroStep::zero(),
            },
            HookCall::AfterMicroStep {
                current: SimTime::from_ticks(2),
                micro: MicroStep::zero(),
            },
            HookCall::AfterTick {
                current: SimTime::from_ticks(2),
                last_micro: MicroStep::zero(),
            },
            HookCall::AfterSimulation(SimTime::from_ticks(2)),
        ];

        assert_eq!(*final_calls, expected);
    }
}
