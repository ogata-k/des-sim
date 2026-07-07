use crate::context::UserContext;
use crate::event_scheduler::EventScheduler;
use crate::modeling::event::EventPriority;
use crate::modeling::hook::Hook;
use crate::modeling::hook::instance::HookDelegate;
use crate::modeling::model::Model;
use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::primitive::time::{MicroStepStatus, TickStatus};
use crate::source_handler::SourceHandler;

pub struct SourceContext<E, M: Model<E>> {
    pub(crate) current_tick_status: TickStatus,
    pub(crate) current_micro_step_status: MicroStepStatus,
    pub(crate) hook_delegate: HookDelegate<E, M>,
    // SourceContextはSourceを詰めなおすときに発火させてから詰めなおす都合上、SourceContextを持っているとライフタイムの問題が発生する。
    // そのため、MicroStepHandlerに渡す時だけSourceContextをSourcePhaseから奪い取る形で実装されている。
    pub(crate) source_handler: Option<SourceHandler<E, M>>,
    pub(crate) event_scheduler: EventScheduler<E>,
}

impl<E, M: Model<E>> UserContext<E, M> for SourceContext<E, M> {
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

impl<E, M: Model<E>> SourceContext<E, M> {
    pub(crate) fn hook(&self) -> &impl Hook<E, M> {
        &self.hook_delegate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::EventContext;
    use crate::event_scheduler::EventScheduler;
    use crate::modeling::event::{Event, EventPriority};
    use crate::modeling::hook::instance::HookDelegate;
    use crate::modeling::model::Model;
    use crate::primitive::time::{Duration, MicroStep, SimTime};
    use crate::primitive::time::{MicroStepStatus, TickStatus};

    #[derive(Debug, PartialEq)]
    enum TestEvent {
        EventA,
    }

    struct TestModel;

    impl Model<TestEvent> for TestModel {
        fn handle_event(
            &mut self,
            _context: &mut EventContext<TestEvent, Self>,
            _event: &Event<TestEvent>,
        ) {
        }
    }

    fn create_test_source_context() -> SourceContext<TestEvent, TestModel> {
        SourceContext {
            current_tick_status: TickStatus::new(SimTime::from_ticks(0), Duration::zero()),
            current_micro_step_status: MicroStepStatus::new(MicroStep::zero()),
            hook_delegate: HookDelegate::new(),
            source_handler: None,
            event_scheduler: EventScheduler::new(),
        }
    }

    #[test]
    fn test_current_tick() {
        let context = create_test_source_context();
        assert_eq!(context.current_tick(), SimTime::from_ticks(0));
    }

    #[test]
    fn test_current_micro_step() {
        let context = create_test_source_context();
        assert_eq!(context.current_micro_step(), MicroStep::zero());
    }

    #[test]
    fn test_schedule_event() {
        let mut context = create_test_source_context();
        // 何個かあらかじめイベントを登録しておく
        context.event_scheduler.schedule(
            SimTime::from_ticks(0),
            Duration::one(),
            EventPriority::minimum(),
            TestEvent::EventA,
        );
        context.event_scheduler.schedule(
            SimTime::from_ticks(0),
            Duration::one(),
            EventPriority::minimum(),
            TestEvent::EventA,
        );
        context.event_scheduler.schedule(
            SimTime::from_ticks(0),
            Duration::one(),
            EventPriority::minimum(),
            TestEvent::EventA,
        );
        // イベントを取得可能の待機列に移動
        context.event_scheduler.flush_pending();
        let initial_event_count = context.event_scheduler.ready_queue_len();

        context.schedule_event(Duration::one(), EventPriority::minimum(), TestEvent::EventA);
        // イベントを取得可能の待機列に移動
        context.event_scheduler.flush_pending();

        assert_eq!(
            context.event_scheduler.ready_queue_len(),
            initial_event_count + 1
        );
    }
}
