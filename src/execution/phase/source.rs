use crate::context::{SourceContext, UserContext};
use crate::execution::phase::MicroStepHandler;
use crate::modeling::hook::Hook;
use crate::modeling::model::Model;
use crate::source_handler::SourceHandler;
use crate::source_handler::{SourceReadyEntry, SourceView};
use std::collections::VecDeque;
use std::sync::Arc;

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
        SourceView::new(ready_entry.source_id(), Arc::clone(&ready_entry.name_arc()))
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
        if self.ready_sources.front().map_or(false, predicate) {
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
