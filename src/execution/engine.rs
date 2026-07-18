use crate::context::{ExecutorContext, SourceContext};
use crate::event_scheduler::EventScheduler;
use crate::modeling::event::EventPriority;
use crate::modeling::hook::Hook;
use crate::modeling::hook::instance::{HookDelegate, SharedHook};
use crate::modeling::model::Model;
use crate::modeling::source::Source;
use crate::primitive::time::{Duration, SimTime};
use crate::primitive::time::{MicroStepStatus, TickStatus};
use crate::source_handler::SourceHandler;

/// The engine responsible for simulation setup and initialization.
///
/// The `Engine` manages the registration of `Hook`s, `Source`s, and initial events
/// required for model execution. Calling `begin_simulation` transitions the engine
/// into an active `ExecutorContext`, freezing the configuration and preparing
/// the engine for the main simulation loop.
pub struct Engine<E, M: Model<E>> {
    pub(crate) hook_delegate: HookDelegate<E, M>,
    pub(crate) source_handler: SourceHandler<E, M>,
    pub(crate) event_scheduler: EventScheduler<E>,
}

impl<E, M: Model<E>> Default for Engine<E, M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E, M: Model<E>> Engine<E, M> {
    /// Creates a new `Engine` instance.
    pub fn new() -> Engine<E, M> {
        Engine {
            hook_delegate: HookDelegate::new(),
            source_handler: SourceHandler::new(),
            event_scheduler: EventScheduler::new(),
        }
    }

    /// Initializes the simulation based on registered configurations and
    /// returns the `ExecutorContext`.
    ///
    /// This process performs the following sequence:
    /// 1. Executes the `before_simulation` hook.
    /// 2. Initializes all registered sources (`on_registered`).
    /// 3. Propagates events generated during initialization to the event scheduler.
    pub fn begin_simulation(mut self, model: &M) -> ExecutorContext<E, M> {
        self.hook_delegate.before_simulation(model);

        // Reflect the source registered in Engine
        self.source_handler.flush_pending();

        // Initialize all registered sources before simulation
        {
            // Create a temporary context for source initialization.
            let mut source_context = SourceContext {
                current_tick_status: TickStatus::initialize(),
                current_micro_step_status: MicroStepStatus::initialize(),
                hook_delegate: self.hook_delegate,
                // Although SourceContext has properties, it only has SourceHandler
                // when it is used as a handler when creating an EventPhase.
                // So here we pass None.
                source_handler: None,
                event_scheduler: self.event_scheduler,
            };

            self.source_handler.initialize_sources(|source| {
                source_context
                    .hook()
                    .before_register_source(model, source.name.as_ref());
                let first_fired_opt = source.source.on_registered(&mut source_context, model);
                source_context
                    .hook()
                    .after_register_source(model, source.name.as_ref());

                first_fired_opt
            });

            // Reconstruct the engine from the state recovered from the context.
            self = Engine {
                hook_delegate: source_context.hook_delegate,
                source_handler: self.source_handler,
                event_scheduler: source_context.event_scheduler,
            };
        }

        // Reflect the events registered when initializing the Engine and the above Source.
        self.event_scheduler.flush_pending();

        ExecutorContext {
            next_tick_status: TickStatus::initialize(),
            current_tick: SimTime::zero(),
            hook_delegate: self.hook_delegate,
            source_handler: self.source_handler,
            event_scheduler: self.event_scheduler,
        }
    }

    /// Adds a simulation hook to the engine.
    pub fn add_hook<H>(&mut self, hook: H) -> &mut Self
    where
        H: Hook<E, M> + 'static,
    {
        self.hook_delegate.add_hook(hook);
        self
    }

    /// Adds a shared simulation hook to the engine.
    pub fn add_shared_hook<H>(&mut self, shared_hook: SharedHook<E, M, H>) -> &mut Self
    where
        E: 'static,
        M: 'static,
        H: Hook<E, M> + 'static,
    {
        self.hook_delegate.add_shared_hook(shared_hook);
        self
    }

    /// Registers a source before the simulation begins.
    pub fn add_source<S>(&mut self, name: &'static str, source: S) -> &mut Self
    where
        S: Source<E, M> + 'static,
    {
        self.source_handler
            .add_source_for_before_simulation(name, source);

        self
    }

    /// Schedules an event to be processed when the simulation starts.
    pub fn schedule_event_at(
        &mut self,
        sim_time: SimTime,
        priority: EventPriority,
        event_payload: E,
    ) -> &mut Self {
        self.event_scheduler
            .schedule(sim_time, Duration::zero(), priority, event_payload);

        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{EventContext, UserContext};
    use crate::modeling::event::Event;
    use crate::modeling::hook::instance::InteractiveStepHook;

    #[derive(Debug, PartialEq)]
    enum TestEvent {
        Start,
    }

    struct TestModel;

    impl TestModel {
        fn new() -> Self {
            TestModel
        }
    }

    impl Model<TestEvent> for TestModel {
        fn handle_event(
            &mut self,
            _context: &mut EventContext<TestEvent, Self>,
            _event: &Event<TestEvent>,
        ) {
            // No-op for testing
        }
    }

    #[test]
    fn test_engine_new() {
        let engine: Engine<TestEvent, TestModel> = Engine::new();

        // Verify initial state is empty
        assert!(engine.hook_delegate.is_empty());
        assert_eq!(engine.source_handler.ready_queue_len(), 0);
        assert_eq!(engine.event_scheduler.ready_queue_len(), 0);
    }

    #[test]
    fn test_engine_begin_simulation() {
        let model = TestModel::new();
        let engine: Engine<TestEvent, TestModel> = Engine::new();

        // Ensure transition to executor context maintains integrity
        let context = engine.begin_simulation(&model);

        assert!(context.hook_delegate.is_empty());
        assert_eq!(context.source_handler.ready_queue_len(), 0);
        assert_eq!(context.event_scheduler.ready_queue_len(), 0);
    }

    #[test]
    fn test_engine_add_hook() {
        let mut engine: Engine<TestEvent, TestModel> = Engine::new();
        engine.add_hook(InteractiveStepHook);

        // Ensure hooks are correctly tracked by the delegate
        assert_eq!(engine.hook_delegate.len(), 1);
    }

    #[test]
    fn test_engine_add_source() {
        struct MySource;
        impl Source<TestEvent, TestModel> for MySource {
            fn on_registered(
                &mut self,
                context: &mut dyn UserContext<TestEvent, TestModel>,
                _model: &TestModel,
            ) -> Option<Duration> {
                // Schedule an event upon registration
                context.schedule_event(Duration::one(), EventPriority::minimum(), TestEvent::Start);
                None
            }

            fn fire(
                &mut self,
                _context: &mut SourceContext<TestEvent, TestModel>,
                _model: &TestModel,
            ) -> Option<Duration> {
                None
            }
        }

        let model = TestModel;
        let mut engine: Engine<TestEvent, TestModel> = Engine::new();
        engine.add_source("my_source", MySource);

        // Verify source registration correctly populates the event scheduler during begin_simulation
        let context = engine.begin_simulation(&model);
        assert_eq!(context.event_scheduler.ready_queue_len(), 1);
    }

    #[test]
    fn test_engine_schedule_event_at() {
        let mut engine: Engine<TestEvent, TestModel> = Engine::new();
        let sim_time = SimTime::from_ticks(10);

        engine.schedule_event_at(sim_time, EventPriority::minimum(), TestEvent::Start);

        // Pending events shouldn't be immediately visible in the ready queue
        assert_eq!(engine.event_scheduler.ready_queue_len(), 0);

        // Verify flushing moves the event to the ready queue
        engine.event_scheduler.flush_pending();
        assert_eq!(engine.event_scheduler.ready_queue_len(), 1);
    }
}
