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
