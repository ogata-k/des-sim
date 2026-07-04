use crate::event_scheduler::EventScheduler;
use crate::execution::phase::MicroStepHandler;
use crate::execution::{SimulationError, SimulationOutput, SimulationResult};
use crate::modeling::event::Event;
use crate::modeling::hook::Hook;
use crate::modeling::hook::instance::HookDelegate;
use crate::modeling::model::Model;
use crate::primitive::time::{Duration, SimTime};
use crate::primitive::time::{MicroStepStatus, TickStatus};
use crate::source_handler::{SourceHandler, SourceReadyEntry};
use std::cmp::min;
use std::collections::VecDeque;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum ExecutorStatus {
    NoMoreEvent,
    ExistsMoreEvent,
}

// Runner開発者向けのContextなのでUserContextは実装していない
pub struct ExecutorContext<E, M: Model<E>> {
    pub(crate) next_tick_status: TickStatus,
    pub(crate) current_tick: SimTime,
    pub(crate) hook_delegate: HookDelegate<E, M>,
    pub(crate) source_handler: SourceHandler<E, M>,
    pub(crate) event_scheduler: EventScheduler<E>,
}

impl<E, M: Model<E>> ExecutorContext<E, M> {
    pub(crate) fn hook(&self) -> &impl Hook<E, M> {
        &self.hook_delegate
    }

    pub fn peek_next_tick(&self) -> (ExecutorStatus, TickStatus) {
        let next_event_fired_at = self.event_scheduler.peek_next_time();
        let executor_status = if next_event_fired_at.is_some() {
            ExecutorStatus::ExistsMoreEvent
        } else {
            ExecutorStatus::NoMoreEvent
        };

        (executor_status, self.next_tick_status)
    }

    pub fn begin_tick(self, model: &M) -> ActiveExecutorContext<E, M> {
        self.hook_delegate.before_tick(
            model,
            self.next_tick_status.current(),
            self.next_tick_status.skipped(),
        );

        ActiveExecutorContext {
            // ここからは現在のTickStatus
            current_tick_status: self.next_tick_status,
            next_micro_step_status: MicroStepStatus::initialize(),
            hook_delegate: self.hook_delegate,
            source_handler: self.source_handler,
            event_scheduler: self.event_scheduler,
        }
    }

    pub fn end_simulation_as_ok<Err>(self, model: M) -> SimulationResult<M, Err> {
        self.hook().after_simulation(&model, self.current_tick);
        Ok(SimulationOutput::new(self.current_tick, model))
    }

    pub fn end_simulation_as_error<Err>(self, model: M, error: Err) -> SimulationResult<M, Err> {
        self.hook().after_simulation(&model, self.current_tick);
        Err(SimulationError::new(self.current_tick, model, error))
    }
}

pub struct ActiveExecutorContext<E, M: Model<E>> {
    pub(crate) current_tick_status: TickStatus,
    // 現在時刻が確定しているタイミングなのでTickStatusは現在の状態を表すものだが、
    // まだMicroStepは始まっていないのでMicroStepStatusは未来の状態。
    pub(crate) next_micro_step_status: MicroStepStatus,
    pub(crate) hook_delegate: HookDelegate<E, M>,
    pub(crate) source_handler: SourceHandler<E, M>,
    pub(crate) event_scheduler: EventScheduler<E>,
}

impl<E, M: Model<E>> ActiveExecutorContext<E, M> {
    pub(crate) fn hook(&self) -> &impl Hook<E, M> {
        &self.hook_delegate
    }

    pub fn begin_micro_step(self, model: &M) -> MicroStepHandler<ActiveExecutorContext<E, M>> {
        self.hook().before_micro_step(
            model,
            self.current_tick_status.current(),
            self.next_micro_step_status.current(),
        );

        MicroStepHandler::new(self)
    }

    pub fn end_tick_with_increment_tick(self, model: &M) -> ExecutorContext<E, M> {
        let current_tick = self.current_tick_status.current();
        self.hook()
            .after_tick(model, current_tick, self.next_micro_step_status.current());
        let next_tick = current_tick + Duration::one();
        let next_tick_status = TickStatus::new(next_tick, Duration::zero());

        ExecutorContext {
            next_tick_status,
            current_tick,
            hook_delegate: self.hook_delegate,
            source_handler: self.source_handler,
            event_scheduler: self.event_scheduler,
        }
    }

    pub fn end_tick_with_jump_to_next_tick(self, model: &M) -> ExecutorContext<E, M> {
        let current_tick = self.current_tick_status.current();
        self.hook()
            .after_tick(model, current_tick, self.next_micro_step_status.current());

        let (skipped, next_tick) = match (
            self.source_handler.peek_next_time(),
            self.event_scheduler.peek_next_time(),
        ) {
            (Some(next_scheduled_at), None) | (None, Some(next_scheduled_at)) => (
                next_scheduled_at - current_tick - Duration::one(),
                next_scheduled_at,
            ),
            (Some(source_next_scheduled_at), Some(event_next_scheduled_at)) => {
                let next_scheduled_at = min(source_next_scheduled_at, event_next_scheduled_at);
                (
                    next_scheduled_at - current_tick - Duration::one(),
                    next_scheduled_at,
                )
            }
            (_, _) => {
                // 次に発火させるべきものがないので次へ順番に進めておく
                (Duration::zero(), current_tick + Duration::one())
            }
        };
        let next_tick_status = TickStatus::new(next_tick, skipped);

        ExecutorContext {
            next_tick_status,
            current_tick,
            hook_delegate: self.hook_delegate,
            source_handler: self.source_handler,
            event_scheduler: self.event_scheduler,
        }
    }

    pub fn discard_remain_micro_step(
        &mut self,
        model: &M,
    ) -> (VecDeque<SourceReadyEntry>, VecDeque<Event<E>>) {
        let current_tick = self.current_tick_status.current();
        let mut ready_sources = self.source_handler.drain_ready(current_tick);
        let mut ready_events = self.event_scheduler.drain_ready(current_tick);

        self.hook().on_discard_remain_micro_step(
            model,
            current_tick,
            self.next_micro_step_status.current(),
            ready_sources.make_contiguous(),
            ready_events.make_contiguous(),
        );

        (ready_sources, ready_events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{EventContext, SourceContext, UserContext};
    use crate::modeling::event::{Event, EventPriority};
    use crate::modeling::source::Source;
    use crate::primitive::time::{Duration, MicroStep, SimTime};

    // Mock Event and Source for testing
    #[derive(Debug, PartialEq, Eq, Copy, Clone)]
    struct TestEvent;

    #[derive(Debug, PartialEq, Eq, Copy, Clone)]
    struct TestSource {
        tick: Duration,
    }

    impl Source<TestEvent, TestModel> for TestSource {
        fn on_registered(
            &mut self,
            _context: &mut dyn UserContext<TestEvent, TestModel>,
            _model: &TestModel,
        ) -> Option<Duration> {
            Some(self.tick)
        }

        fn fire(
            &mut self,
            _context: &mut SourceContext<TestEvent, TestModel>,
            _model: &TestModel,
        ) -> Option<Duration> {
            Some(self.tick)
        }
    }

    #[derive(Debug)]
    struct TestModel;

    impl Model<TestEvent> for TestModel {
        fn handle_event(
            &mut self,
            _context: &mut EventContext<TestEvent, Self>,
            _event: &Event<TestEvent>,
        ) {
            // none
        }
    }

    fn create_mock_executor_context(
        current_tick: SimTime,
        next_tick_status: TickStatus,
    ) -> ExecutorContext<TestEvent, TestModel> {
        ExecutorContext {
            next_tick_status,
            current_tick,
            hook_delegate: HookDelegate::new(),
            source_handler: SourceHandler::new(),
            event_scheduler: EventScheduler::new(),
        }
    }

    #[test]
    fn test_executor_context_peek_next_tick_no_event() {
        let current_tick = SimTime::new(0);
        let next_tick_status = TickStatus::new(SimTime::new(1), Duration::zero());

        let context = create_mock_executor_context(current_tick, next_tick_status);

        let (status, tick_status) = context.peek_next_tick();
        assert_eq!(status, ExecutorStatus::NoMoreEvent);
        assert_eq!(tick_status, next_tick_status);
    }

    #[test]
    fn test_executor_context_peek_next_tick_with_event() {
        let current_tick = SimTime::new(0);
        let next_tick_status = TickStatus::new(SimTime::new(1), Duration::zero());
        let mut context = create_mock_executor_context(current_tick, next_tick_status);
        context.event_scheduler.schedule(
            SimTime::new(5),
            Duration::ticks(0),
            EventPriority::minimum(),
            TestEvent,
        );
        context.event_scheduler.flush_pending();

        assert_eq!(
            context.event_scheduler.peek_next_time(),
            Some(SimTime::new(5))
        );

        let (status, tick_status) = context.peek_next_tick();
        assert_eq!(status, ExecutorStatus::ExistsMoreEvent);
        assert_eq!(tick_status, next_tick_status);
    }

    #[test]
    fn test_executor_context_begin_tick() {
        let current_tick = SimTime::new(0);
        let next_tick_status = TickStatus::new(SimTime::new(1), Duration::zero());
        let context = create_mock_executor_context(current_tick, next_tick_status);

        let model = TestModel;
        let active_context = context.begin_tick(&model);

        assert_eq!(active_context.current_tick_status, next_tick_status);
        assert_eq!(
            active_context.next_micro_step_status,
            MicroStepStatus::initialize()
        );
    }

    #[test]
    fn test_executor_context_end_simulation_as_ok() {
        let current_tick = SimTime::new(10);
        let next_tick_status = TickStatus::new(SimTime::new(11), Duration::zero());
        let context = create_mock_executor_context(current_tick, next_tick_status);

        let model = TestModel;
        let result: SimulationResult<TestModel, &'static str> = context.end_simulation_as_ok(model);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.last_tick(), current_tick);
    }

    #[test]
    fn test_executor_context_end_simulation_as_error() {
        let current_tick = SimTime::new(10);
        let next_tick_status = TickStatus::new(SimTime::new(11), Duration::zero());
        let context = create_mock_executor_context(current_tick, next_tick_status);

        let model = TestModel;
        let error_msg = "Simulation failed";
        let result = context.end_simulation_as_error(model, error_msg);

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.last_tick(), current_tick);
        assert_eq!(error.error(), &error_msg);
    }

    // ActiveExecutorContext Tests
    fn create_mock_active_executor_context(
        current_tick_status: TickStatus,
        next_micro_step_status: MicroStepStatus,
    ) -> ActiveExecutorContext<TestEvent, TestModel> {
        ActiveExecutorContext {
            current_tick_status,
            next_micro_step_status,
            hook_delegate: HookDelegate::new(),
            source_handler: SourceHandler::new(),
            event_scheduler: EventScheduler::new(),
        }
    }

    #[test]
    fn test_active_executor_context_begin_micro_step() {
        let current_tick_status = TickStatus::new(SimTime::new(5), Duration::zero());
        let next_micro_step_status = MicroStepStatus::new(MicroStep::zero());
        let context =
            create_mock_active_executor_context(current_tick_status, next_micro_step_status);

        let model = TestModel;
        let micro_step_handler = context.begin_micro_step(&model);

        assert_eq!(
            micro_step_handler.ref_context().current_tick_status,
            current_tick_status
        );
    }

    #[test]
    fn test_active_executor_context_end_tick_with_increment_tick() {
        let current_tick_status = TickStatus::new(SimTime::new(5), Duration::zero());
        let mut micro_step_ten = MicroStep::zero();
        for _ in 0..10 {
            micro_step_ten = micro_step_ten.next();
        }
        let next_micro_step_status = MicroStepStatus::new(micro_step_ten);
        let context =
            create_mock_active_executor_context(current_tick_status, next_micro_step_status);

        let model = TestModel;
        let next_context = context.end_tick_with_increment_tick(&model);

        assert_eq!(next_context.current_tick, SimTime::new(5));
        assert_eq!(next_context.next_tick_status.current(), SimTime::new(6));
        assert_eq!(next_context.next_tick_status.skipped(), Duration::zero());
    }

    #[test]
    fn test_active_executor_context_end_tick_with_jump_to_next_tick_only_source() {
        let current_tick_status = TickStatus::new(SimTime::new(5), Duration::zero());
        let mut micro_step_ten = MicroStep::zero();
        for _ in 0..10 {
            micro_step_ten = micro_step_ten.next();
        }
        let next_micro_step_status = MicroStepStatus::new(micro_step_ten);
        let mut context =
            create_mock_active_executor_context(current_tick_status, next_micro_step_status);
        context.source_handler.add_source_after_registered_action(
            "test source",
            current_tick_status.current(),
            Some(Duration::ticks(7)),
            TestSource {
                tick: Duration::ticks(3),
            },
        );
        context.source_handler.flush_pending();

        let model = TestModel;
        let next_context = context.end_tick_with_jump_to_next_tick(&model);

        assert_eq!(next_context.current_tick, SimTime::new(5));
        assert_eq!(next_context.next_tick_status.current(), SimTime::new(12));
        assert_eq!(next_context.next_tick_status.skipped(), Duration::ticks(6));
    }

    #[test]
    fn test_active_executor_context_end_tick_with_jump_to_next_tick_only_event() {
        let current_tick_status = TickStatus::new(SimTime::new(5), Duration::zero());
        let mut micro_step_ten = MicroStep::zero();
        for _ in 0..10 {
            micro_step_ten = micro_step_ten.next();
        }
        let next_micro_step_status = MicroStepStatus::new(micro_step_ten);
        let mut context =
            create_mock_active_executor_context(current_tick_status, next_micro_step_status);
        context.event_scheduler.schedule(
            SimTime::new(5),
            Duration::ticks(5),
            EventPriority::minimum(),
            TestEvent,
        );
        context.event_scheduler.flush_pending();

        let model = TestModel;
        let next_context = context.end_tick_with_jump_to_next_tick(&model);

        assert_eq!(next_context.current_tick, SimTime::new(5));
        assert_eq!(next_context.next_tick_status.current(), SimTime::new(10));
        assert_eq!(next_context.next_tick_status.skipped(), Duration::ticks(4));
    }

    #[test]
    fn test_active_executor_context_end_tick_with_jump_to_next_tick_both_event_and_source() {
        let current_tick_status = TickStatus::new(SimTime::new(5), Duration::zero());
        let mut micro_step_ten = MicroStep::zero();
        for _ in 0..10 {
            micro_step_ten = micro_step_ten.next();
        }
        let next_micro_step_status = MicroStepStatus::new(micro_step_ten);
        let mut context =
            create_mock_active_executor_context(current_tick_status, next_micro_step_status);
        context.source_handler.add_source_after_registered_action(
            "test source",
            current_tick_status.current(),
            Some(Duration::ticks(7)),
            TestSource {
                tick: Duration::ticks(3),
            },
        );
        context.event_scheduler.schedule(
            SimTime::new(5),
            Duration::ticks(5),
            EventPriority::minimum(),
            TestEvent,
        );
        context.source_handler.flush_pending();
        context.event_scheduler.flush_pending();

        let model = TestModel;
        let next_context = context.end_tick_with_jump_to_next_tick(&model);

        assert_eq!(next_context.current_tick, SimTime::new(5));
        assert_eq!(next_context.next_tick_status.current(), SimTime::new(10)); // min(10, 12) = 10
        assert_eq!(next_context.next_tick_status.skipped(), Duration::ticks(4));
    }

    #[test]
    fn test_active_executor_context_end_tick_with_jump_to_next_tick_no_next() {
        let current_tick_status = TickStatus::new(SimTime::new(5), Duration::zero());
        let mut micro_step_ten = MicroStep::zero();
        for _ in 0..10 {
            micro_step_ten = micro_step_ten.next();
        }
        let next_micro_step_status = MicroStepStatus::new(micro_step_ten);
        let context =
            create_mock_active_executor_context(current_tick_status, next_micro_step_status);

        let model = TestModel;
        let next_context = context.end_tick_with_jump_to_next_tick(&model);

        assert_eq!(next_context.current_tick, SimTime::new(5));
        assert_eq!(next_context.next_tick_status.current(), SimTime::new(6));
        assert_eq!(next_context.next_tick_status.skipped(), Duration::zero());
    }

    #[test]
    fn test_active_executor_context_discard_remain_micro_step() {
        let current_tick_status = TickStatus::new(SimTime::new(5), Duration::zero());
        let mut micro_step_ten = MicroStep::zero();
        for _ in 0..10 {
            micro_step_ten = micro_step_ten.next();
        }
        let next_micro_step_status = MicroStepStatus::new(micro_step_ten);
        let mut context =
            create_mock_active_executor_context(current_tick_status, next_micro_step_status);
        context.source_handler.add_source_after_registered_action(
            "test source",
            current_tick_status.current(),
            Some(Duration::ticks(0)),
            TestSource {
                tick: Duration::ticks(3),
            },
        );
        context.event_scheduler.schedule(
            SimTime::new(5),
            Duration::ticks(0),
            EventPriority::minimum(),
            TestEvent,
        );
        context.event_scheduler.schedule(
            SimTime::new(5),
            Duration::ticks(0),
            EventPriority::minimum(),
            TestEvent,
        );
        context.source_handler.flush_pending();
        context.event_scheduler.flush_pending();

        let model = TestModel;
        let (discarded_sources, discarded_events) = context.discard_remain_micro_step(&model);
        assert_eq!(discarded_sources.len(), 1);
        assert_eq!(discarded_sources[0].name(), "test source");

        assert_eq!(discarded_events.len(), 2);
        assert_eq!(discarded_events[0].payload, TestEvent);
        assert_eq!(discarded_events[1].payload, TestEvent);
    }
}
