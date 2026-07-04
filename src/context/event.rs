use crate::context::UserContext;
use crate::event_scheduler::EventScheduler;
use crate::modeling::event::{Event, EventPriority};
use crate::modeling::hook::Hook;
use crate::modeling::hook::instance::HookDelegate;
use crate::modeling::model::Model;
use crate::modeling::source::Source;
use crate::primitive::time::{Duration, MicroStep, MicroStepStatus, SimTime, TickStatus};
use crate::source_handler::{SourceHandler, SourceReadyEntry, SourceView};

pub struct EventContext<E, M: Model<E>> {
    pub(crate) current_tick_status: TickStatus,
    pub(crate) current_micro_step_status: MicroStepStatus,
    pub(crate) hook_delegate: HookDelegate<E, M>,
    pub(crate) source_handler: SourceHandler<E, M>,
    pub(crate) event_scheduler: EventScheduler<E>,
}

impl<E, M: Model<E>> UserContext<E, M> for EventContext<E, M> {
    fn current_tick(&self) -> SimTime {
        self.current_tick_status.current()
    }

    fn current_micro_step(&self) -> MicroStep {
        self.current_micro_step_status.current()
    }

    fn schedule_event(&mut self, delay: Duration, priority: EventPriority, event_payload: E) {
        self.event_scheduler
            .schedule(self.current_tick(), delay, priority, event_payload);
    }
}

impl<E, M: Model<E>> EventContext<E, M> {
    pub(crate) fn hook(&self) -> &impl Hook<E, M> {
        &self.hook_delegate
    }

    pub fn add_source<S>(&mut self, model: &M, name: &'static str, mut source: S)
    where
        S: Source<E, M> + 'static,
    {
        self.hook().before_register_source(model, name);
        let first_fire_delay = source.on_registered(self, model);
        self.source_handler.add_source_after_registered_action(
            name,
            self.current_tick(),
            first_fire_delay,
            source,
        );
        self.hook().after_register_source(model, name);
    }

    pub fn cancel_scheduled_sources<S, F>(
        &mut self,
        model: &M,
        pred: F,
    ) -> Vec<(SimTime, SourceReadyEntry)>
    where
        S: Source<E, M> + 'static,
        F: FnMut(SimTime, &SourceReadyEntry) -> bool,
    {
        let mut result = Vec::new();
        let now = self.current_tick();
        let micro_step = self.current_micro_step();
        let canceled = self.source_handler.drain_cancel_scheduled(pred);
        canceled.into_iter().for_each(|(scheduled_at, entry)| {
            self.hook().cancel_source(
                model,
                now,
                micro_step,
                scheduled_at,
                &SourceView::new(entry.source_id(), entry.clone_name_arc()),
            );
            result.push((scheduled_at, entry));
        });

        result
    }

    pub fn cancel_scheduled_events<F>(&mut self, model: &M, pred: F) -> Vec<(SimTime, Event<E>)>
    where
        F: FnMut(SimTime, &Event<E>) -> bool,
    {
        let mut result = Vec::new();
        let now = self.current_tick();
        let micro_step = self.current_micro_step();
        let canceled = self.event_scheduler.drain_cancel_scheduled(pred);
        canceled.into_iter().for_each(|(scheduled_at, event)| {
            self.hook()
                .cancel_event(model, now, micro_step, scheduled_at, &event);
            result.push((scheduled_at, event));
        });

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::SourceContext;

    #[derive(Debug, Clone, PartialEq)]
    struct TestEvent;

    struct TestModel;

    impl Model<TestEvent> for TestModel {
        fn handle_event(
            &mut self,
            _event_context: &mut EventContext<TestEvent, Self>,
            _event: &Event<TestEvent>,
        ) {
            // Do nothing for test
        }
    }

    struct TestSource {
        initial_delay: Duration,
    }

    impl Source<TestEvent, TestModel> for TestSource {
        fn on_registered(
            &mut self,
            context: &mut dyn UserContext<TestEvent, TestModel>,
            _model: &TestModel,
        ) -> Option<Duration> {
            context.schedule_event(Duration::ticks(5), EventPriority::minimum(), TestEvent);
            Some(self.initial_delay)
        }

        fn fire(
            &mut self,
            context: &mut SourceContext<TestEvent, TestModel>,
            _model: &TestModel,
        ) -> Option<Duration> {
            context.schedule_event(Duration::ticks(5), EventPriority::minimum(), TestEvent);
            Some(Duration::ticks(5))
        }
    }

    fn setup() -> EventContext<TestEvent, TestModel> {
        EventContext {
            current_tick_status: TickStatus::initialize(),
            current_micro_step_status: MicroStepStatus::new(MicroStep::zero()),
            hook_delegate: HookDelegate::new(),
            source_handler: SourceHandler::new(),
            event_scheduler: EventScheduler::new(),
        }
    }

    #[test]
    fn test_current_tick() {
        let context = setup();
        assert_eq!(context.current_tick(), SimTime::new(0));
    }

    #[test]
    fn test_current_micro_step() {
        let context = setup();
        assert_eq!(context.current_micro_step(), MicroStep::zero());
    }

    #[test]
    fn test_add_source_after() {
        let model = TestModel;
        let mut context = setup();
        let initial_sources_count = context.source_handler.ready_queue_len();
        let delay = Duration::ticks(10);
        context.add_source(
            &model,
            "test_source",
            TestSource {
                initial_delay: delay,
            },
        );
        context.source_handler.flush_pending();
        context.event_scheduler.flush_pending(); // Ensure event scheduler is also flushed if it was used by source initialization

        assert_eq!(
            context.source_handler.ready_queue_len(),
            initial_sources_count + 1
        );

        let (scheduled_at, scheduled_source) = context.source_handler.peek().unwrap();
        assert_eq!(scheduled_at, SimTime::new(0) + delay);
        assert_eq!(scheduled_source.source_id.value(), 0); // Assuming it's the first source added
    }

    #[test]
    fn test_add_source_at_now() {
        let model = TestModel;
        let mut context = setup();
        let initial_sources_count = context.source_handler.ready_queue_len();
        context.add_source(
            &model,
            "test_source_now",
            TestSource {
                initial_delay: Duration::zero(),
            },
        );
        context.source_handler.flush_pending();
        context.event_scheduler.flush_pending(); // Ensure event scheduler is also flushed if it was used by source initialization

        assert_eq!(
            context.source_handler.ready_queue_len(),
            initial_sources_count + 1
        );

        let (scheduled_at, scheduled_source) = context.source_handler.peek().unwrap();
        assert_eq!(scheduled_at, SimTime::new(0));
        assert_eq!(scheduled_source.source_id.value(), 0); // Assuming it's the first source added
    }

    #[test]
    fn test_schedule_event() {
        let mut context = setup();
        // 何個かあらかじめイベントを登録しておく
        context.event_scheduler.schedule(
            SimTime::new(0),
            Duration::one(),
            EventPriority::minimum(),
            TestEvent,
        );
        context.event_scheduler.schedule(
            SimTime::new(0),
            Duration::one(),
            EventPriority::minimum(),
            TestEvent,
        );
        context.event_scheduler.schedule(
            SimTime::new(0),
            Duration::one(),
            EventPriority::minimum(),
            TestEvent,
        );
        context.event_scheduler.flush_pending();

        let initial_scheduled_events_count = context.event_scheduler.ready_queue_len();
        let delay = Duration::ticks(5);
        let priority = EventPriority::new(10);
        let event_payload = TestEvent;

        context.schedule_event(delay, priority, event_payload.clone());
        context.source_handler.flush_pending(); // Ensure source handler is also flushed
        context.event_scheduler.flush_pending();

        assert_eq!(
            context.event_scheduler.ready_queue_len(),
            initial_scheduled_events_count + 1
        );

        let (scheduled_at, event) = context.event_scheduler.peek().unwrap();
        assert_eq!(scheduled_at, SimTime::new(1));
        assert_eq!(event.priority, EventPriority::minimum());
        assert_eq!(event.payload, TestEvent);
    }

    #[test]
    fn test_cancel_scheduled_events() {
        let mut context = setup();
        let model = TestModel;

        context.schedule_event(Duration::ticks(5), EventPriority::minimum(), TestEvent);
        context.schedule_event(Duration::ticks(10), EventPriority::minimum(), TestEvent);
        context.source_handler.flush_pending();
        context.event_scheduler.flush_pending();

        let canceled_events = context.cancel_scheduled_events(&model, |_, _| true);
        assert_eq!(canceled_events.len(), 2);
        assert_eq!(context.event_scheduler.ready_queue_len(), 0);

        context.schedule_event(Duration::ticks(5), EventPriority::minimum(), TestEvent);
        context.schedule_event(Duration::ticks(10), EventPriority::minimum(), TestEvent);

        context.source_handler.flush_pending();
        context.event_scheduler.flush_pending();

        let canceled_events_filtered = context
            .cancel_scheduled_events(&model, |scheduled_at, _| scheduled_at == SimTime::new(5));
        assert_eq!(canceled_events_filtered.len(), 1);
        assert_eq!(canceled_events_filtered[0].0, SimTime::new(5));
        assert_eq!(context.event_scheduler.ready_queue_len(), 1);
        let (remaining_scheduled_at, _) = context.event_scheduler.peek().unwrap();
        assert_eq!(remaining_scheduled_at, SimTime::new(10));
    }

    #[test]
    fn test_cancel_scheduled_sources() {
        let mut context = setup();
        let model = TestModel;

        context.add_source(
            &model,
            "source1",
            TestSource {
                initial_delay: Duration::ticks(5),
            },
        );
        context.add_source(
            &model,
            "source2",
            TestSource {
                initial_delay: Duration::ticks(10),
            },
        );
        context.source_handler.flush_pending();
        context.event_scheduler.flush_pending();

        let canceled_sources =
            context.cancel_scheduled_sources::<TestSource, _>(&model, |_, _| true);
        assert_eq!(canceled_sources.len(), 2);
        assert_eq!(context.source_handler.ready_queue_len(), 0);

        context.add_source(
            &model,
            "source3",
            TestSource {
                initial_delay: Duration::ticks(5),
            },
        );
        context.add_source(
            &model,
            "source4",
            TestSource {
                initial_delay: Duration::ticks(10),
            },
        );
        context.source_handler.flush_pending();
        context.event_scheduler.flush_pending();

        let canceled_sources_filtered = context
            .cancel_scheduled_sources::<TestSource, _>(&model, |scheduled_at, _| {
                scheduled_at == SimTime::new(5)
            });
        assert_eq!(canceled_sources_filtered.len(), 1);
        assert_eq!(canceled_sources_filtered[0].0, SimTime::new(5));
        assert_eq!(context.source_handler.ready_queue_len(), 1);
        let (remaining_scheduled_at, _) = context.source_handler.peek().unwrap();
        assert_eq!(remaining_scheduled_at, SimTime::new(10));
    }
}
