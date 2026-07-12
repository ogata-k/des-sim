use crate::context::{SourceContext, UserContext};
use crate::execution::phase::MicroStepHandler;
use crate::modeling::hook::Hook;
use crate::modeling::model::Model;
use crate::source_handler::SourceHandler;
use crate::source_handler::{SourceReadyEntry, SourceView};
use std::collections::VecDeque;

pub struct SourcePhase<E, M: Model<E>> {
    context: SourceContext<E, M>,
    // SourceContextはSourceを詰めなおすときに発火させてから詰めなおす都合上、SourceContextを持っているとライフタイムの問題が発生する。
    // そのため、MicroStepHandlerに渡す時だけSourceContextをSourcePhaseから奪い取る形で実装されている。
    pub(crate) source_handler: Option<SourceHandler<E, M>>,
    ready_sources: VecDeque<SourceReadyEntry>,
}

impl<E, M: Model<E>> SourcePhase<E, M> {
    pub(crate) fn new(
        context: SourceContext<E, M>,
        source_handler: SourceHandler<E, M>,
        ready_sources: VecDeque<SourceReadyEntry>,
    ) -> Self {
        SourcePhase {
            context,
            source_handler: Some(source_handler),
            ready_sources,
        }
    }

    pub fn get_context(&mut self) -> &mut SourceContext<E, M> {
        &mut self.context
    }

    pub fn get_source_view(&self, ready_entry: &SourceReadyEntry) -> SourceView {
        SourceView::new(ready_entry.source_id(), ready_entry.clone_name_arc())
    }

    pub fn complete_source_phase(self, model: &M) -> MicroStepHandler<SourceContext<E, M>> {
        self.context.hook().after_source_phase(
            model,
            self.context.current_tick(),
            self.context.current_micro_step(),
        );

        // SourcePhaseが持っているsource_handlerをMicroStepHandler内で次のフェーズに行くためにSourceContextに渡す。
        let mut context = self.context;
        context.source_handler = self.source_handler;
        MicroStepHandler::new(context)
    }

    pub fn take_one(&mut self) -> Option<SourceReadyEntry> {
        self.ready_sources.pop_front()
    }

    /// 全体から条件を満たす一件を取得して取り出す。
    pub fn take_one_if<F>(&mut self, predicate: F) -> Option<SourceReadyEntry>
    where
        F: FnOnce(&SourceReadyEntry) -> bool,
    {
        self.ready_sources.pop_front_if(|e| predicate(e))
    }

    /// 先頭が条件を満たす時だけ取り出す。
    pub fn take_front_if<F>(&mut self, predicate: F) -> Option<SourceReadyEntry>
    where
        F: FnOnce(&SourceReadyEntry) -> bool,
    {
        // 先頭要素を覗いて、条件に合致するか判定
        if self.ready_sources.front().is_some_and(predicate) {
            self.ready_sources.pop_front()
        } else {
            None
        }
    }

    pub fn take_all(&mut self) -> VecDeque<SourceReadyEntry> {
        std::mem::take(&mut self.ready_sources)
    }

    pub fn take_all_if<F>(&mut self, predicate: F) -> VecDeque<SourceReadyEntry>
    where
        F: FnMut(&SourceReadyEntry) -> bool,
    {
        // 一時的にすべて取得して抽出して差し替える
        let all_sources = std::mem::take(&mut self.ready_sources);

        let (taken, remaining): (VecDeque<_>, VecDeque<_>) =
            all_sources.into_iter().partition(predicate);

        self.ready_sources = remaining;

        taken
    }

    pub fn fire_and_schedule(&mut self, model: &M, entry: SourceReadyEntry) {
        let now = self.context.current_tick();
        let current_microstep = self.context.current_micro_step();
        let view = self.get_source_view(&entry);

        self.context
            .hook()
            .before_source(model, now, current_microstep, &view);

        let source_handler = self
            .source_handler
            .as_mut()
            .expect("Fail impl keep source_handler in SourcePhase.");

        let source_id = entry.source_id();
        let entry = source_handler.get_by_source_id(source_id);
        let next_fire_delay_optional = entry.source.fire(&mut self.context, model);
        if let Some(next_fire_delay) = next_fire_delay_optional {
            source_handler.schedule_next(now, next_fire_delay, source_id);
        }

        let computed_next_scheduled_at = next_fire_delay_optional.map(|d| now + d);

        self.context.hook().after_source(
            model,
            now,
            current_microstep,
            &view,
            computed_next_scheduled_at,
        );
    }

    pub fn discard(&mut self, model: &M, entry: SourceReadyEntry) {
        let view = self.get_source_view(&entry);

        self.context.hook().discard_source(
            model,
            self.context.current_tick(),
            self.context.current_micro_step(),
            &view,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{EventContext, UserContext};
    use crate::event_scheduler::EventScheduler;
    use crate::modeling::event::Event;
    use crate::modeling::hook::Hook;
    use crate::modeling::hook::instance::{HookDelegate, SharedHook};
    use crate::modeling::model::Model;
    use crate::primitive::id::SourceId;
    use crate::primitive::time::{Duration, MicroStep, MicroStepStatus, SimTime, TickStatus};
    use std::collections::VecDeque;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    enum TestEvent {}

    struct TestModel {
        handled_events: Vec<TestEvent>,
    }

    impl Model<TestEvent> for TestModel {
        fn handle_event(
            &mut self,
            _context: &mut EventContext<TestEvent, Self>,
            event: &Event<TestEvent>,
        ) {
            self.handled_events.push(event.payload);
        }
    }

    struct DiscardHook {
        discarded_events: Rc<Mutex<Vec<TestEvent>>>,
        discarded_sources: Rc<Mutex<Vec<SourceId>>>,
    }

    impl Hook<TestEvent, TestModel> for DiscardHook {
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
            // none
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
            // none
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
            source_view: &SourceView,
        ) {
            self.discarded_sources
                .lock()
                .unwrap()
                .push(source_view.source_id());
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
            // none
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
            event: &Event<TestEvent>,
        ) {
            self.discarded_events.lock().unwrap().push(event.payload);
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

    fn setup_source_phase() -> (SourcePhase<TestEvent, TestModel>, TestModel) {
        let model = TestModel {
            handled_events: Vec::new(),
        };

        // SourceContext 生存中は source_handler は常に None
        let source_context = SourceContext {
            current_tick_status: TickStatus::initialize(),
            current_micro_step_status: MicroStepStatus::initialize(),
            hook_delegate: HookDelegate::new(),
            source_handler: None,
            event_scheduler: EventScheduler::new(),
        };

        // クローン不可のハンドラ実体を1つだけ生成
        let source_handler = SourceHandler::new();

        let mut ready_sources = VecDeque::new();
        ready_sources.push_back(SourceReadyEntry::new(
            SourceId::new(1),
            Arc::from("SourceA"),
        ));
        ready_sources.push_back(SourceReadyEntry::new(
            SourceId::new(2),
            Arc::from("SourceB"),
        ));
        ready_sources.push_back(SourceReadyEntry::new(
            SourceId::new(3),
            Arc::from("SourceC"),
        ));

        let source_phase = SourcePhase::new(source_context, source_handler, ready_sources);

        (source_phase, model)
    }

    #[test]
    fn test_new() {
        let (source_phase, _model) = setup_source_phase();
        assert_eq!(source_phase.ready_sources.len(), 3);
        assert!(source_phase.source_handler.is_some());
    }

    #[test]
    fn test_get_context() {
        let (mut source_phase, _model) = setup_source_phase();
        let context = source_phase.get_context();
        assert_eq!(context.current_tick(), SimTime::zero());
    }

    #[test]
    fn test_get_source_view() {
        let (source_phase, _model) = setup_source_phase();
        let entry = SourceReadyEntry::new(SourceId::new(1), Arc::from("SourceA"));
        let view = source_phase.get_source_view(&entry);
        assert_eq!(view.source_id(), SourceId::new(1));
        assert_eq!(view.name(), "SourceA");
    }

    #[test]
    fn test_take_one() {
        let (mut source_phase, _model) = setup_source_phase();
        let entry = source_phase.take_one().unwrap();
        assert_eq!(entry.source_id(), SourceId::new(1));
        assert_eq!(source_phase.ready_sources.len(), 2);
    }

    #[test]
    fn test_take_front_if() {
        let (mut source_phase, _model) = setup_source_phase();
        let entry_a = source_phase
            .take_front_if(|e| e.source_id() == SourceId::new(1))
            .unwrap();
        assert_eq!(entry_a.source_id(), SourceId::new(1));
        assert_eq!(source_phase.ready_sources.len(), 2);

        let entry_c = source_phase.take_front_if(|e| e.source_id() == SourceId::new(3));
        assert!(entry_c.is_none());
        assert_eq!(source_phase.ready_sources.len(), 2);
    }

    #[test]
    fn test_take_all() {
        let (mut source_phase, _model) = setup_source_phase();
        let all_entries = source_phase.take_all();
        assert_eq!(all_entries.len(), 3);
        assert_eq!(source_phase.ready_sources.len(), 0);
    }

    #[test]
    fn test_take_all_if() {
        let (mut source_phase, _model) = setup_source_phase();
        source_phase.ready_sources.push_back(SourceReadyEntry::new(
            SourceId::new(1),
            Arc::from("SourceA_again"),
        ));

        let taken_entries = source_phase.take_all_if(|e| e.source_id() == SourceId::new(1));
        assert_eq!(taken_entries.len(), 2);
        assert_eq!(taken_entries.front().unwrap().source_id(), SourceId::new(1));
        assert_eq!(taken_entries.get(1).unwrap().source_id(), SourceId::new(1));

        assert_eq!(source_phase.ready_sources.len(), 2);
        assert_eq!(
            source_phase.ready_sources.front().unwrap().source_id(),
            SourceId::new(2)
        );
        assert_eq!(
            source_phase.ready_sources.get(1).unwrap().source_id(),
            SourceId::new(3)
        );
    }

    #[test]
    fn test_fire_and_schedule() {
        let (source_phase, _model) = setup_source_phase();
        assert!(source_phase.source_handler.is_some());
    }

    #[test]
    fn test_discard() {
        let (mut source_phase, model) = setup_source_phase();

        // EventPhaseのお手本通り、SharedHook を作成して動的に add_shared_hook する
        let hook = SharedHook::new(DiscardHook {
            discarded_events: Rc::new(Mutex::new(Vec::new())),
            discarded_sources: Rc::new(Mutex::new(Vec::new())),
        });

        source_phase
            .get_context()
            .hook_delegate
            .add_shared_hook(hook.clone());

        let source_a_entry = source_phase.take_one().unwrap();
        source_phase.discard(&model, source_a_entry);

        // hook.get_ref() から内部状態を美しく検証
        assert_eq!(hook.get_ref().discarded_sources.lock().unwrap().len(), 1);
        assert_eq!(
            hook.get_ref().discarded_sources.lock().unwrap()[0],
            SourceId::new(1)
        );
    }

    #[test]
    fn test_complete_source_phase() {
        let (source_phase, model) = setup_source_phase();
        let micro_step_handler = source_phase.complete_source_phase(&model);

        assert_eq!(
            micro_step_handler.ref_context().current_tick(),
            SimTime::zero()
        );
        assert!(micro_step_handler.ref_context().source_handler.is_some());
    }
}
