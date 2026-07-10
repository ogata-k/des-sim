use crate::context::{ActiveExecutorContext, EventContext, SourceContext};
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
    pub fn start_source_phase(self, model: &M) -> SourcePhase<E, M> {
        let mut context = self.context;
        context.hook().before_source_phase(
            model,
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
    pub fn to_event_phase(self, model: &M) -> EventPhase<E, M> {
        let mut context = self.context;
        context.hook().before_event_phase(
            model,
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
    pub fn end_micro_step(mut self, model: &M) -> MicroStepResult<E, M> {
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
                    model,
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
                    model,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{ActiveExecutorContext, EventContext, SourceContext};
    use crate::event_scheduler::EventScheduler;
    use crate::modeling::event::{Event, EventPriority};
    use crate::modeling::hook::instance::{HookDelegate, SharedHook};
    use crate::modeling::model::Model;
    use crate::primitive::time::{Duration, MicroStep, MicroStepStatus, SimTime, TickStatus};
    use crate::source_handler::{SourceHandler, SourceReadyEntry, SourceView};
    use std::rc::Rc;
    use std::sync::Mutex;

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    enum TestEvent {
        A,
    }

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

    // 遷移時の各フェーズフック呼び出しを追跡するためのテスト用フック
    struct TransitionTrackerHook {
        called_before_source_phase: Rc<Mutex<bool>>,
        called_before_event_phase: Rc<Mutex<bool>>,
        called_after_micro_step: Rc<Mutex<bool>>,
    }

    impl Hook<TestEvent, TestModel> for TransitionTrackerHook {
        fn before_simulation(&self, _model: &TestModel) {
            // none
        }
        fn after_simulation(&self, _model: &TestModel, _end_tick: SimTime) {
            // none
        }
        fn before_tick(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _skipped_duration: Duration,
        ) {
            // none
        }
        fn after_tick(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _last_micro_step: MicroStep,
        ) {
            // none
        }
        fn before_micro_step(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            // none
        }
        fn after_micro_step(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            *self.called_after_micro_step.lock().unwrap() = true;
        }
        fn on_discard_remain_micro_step(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _first_discarded_micro_step: MicroStep,
            _discarded_sources: &[SourceReadyEntry],
            _discarded_events: &[Event<TestEvent>],
        ) {
            // none
        }
        fn before_register_source(&self, _model: &TestModel, _name: &str) {
            // none
        }
        fn after_register_source(&self, _model: &TestModel, _name: &str) {
            // none
        }
        fn before_source_phase(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            *self.called_before_source_phase.lock().unwrap() = true;
        }
        fn before_source(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _source_view: &SourceView,
        ) {
            // none
        }
        fn after_source(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _source_view: &SourceView,
            _computed_next_fire: Option<SimTime>,
        ) {
            // none
        }
        fn cancel_source(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _scheduled_at: SimTime,
            _source_view: &SourceView,
        ) {
            // none
        }
        fn discard_source(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _source_view: &SourceView,
        ) {
            // none
        }
        fn after_source_phase(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            // none
        }
        fn before_event_phase(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            *self.called_before_event_phase.lock().unwrap() = true;
        }
        fn before_event(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _event: &Event<TestEvent>,
        ) {
            // none
        }
        fn after_event(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _event: &Event<TestEvent>,
        ) {
            // none
        }
        fn cancel_event(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _scheduled_at: SimTime,
            _event: &Event<TestEvent>,
        ) {
            // none
        }
        fn discard_event(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _event: &Event<TestEvent>,
        ) {
            // none
        }
        fn after_event_phase(
            &self,
            _model: &TestModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            // none
        }
    }

    // SharedHook の型パラメータに正しく E, M, H をマッピング
    fn setup_tracker_hook() -> SharedHook<TestEvent, TestModel, TransitionTrackerHook> {
        SharedHook::new(TransitionTrackerHook {
            called_before_source_phase: Rc::new(Mutex::new(false)),
            called_before_event_phase: Rc::new(Mutex::new(false)),
            called_after_micro_step: Rc::new(Mutex::new(false)),
        })
    }

    #[test]
    fn test_ref_context_and_mut() {
        let mut handler = MicroStepHandler::new(42);
        assert_eq!(*handler.ref_context(), 42);

        *handler.ref_mut_context() = 100;
        assert_eq!(*handler.ref_context(), 100);
    }

    #[test]
    fn test_start_source_phase() {
        let model = TestModel;
        let hook = setup_tracker_hook();
        let mut hook_delegate = HookDelegate::new();
        hook_delegate.add_shared_hook(hook.clone());

        let active_context = ActiveExecutorContext {
            current_tick_status: TickStatus::initialize(),
            next_micro_step_status: MicroStepStatus::initialize(),
            hook_delegate,
            source_handler: SourceHandler::new(),
            event_scheduler: EventScheduler::new(),
        };

        let handler = MicroStepHandler::new(active_context);
        let mut source_phase = handler.start_source_phase(&model);

        assert!(source_phase.source_handler.is_some());
        assert!(source_phase.get_context().source_handler.is_none());
        assert!(*hook.get_ref().called_before_source_phase.lock().unwrap());
    }

    #[test]
    fn test_to_event_phase() {
        let model = TestModel;
        let hook = setup_tracker_hook();
        let mut hook_delegate = HookDelegate::new();
        hook_delegate.add_shared_hook(hook.clone());

        let source_context = SourceContext {
            current_tick_status: TickStatus::initialize(),
            current_micro_step_status: MicroStepStatus::initialize(),
            hook_delegate,
            source_handler: Some(SourceHandler::new()),
            event_scheduler: EventScheduler::new(),
        };

        let handler = MicroStepHandler::new(source_context);
        let _event_phase = handler.to_event_phase(&model);

        assert!(*hook.get_ref().called_before_event_phase.lock().unwrap());
    }

    #[test]
    fn test_end_micro_step_continue() {
        let model = TestModel;
        let hook = setup_tracker_hook();
        let mut hook_delegate = HookDelegate::new();
        hook_delegate.add_shared_hook(hook.clone());

        // 現在時間を明確に作成（例: 時間ゼロ）
        let tick_status = TickStatus::initialize();
        let current_time = tick_status.current();

        let mut event_scheduler = EventScheduler::new();
        event_scheduler.schedule(
            current_time,
            Duration::zero(),
            EventPriority::minimum(),
            TestEvent::A,
        );
        event_scheduler.flush_pending();

        // 念のため、この時点で正しく現在時間と一致するイベントがスケジュールされているか検証
        assert_eq!(event_scheduler.peek_next_time(), Some(current_time));

        let event_context = EventContext {
            current_tick_status: tick_status,
            current_micro_step_status: MicroStepStatus::initialize(),
            hook_delegate,
            source_handler: SourceHandler::new(),
            event_scheduler,
        };

        let handler = MicroStepHandler::new(event_context);
        let result = handler.end_micro_step(&model);

        match result {
            MicroStepResult::Continue(_) => {
                // パス
            }
            MicroStepResult::Complete(_, _) => {
                panic!("Expected MicroStepResult::Continue, but got Complete");
            }
        }
        assert!(*hook.get_ref().called_after_micro_step.lock().unwrap());
    }

    #[test]
    fn test_end_micro_step_complete() {
        let model = TestModel;
        let hook = setup_tracker_hook();
        let mut hook_delegate = HookDelegate::new();
        hook_delegate.add_shared_hook(hook.clone());

        let event_context = EventContext {
            current_tick_status: TickStatus::initialize(),
            current_micro_step_status: MicroStepStatus::initialize(),
            hook_delegate,
            source_handler: SourceHandler::new(),
            event_scheduler: EventScheduler::new(),
        };

        let handler = MicroStepHandler::new(event_context);
        let result = handler.end_micro_step(&model);

        match result {
            MicroStepResult::Complete(_, _) => {}
            MicroStepResult::Continue(_) => {
                panic!("Expected MicroStepResult::Complete, but got Continue");
            }
        }
        assert!(*hook.get_ref().called_after_micro_step.lock().unwrap());
    }
}
