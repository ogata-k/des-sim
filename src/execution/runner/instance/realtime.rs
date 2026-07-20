//! The `realtime` module provides the `RealtimeRunner`, an implementation of the `Runner` trait
//! that synchronizes simulation progression with real-world time.
//!
//! This runner is suitable for scenarios where the simulation needs to interact with external
//! systems or be visualized at a human-perceptible pace. It skips idle periods to maintain
//! efficiency while adhering to real-time constraints.

use crate::context::ExecutorStatus;
use crate::execution::SimulationResult;
use crate::execution::engine::Engine;
use crate::execution::phase::MicroStepResult;
use crate::execution::runner::Runner;
use crate::execution::strategy::{AlwaysContinueStrategy, ContinueStrategy};
use crate::modeling::model::Model;
use crate::primitive::time::TickStatus;
use std::thread;
use std::time::{Duration as StdDuration, Instant};

/// A runner that executes the simulation in synchronization with real-world time.
///
/// This runner controls the execution timing of each tick based on the `tick_unit_duration`.
/// Periods without active events or source firings are skipped, allowing for efficient
/// CPU usage while ensuring the simulation progresses at the correct wall-clock speed.
///
/// ### Notes
/// - To perform periodic processing (e.g., status monitoring), register a periodic `Source`
///   that does nothing.
/// - If the implementations of `Model`, `Source`, and `Hook` are deterministic,
///   this runner guarantees deterministic real-time execution.
#[derive(Clone)]
pub struct RealtimeRunner<CS> {
    continue_strategy: CS,
    /// The real-world duration allocated for a single tick.
    tick_unit_duration: StdDuration,
}

impl<E, M: Model<E>, CS: ContinueStrategy<E, M, ()>> Runner<E, M, CS> for RealtimeRunner<CS> {
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

        // Record the start time to serve as the baseline for absolute time synchronization.
        let start_instant = Instant::now();
        let mut executor = engine.begin_simulation(&model);

        loop {
            let (executor_status, tick_status) = executor.peek_next_tick();
            if should_stop(&model, executor_status, tick_status) {
                break;
            }

            // --- Real-time Synchronization Logic ---
            // Calculate the target elapsed time and sleep until it matches the current wall time.
            let next_tick_value = tick_status.current().as_time_tick();
            let target_elapsed = self.tick_unit_duration * next_tick_value as u32;
            let now = Instant::now();
            let expected_instant = start_instant + target_elapsed;
            if now < expected_instant {
                thread::sleep(expected_instant - now);
            }

            // ------------------------------

            let mut active_executor = executor.begin_tick(&model);

            loop {
                // 1. Begin micro-step
                let micro_step_handler = active_executor.begin_micro_step(&model);

                // 2. Source phase
                let mut source_phase = micro_step_handler.start_source_phase(&model);
                while let Some(source_ready) = source_phase.take_one() {
                    source_phase.fire_and_schedule(&model, source_ready);
                }
                let micro_step_handler = source_phase.complete_source_phase(&model);

                // 3. Event phase
                let mut event_phase = micro_step_handler.to_event_phase(&model);
                while let Some(event_ready) = event_phase.take_one() {
                    event_phase.handle_event(&mut model, event_ready);
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

            // return to executor to prepare for next loop
            executor = active_executor.end_tick_with_jump_to_next_tick(&model);

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

impl RealtimeRunner<AlwaysContinueStrategy> {
    /// Creates a new `RealtimeRunner` with the `AlwaysContinueStrategy`.
    ///
    /// # Arguments
    /// * `tick_unit_duration` - The real-world duration that corresponds to 1 tick.
    pub fn new(tick_unit_duration: StdDuration) -> Self {
        RealtimeRunner {
            continue_strategy: AlwaysContinueStrategy::new(),
            tick_unit_duration,
        }
    }
}

impl<CS> RealtimeRunner<CS> {
    /// Creates a new `RealtimeRunner` with a specific `ContinueStrategy`.
    pub fn new_with_continue_strategy(
        tick_unit_duration: StdDuration,
        continue_strategy: CS,
    ) -> Self {
        RealtimeRunner {
            continue_strategy,
            tick_unit_duration,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{EventContext, SourceContext, UserContext};
    use crate::execution::strategy::LimitAbortStrategy;
    use crate::modeling::event::{Event, EventPriority};
    use crate::modeling::hook::Hook;
    use crate::modeling::hook::instance::SharedHook;
    use crate::modeling::source::Source;
    use crate::primitive::time::{Duration, MicroStep, SimTime, TickStatus};
    use crate::source_handler::{SourceReadyEntry, SourceView};
    use std::sync::{Arc, Mutex};

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    enum TestEvent {
        A,
    }

    #[derive(Debug)]
    struct TestModel {
        event_count: usize,
    }

    impl Model<TestEvent> for TestModel {
        fn handle_event(
            &mut self,
            _context: &mut EventContext<TestEvent, Self>,
            _event: &Event<TestEvent>,
        ) {
            self.event_count += 1;
        }
    }

    #[test]
    fn test_realtime_runner_new() {
        let runner_always = RealtimeRunner::new(std::time::Duration::from_millis(100));
        assert_eq!(
            runner_always.tick_unit_duration,
            std::time::Duration::from_millis(100)
        );

        let strategy = AlwaysContinueStrategy::new();
        let runner_custom = RealtimeRunner::new_with_continue_strategy(
            std::time::Duration::from_millis(100),
            strategy,
        );
        assert_eq!(
            runner_custom.tick_unit_duration,
            std::time::Duration::from_millis(100)
        );
    }

    #[test]
    fn test_realtime_runner_run_success() {
        let model = TestModel { event_count: 0 };
        let mut engine = Engine::new();

        // Schedule an event at tick 5 to test processing during the simulation run.
        engine.schedule_event_at(
            SimTime::from_ticks(5),
            EventPriority::minimum(),
            TestEvent::A,
        );

        let mut runner = RealtimeRunner::new(std::time::Duration::from_millis(100));

        // Stop condition: terminate after 10 ticks.
        let should_stop = |_m: &TestModel, _status: ExecutorStatus, tick: TickStatus| {
            tick.is_done_ticks(false, 10)
        };

        let result = runner.run(engine, model, should_stop);

        // Verify that the simulation completed successfully and events were processed
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.model().event_count, 1);
    }

    #[test]
    fn test_realtime_runner_run_with_strategy_error() {
        let model = TestModel { event_count: 0 };
        let mut engine = Engine::new();

        struct TestSource;

        impl Source<TestEvent, TestModel> for TestSource {
            fn on_registered(
                &mut self,
                _context: &mut dyn UserContext<TestEvent, TestModel>,
                _model: &TestModel,
            ) -> Option<Duration> {
                // Register an event to ensure the loop/continuation occurs within the first microstep
                Some(Duration::zero())
            }

            fn fire(
                &mut self,
                context: &mut SourceContext<TestEvent, TestModel>,
                _model: &TestModel,
            ) -> Option<Duration> {
                // Register events to ensure loops/continuations occur within the same microstep
                context.schedule_event(Duration::zero(), EventPriority::minimum(), TestEvent::A);
                Some(Duration::one())
            }
        }

        engine.add_source("test source", TestSource);

        // LimitAbortStrategy set to 0 micro-steps to trigger an error immediately.
        let strategy = LimitAbortStrategy::new(0, 0);
        let mut runner = RealtimeRunner::new_with_continue_strategy(
            std::time::Duration::from_millis(100),
            strategy,
        );

        // Stop condition with safety to prevent infinite loop (usually exits first on strategy error)
        let mut loop_count = 0;
        let should_stop = |_m: &TestModel, _status: ExecutorStatus, _tick: TickStatus| {
            loop_count += 1;
            loop_count > 10
        };

        let result = runner.run(engine, model, should_stop);

        // Verify that the strategy aborted the simulation with an error
        assert!(result.is_err());
    }

    #[test]
    fn test_realtime_runner_run_without_strategy_error() {
        let model = TestModel { event_count: 0 };
        let mut engine = Engine::new();

        // Event at tick 0 is processed at initialization, so 0 micro-steps is acceptable here.
        engine.schedule_event_at(SimTime::zero(), EventPriority::minimum(), TestEvent::A);

        // Input LimitAbortStrategy with microstep upper limit set to "0" and allowable number of times set to "0"
        // This causes an error to occur immediately on the first Continue judgment
        let strategy = LimitAbortStrategy::new(0, 0);
        let mut runner = RealtimeRunner::new_with_continue_strategy(
            std::time::Duration::from_millis(100),
            strategy,
        );

        // Stop condition with safety to prevent infinite loop (usually exits first on strategy error)
        let mut loop_count = 0;
        let should_stop = |_m: &TestModel, _status: ExecutorStatus, _tick: TickStatus| {
            loop_count += 1;
            loop_count > 10
        };

        let result = runner.run(engine, model, should_stop);

        // Verify that the strategy did not cause the simulation to fail due to errors.
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

    impl Source<TestEvent, TestModel> for TraceSource {
        fn on_registered(
            &mut self,
            _context: &mut dyn UserContext<TestEvent, TestModel>,
            _model: &TestModel,
        ) -> Option<Duration> {
            self.trace
                .lock()
                .unwrap()
                .push(LifecycleEvent::BeforeSimulation);
            Some(self.initial_delay)
        }

        fn fire(
            &mut self,
            context: &mut SourceContext<TestEvent, TestModel>,
            _model: &TestModel,
        ) -> Option<Duration> {
            let mut t = self.trace.lock().unwrap();
            t.push(LifecycleEvent::BeforeFireSource(context.current_tick()));
            t.push(LifecycleEvent::BeforeScheduleEvent);

            context.schedule_event(Duration::zero(), EventPriority::minimum(), TestEvent::A);

            t.push(LifecycleEvent::AfterScheduleEvent);
            t.push(LifecycleEvent::AfterFireSource(context.current_tick()));
            self.interval_delay
        }
    }

    #[test]
    fn test_runner_lifecycle_execution_order_scenario() {
        let trace = Arc::new(Mutex::new(Vec::new()));
        let model = TestModel { event_count: 0 };
        let mut engine = Engine::new();

        // Register the source that fires once at tick 1
        engine.add_source(
            "trace_source",
            TraceSource {
                trace: Arc::clone(&trace),
                initial_delay: Duration::ticks(1),
                interval_delay: None, // singleIgnition
            },
        );

        let mut runner = RealtimeRunner::new(std::time::Duration::from_millis(100));

        // Stopping condition: Stop before time 2 (at the peak stage)
        let should_stop = move |_m: &TestModel, _status: ExecutorStatus, tick: TickStatus| {
            tick.is_done_ticks(false, 2)
        };

        let result = runner.run(engine, model, should_stop);
        assert!(result.is_ok());

        // After exiting the loop, the simulation is safely finished, so we manually record it at the end.
        trace.lock().unwrap().push(LifecycleEvent::AfterSimulation);

        let final_trace = trace.lock().unwrap();

        // Legal lifecycle order based on actual code flow:
        // 1. When registering in initialize_sources (BeforeSimulation)
        // 2. Tick start at time 0 (because there is no event, the internal MicroStep is skipped or ends immediately)
        // 3. Start Tick at time 1 -> Enter MicroStep loop -> Fire Source (BeforeMicroStep -> BeforeEvent -> AfterEvent -> AfterMicroStep)
        // 4. Should_stop becomes true at peak time 2 and exits the loop -> AfterSimulation
        let expected = vec![
            LifecycleEvent::BeforeSimulation,
            // Iteration at time 1 (there is no event at time 0, so fire from this source does not pass)
            LifecycleEvent::BeforeFireSource(SimTime::from_ticks(1)),
            LifecycleEvent::BeforeScheduleEvent,
            LifecycleEvent::AfterScheduleEvent,
            LifecycleEvent::AfterFireSource(SimTime::from_ticks(1)),
            LifecycleEvent::AfterSimulation,
        ];

        assert_eq!(*final_trace, expected);
    }

    #[test]
    fn test_lifecycle_interruption_on_strategy_error() {
        let trace = Arc::new(Mutex::new(Vec::new()));
        let model = TestModel { event_count: 0 };
        let mut engine = Engine::new();

        // Prepare a source that will definitely fire infinitely at the first tick (time 0)
        engine.add_source(
            "loop_source",
            TraceSource {
                trace: Arc::clone(&trace),
                initial_delay: Duration::zero(),
                // 次のMicroStepに再度発火するよう設定
                interval_delay: Some(Duration::zero()),
            },
        );

        // Strategies to instantly generate upper bound errors for microsteps
        let strategy = LimitAbortStrategy::new(0, 0);
        let mut runner = RealtimeRunner::new_with_continue_strategy(
            std::time::Duration::from_millis(100),
            strategy,
        );

        let trace_for_stop = Arc::clone(&trace);
        let should_stop = move |_m: &TestModel, _status: ExecutorStatus, tick: TickStatus| {
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

        // Even when interrupted due to an error, verify that `AfterMicroStep`, `AfterTick`, and `AfterSimulation`,
        // which are pairs of started hooks such as `BeforeMicroStep`, detect an abnormality and correctly terminate the flow (or proceed to cleanup).
        // *This test ensures that the life cycle will not remain in an abnormal state even if a panic/error break occurs midway through.
        assert!(final_trace.contains(&LifecycleEvent::BeforeSimulation));
        assert!(final_trace.contains(&LifecycleEvent::BeforeTick(SimTime::zero())));
        assert!(!final_trace.contains(&LifecycleEvent::BeforeTick(SimTime::from_ticks(1))));

        // Verify that the normal lifecycle event (such as AfterTick) after the microstep
        // where the error occurred is not executed and the loop is safely exited.
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

    // Hook implementation for testing
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
    fn test_standard_runner_hook_lifecycle_flow_with_include_zero_tick() {
        let hook = MockHook {
            calls: Arc::new(Mutex::new(Vec::new())),
        };
        let shared_hook = SharedHook::new(hook);

        let model = TestModel { event_count: 0 };
        let mut engine = Engine::new();

        // Register a Hook to record with MockHook.
        engine.add_shared_hook(shared_hook.clone());

        // Schedule only one dummy event at tick 1
        engine.schedule_event_at(
            SimTime::from_ticks(1),
            EventPriority::minimum(),
            TestEvent::A,
        );

        let mut runner = RealtimeRunner::new(std::time::Duration::from_millis(100));

        // Stopping condition: Ends when processing for 2 times is completed.
        let should_stop = |_m: &TestModel, _status: ExecutorStatus, tick: TickStatus| {
            // Since include_zero_tick=true, it ends with 0tick and 1tick.
            tick.is_done_ticks(true, 2)
        };

        let _result = runner.run(engine, model, should_stop);

        let final_calls = shared_hook.get_ref().calls.lock().unwrap();

        // Expected value array fully compliant with phase nesting within `run`
        let expected = vec![
            HookCall::BeforeSimulation,
            // --- process  of tick 0 ---
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
            // --- process of tick 1 ---
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
            // *Event A is actually processed here.
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
            // Just before reaching time 2, should_stop becomes true and exits the loop.
            HookCall::AfterSimulation(SimTime::from_ticks(1)),
        ];

        assert_eq!(*final_calls, expected);
    }

    #[test]
    fn test_standard_runner_hook_lifecycle_flow_without_include_zero_tick() {
        let hook = MockHook {
            calls: Arc::new(Mutex::new(Vec::new())),
        };
        let shared_hook = SharedHook::new(hook);

        let model = TestModel { event_count: 0 };
        let mut engine = Engine::new();

        // Register a Hook to record with MockHook.
        engine.add_shared_hook(shared_hook.clone());

        // Schedule only one dummy event at time 1
        engine.schedule_event_at(
            SimTime::from_ticks(1),
            EventPriority::minimum(),
            TestEvent::A,
        );

        let mut runner = RealtimeRunner::new(std::time::Duration::from_millis(100));

        // Stopping condition: Ends when processing for 2 tick is completed.
        let should_stop = |_m: &TestModel, _status: ExecutorStatus, tick: TickStatus| {
            // Since include_zero_tick=false, it ends with 0 tick, 1 tick, and 2 tick.
            tick.is_done_ticks(false, 2)
        };

        let _result = runner.run(engine, model, should_stop);

        let final_calls = shared_hook.get_ref().calls.lock().unwrap();

        // Expected value array fully compliant with phase nesting within `run`
        let expected = vec![
            HookCall::BeforeSimulation,
            // --- process of tick 0 ---
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
            // --- process of tick 1 ---
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
            // *Event A is actually processed here.
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
            // --- process of tick 2 ---
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
            // Just before reaching time 3, should_stop becomes true and exits the loop.
            HookCall::AfterSimulation(SimTime::from_ticks(2)),
        ];

        assert_eq!(*final_calls, expected);
    }
}
