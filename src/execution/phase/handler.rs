use crate::context::{ActiveExecutorContext, EventContext, SourceContext, UserContext};
use crate::execution::phase::{EventPhase, MicroStepResult, SourcePhase, UncheckedActiveExecutor};
use crate::modeling::hook::Hook;
use crate::modeling::model::Model;
use crate::primitive::time::MicroStepStatus;

// 1回限りの使い捨て入場券構造体（Cloneは絶対に実装しない）
pub struct MicroStepHandler<CTX> {
    context: CTX,
}

impl<CTX> MicroStepHandler<CTX> {
    pub(crate) fn new(context: CTX) -> MicroStepHandler<CTX> {
        MicroStepHandler { context }
    }

    pub fn ref_context(&self) -> &CTX {
        &self.context
    }

    pub fn ref_mut_context(&mut self) -> &mut CTX {
        &mut self.context
    }
}

impl<E, M: Model<E>> MicroStepHandler<ActiveExecutorContext<E, M>> {
    pub fn start_source_phase(self) -> SourcePhase<E, M> {
        let mut context = self.context;
        context.hook().before_source_phase(
            context.current_tick_status.current(),
            context.next_micro_step_status.current(),
        );

        let ready_sources = context
            .source_handler
            .drain_ready(context.current_tick_status.current());
        SourcePhase::new(
            SourceContext {
                current_tick_status: context.current_tick_status,
                current_micro_step_status: context.next_micro_step_status,
                hook_delegate: context.hook_delegate,
                source_handler: None,
                event_scheduler: context.event_scheduler,
            },
            context.source_handler,
            ready_sources,
        )
    }
}

impl<E, M: Model<E>> MicroStepHandler<SourceContext<E, M>> {
    pub fn to_event_phase(self) -> EventPhase<E, M> {
        let mut context = self.context;
        context.hook().before_event_phase(
            context.current_tick_status.current(),
            context.current_micro_step_status.current(),
        );

        let ready_events = context
            .event_scheduler
            .drain_ready(context.current_tick_status.current());
        EventPhase::new(
            EventContext {
                current_tick_status: context.current_tick_status,
                current_micro_step_status: context.current_micro_step_status,
                hook_delegate: context.hook_delegate,
                source_handler: context
                    .source_handler
                    .expect("Fail impl take source_handler from SourcePhase."),
                event_scheduler: context.event_scheduler,
            },
            ready_events,
        )
    }
}

impl<E, M: Model<E>> MicroStepHandler<EventContext<E, M>> {
    pub fn end_micro_step(mut self) -> MicroStepResult<E, M> {
        // 次のイベント登録状況を更新するためにペンディングしているものを反映する。
        // これがないとここ以後の処理が事故る。
        self.ref_mut_context().source_handler.flush_pending();
        self.ref_mut_context().event_scheduler.flush_pending();

        let next_event_scheduled_at = self.ref_context().event_scheduler.peek_next_time();
        match next_event_scheduled_at {
            Some(next_scheduled_at)
                if self.ref_context().current_tick_status.current() == next_scheduled_at =>
            {
                // まだ同tick中に発火可能なイベントがあるので次のマイクロステップに進める
                let current_micro_step = self.ref_context().current_micro_step_status.current();
                let next_micro_step = current_micro_step.next();
                self.context.hook().after_micro_step(
                    self.ref_context().current_tick_status.current(),
                    self.ref_context().current_micro_step_status.current(),
                );

                let active_context = ActiveExecutorContext {
                    current_tick_status: self.ref_context().current_tick_status,
                    next_micro_step_status: MicroStepStatus::new(next_micro_step),
                    hook_delegate: self.context.hook_delegate,
                    source_handler: self.context.source_handler,
                    event_scheduler: self.context.event_scheduler,
                };
                MicroStepResult::Continue(UncheckedActiveExecutor::new(
                    active_context,
                    current_micro_step,
                ))
            }
            _ => {
                // 処理すべきイベントがある次のtickが未来の時間かそもそもないので、今のマイクロステップで終了
                let last_micro_step_status = self.ref_context().current_micro_step_status;
                self.context.hook().after_micro_step(
                    self.ref_context().current_tick_status.current(),
                    self.ref_context().current_micro_step_status.current(),
                );

                let active_context = ActiveExecutorContext {
                    current_tick_status: self.ref_context().current_tick_status,
                    next_micro_step_status: last_micro_step_status,
                    hook_delegate: self.context.hook_delegate,
                    source_handler: self.context.source_handler,
                    event_scheduler: self.context.event_scheduler,
                };
                MicroStepResult::Complete(active_context, last_micro_step_status)
            }
        }
    }
}
