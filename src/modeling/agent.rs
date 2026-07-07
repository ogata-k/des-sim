use crate::context::{EventContext, UserContext};
use crate::modeling::event::EventPriority;
use crate::modeling::model::Model;
use crate::primitive::time::Duration;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::rc::Rc;

pub struct AgentStep<E, M: Model<E>> {
    /// このステップが「シミュレーション上、何をする段階か」を表すメタデータ。キャンセル時に利用を想定。
    pub tag: &'static str,
    pub delay: Duration,
    pub priority: EventPriority,
    #[allow(clippy::type_complexity)]
    pub logic: Box<dyn FnOnce(&mut EventContext<E, M>, &mut M, &mut VecDeque<AgentStep<E, M>>)>,
}

impl<E, M: Model<E>> fmt::Debug for AgentStep<E, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentStep")
            .field("tag", &self.tag)
            .field("delay", &self.delay)
            .field("priority", &self.priority)
            // トレイトオブジェクトは中身が見えないため、型名と関数ポインタのアドレスを出力
            .field("logic", &format_args!("Box<dyn FnOnce>({:p})", self.logic))
            .finish()
    }
}

pub struct AgentContinuation<E, M: Model<E>> {
    future_steps: VecDeque<AgentStep<E, M>>,
    to_event_payload: Rc<dyn Fn(AgentContinuation<E, M>) -> E>,
}

impl<E, M: Model<E>> fmt::Debug for AgentContinuation<E, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentContinuation")
            .field("future_steps", &self.future_steps)
            // Rc の内部ポインタアドレスを出力して識別できるようにする
            .field(
                "to_event_payload",
                &format_args!("Rc<dyn Fn>({:p})", Rc::as_ptr(&self.to_event_payload)),
            )
            .finish()
    }
}

impl<E: 'static, M: Model<E> + 'static> AgentContinuation<E, M> {
    pub fn new<F>(to_event_payload: F) -> Self
    where
        F: Fn(AgentContinuation<E, M>) -> E + 'static,
    {
        Self {
            future_steps: VecDeque::new(),
            to_event_payload: Rc::new(to_event_payload),
        }
    }

    pub fn then_after<F>(
        mut self,
        tag: &'static str,
        delay: Duration,
        priority: EventPriority,
        logic: F,
    ) -> Self
    where
        F: FnOnce(&mut EventContext<E, M>, &mut M, &mut VecDeque<AgentStep<E, M>>) + 'static,
    {
        self.future_steps.push_back(AgentStep {
            tag,
            delay,
            priority,
            logic: Box::new(logic),
        });

        self
    }
}

impl<E, M: Model<E>> AgentContinuation<E, M> {
    pub fn peek_next_step(&self) -> Option<&AgentStep<E, M>> {
        self.future_steps.front()
    }

    pub fn peek_next_step_tag(&self) -> Option<&'static str> {
        self.peek_next_step().map(|step| step.tag)
    }

    pub fn get_remain_step_count(&self) -> usize {
        self.future_steps.len()
    }

    /// 今回のステップを1つ消費して実行し、そのまま次をスケジュールする
    /// (もし最後のステップだったら、実行だけして綺麗に終了する)
    pub fn execute_and_schedule(mut self, context: &mut EventContext<E, M>, model: &mut M) {
        // 今回のステップを内部で取り出す
        if let Some(current_step) = self.future_steps.pop_front() {
            // 処理できるイベントを処理
            // このlogic内部でステップが上書きされる可能性がある。
            (current_step.logic)(context, model, &mut self.future_steps);

            // 次のステップがあることを確認し、次のステップがあるなら次の時刻に記録しておく
            let next_info = self.future_steps.front().map(|s| (s.delay, s.priority));
            if let Some((next_delay, next_priority)) = next_info {
                // 参照を複製して次に渡す
                let to_event_payload = Rc::clone(&self.to_event_payload);
                let next_payload = to_event_payload(self);

                context.schedule_event(next_delay, next_priority, next_payload);
            }
        }
    }
}

pub struct AgentActionTicket<E, M: Model<E>> {
    action: RefCell<Option<AgentContinuation<E, M>>>,
}

impl<E, M: Model<E>> fmt::Debug for AgentActionTicket<E, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.action.borrow().as_ref() {
            Some(continuation) => fmt::Debug::fmt(continuation, f),
            None => write!(f, "ExecutedAction"),
        }
    }
}

impl<E, M: Model<E>> AgentActionTicket<E, M> {
    pub fn issue(continuation: AgentContinuation<E, M>) -> Self {
        Self {
            action: RefCell::new(Some(continuation)),
        }
    }

    pub fn execute(&self) -> Option<AgentContinuation<E, M>> {
        self.action.borrow_mut().take()
    }

    pub fn inspect<R, F>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&AgentContinuation<E, M>) -> R,
    {
        self.action.borrow().as_ref().map(f)
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
    use crate::primitive::time::{Duration, MicroStepStatus, SimTime, TickStatus};
    use crate::source_handler::SourceHandler;
    use std::assert_matches;
    use std::cell::RefCell;
    use std::rc::Rc;

    // Dummy Event for testing
    #[derive(Debug)]
    enum TestEvent {
        AgentContinuationEvent(AgentActionTicket<TestEvent, TestModel>),
        SimpleEvent(u32),
    }

    // Dummy Model for testing
    #[derive(Debug, Default)]
    struct TestModel {
        pub counter: Rc<RefCell<u32>>,
    }

    impl Model<TestEvent> for TestModel {
        fn handle_event(
            &mut self,
            context: &mut EventContext<TestEvent, Self>,
            event: &Event<TestEvent>,
        ) {
            match &event.payload {
                TestEvent::AgentContinuationEvent(ticket) => {
                    if let Some(continuation) = ticket.execute() {
                        continuation.execute_and_schedule(context, self);
                    }
                }
                TestEvent::SimpleEvent(val) => {
                    *self.counter.borrow_mut() += val;
                }
            }
        }
    }

    fn create_mock_event_context() -> EventContext<TestEvent, TestModel> {
        EventContext {
            current_tick_status: TickStatus::initialize(),
            current_micro_step_status: MicroStepStatus::initialize(),
            hook_delegate: HookDelegate::new(),
            source_handler: SourceHandler::new(),
            event_scheduler: EventScheduler::new(),
        }
    }

    #[test]
    fn test_agent_step_creation() {
        let step = AgentStep {
            tag: "test_step",
            delay: Duration::ticks(1),
            priority: EventPriority::new(5),
            logic: Box::new(|_, _: &mut TestModel, _| {}),
        };

        assert_eq!(step.tag, "test_step");
        assert_eq!(step.delay, Duration::ticks(1));
        assert_eq!(step.priority, EventPriority::new(5));
    }

    #[test]
    fn test_agent_continuation_new() {
        let continuation: AgentContinuation<TestEvent, TestModel> =
            AgentContinuation::new(|_| TestEvent::SimpleEvent(0));
        assert_eq!(continuation.get_remain_step_count(), 0);
        assert!(continuation.future_steps.is_empty());
    }

    #[test]
    fn test_agent_continuation_then_after() {
        let continuation: AgentContinuation<TestEvent, TestModel> =
            AgentContinuation::new(|_| TestEvent::SimpleEvent(0))
                .then_after(
                    "step1",
                    Duration::ticks(1),
                    EventPriority::new(5),
                    |_, _, _| {},
                )
                .then_after(
                    "step2",
                    Duration::ticks(2),
                    EventPriority::new(10),
                    |_, _, _| {},
                );

        assert_eq!(continuation.get_remain_step_count(), 2);

        let step1 = continuation.future_steps.get(0).unwrap();
        assert_eq!(step1.tag, "step1");
        assert_eq!(step1.delay, Duration::ticks(1));
        assert_eq!(step1.priority, EventPriority::new(5));

        let step2 = continuation.future_steps.get(1).unwrap();
        assert_eq!(step2.tag, "step2");
        assert_eq!(step2.delay, Duration::ticks(2));
        assert_eq!(step2.priority, EventPriority::new(10));
    }

    #[test]
    fn test_agent_continuation_peek_next_step() {
        let continuation: AgentContinuation<TestEvent, TestModel> =
            AgentContinuation::new(|_| TestEvent::SimpleEvent(0))
                .then_after(
                    "first",
                    Duration::ticks(1),
                    EventPriority::new(5),
                    |_, _, _| {},
                )
                .then_after(
                    "second",
                    Duration::ticks(2),
                    EventPriority::new(10),
                    |_, _, _| {},
                );

        assert_eq!(continuation.peek_next_step_tag(), Some("first"));
        assert_eq!(
            continuation.peek_next_step().unwrap().delay,
            Duration::ticks(1)
        );
    }

    #[test]
    fn test_agent_continuation_get_remain_step_count() {
        let continuation: AgentContinuation<TestEvent, TestModel> =
            AgentContinuation::new(|_| TestEvent::SimpleEvent(0))
                .then_after(
                    "s1",
                    Duration::ticks(1),
                    EventPriority::new(5),
                    |_, _, _| {},
                )
                .then_after(
                    "s2",
                    Duration::ticks(2),
                    EventPriority::new(10),
                    |_, _, _| {},
                )
                .then_after(
                    "s3",
                    Duration::ticks(3),
                    EventPriority::new(0),
                    |_, _, _| {},
                );

        assert_eq!(continuation.get_remain_step_count(), 3);
    }

    #[test]
    fn test_agent_continuation_execute_and_schedule_single_step() {
        let counter = Rc::new(RefCell::new(0));
        let mut context = create_mock_event_context();
        let mut model = TestModel {
            counter: Rc::clone(&counter),
        };

        let continuation: AgentContinuation<TestEvent, TestModel> =
            AgentContinuation::new(|_| TestEvent::SimpleEvent(0)).then_after(
                "single_step",
                Duration::ticks(1),
                EventPriority::new(5),
                |_, model: &mut TestModel, _| {
                    *model.counter.borrow_mut() += 10;
                },
            );

        continuation.execute_and_schedule(&mut context, &mut model);

        assert_eq!(*counter.borrow(), 10);
    }

    #[test]
    fn test_agent_continuation_execute_and_schedule_multiple_steps() {
        let counter = Rc::new(RefCell::new(0));
        let mut context = create_mock_event_context();
        let mut model = TestModel {
            counter: Rc::clone(&counter),
        };

        let continuation: AgentContinuation<TestEvent, TestModel> = AgentContinuation::new(|c| {
            TestEvent::AgentContinuationEvent(AgentActionTicket::issue(c))
        })
        .then_after(
            "step1",
            Duration::ticks(1),
            EventPriority::new(5),
            |_, model, _| {
                *model.counter.borrow_mut() += 1;
            },
        )
        .then_after(
            "step2",
            Duration::ticks(2),
            EventPriority::new(10),
            |_, model, _| {
                *model.counter.borrow_mut() += 10;
            },
        );

        context.schedule_event(
            Duration::one(),
            EventPriority::minimum(),
            TestEvent::AgentContinuationEvent(AgentActionTicket::issue(continuation)),
        );
        context.event_scheduler.flush_pending();
        let events = context.event_scheduler.drain_ready(SimTime::from_ticks(1));
        for event in events {
            model.handle_event(&mut context, &event);
        }

        assert_eq!(*counter.borrow(), 1); // Only step1 logic should have run

        context.event_scheduler.flush_pending();
        let mut scheduled_events = context.event_scheduler.drain_ready(SimTime::from_ticks(2));

        assert_eq!(scheduled_events.len(), 1);
        let event = scheduled_events.pop_front().unwrap();
        assert_eq!(event.priority, EventPriority::new(10));
        assert_matches!(event.payload, TestEvent::AgentContinuationEvent(_));

        if let TestEvent::AgentContinuationEvent(ticket) = event.payload {
            assert_eq!(
                ticket.inspect(|c| c.peek_next_step_tag()).unwrap(),
                Some("step2")
            );
        } else {
            panic!("Expected AgentContinuationEvent");
        }
    }

    #[test]
    fn test_agent_continuation_execute_and_schedule_no_steps() {
        let counter = Rc::new(RefCell::new(0));
        let mut context = create_mock_event_context();
        let mut model = TestModel {
            counter: Rc::clone(&counter),
        };

        let continuation: AgentContinuation<TestEvent, TestModel> =
            AgentContinuation::new(|_| TestEvent::SimpleEvent(0));

        continuation.execute_and_schedule(&mut context, &mut model);
        context.event_scheduler.flush_pending();

        assert_eq!(*counter.borrow(), 0);
        assert_eq!(context.event_scheduler.ready_queue_len(), 0);
    }

    #[test]
    fn test_agent_action_ticket_issue_and_execute() {
        let continuation: AgentContinuation<TestEvent, TestModel> =
            AgentContinuation::new(|_| TestEvent::SimpleEvent(0));
        let ticket = AgentActionTicket::issue(continuation);

        let executed_continuation = ticket.execute().unwrap();
        assert_eq!(executed_continuation.get_remain_step_count(), 0);

        assert!(ticket.execute().is_none()); // Should be None after first execution
    }

    #[test]
    fn test_agent_action_ticket_inspect() {
        let continuation: AgentContinuation<TestEvent, TestModel> =
            AgentContinuation::new(|_| TestEvent::SimpleEvent(0)).then_after(
                "inspect_step",
                Duration::ticks(1),
                EventPriority::new(5),
                |_, _, _| {},
            );
        let ticket = AgentActionTicket::issue(continuation);

        let tag = ticket.inspect(|c| c.peek_next_step_tag().unwrap()).unwrap();
        assert_eq!(tag, "inspect_step");

        let _ = ticket.execute(); // Consume the continuation
        assert!(ticket.inspect(|c| c.peek_next_step_tag()).is_none()); // Should be None after execution
    }
}
