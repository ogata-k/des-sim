use crate::execution::engine::SourceContext;
use crate::execution::engine::handler::MicroStepHandler;
use crate::world::context::UserContext;
use crate::world::hook::Hook;
use crate::world::model::Model;
use crate::world::source::{SourceHandler, SourceReadyEntry, SourceView};
use std::sync::Arc;

pub struct SourcePhase<E, M: Model<E>> {
    context: SourceContext<E, M>,
    // SourceContextはSourceを詰めなおすときに発火させてから詰めなおす都合上、SourceContextを持っているとライフタイムの問題が発生する。
    // そのため、MicroStepHandlerに渡す時だけSourceContextをSourcePhaseから奪い取る形で実装されている。
    pub(crate) source_handler: Option<SourceHandler<E, M, SourceContext<E, M>>>,
    ready_sources: Vec<SourceReadyEntry>,
}

impl<E, M: Model<E>> SourcePhase<E, M> {
    pub(crate) fn new(
        context: SourceContext<E, M>,
        source_handler: SourceHandler<E, M, SourceContext<E, M>>,
        ready_sources: Vec<SourceReadyEntry>,
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

    pub fn finish_source_phase(self) -> MicroStepHandler<SourceContext<E, M>> {
        // SourcePhaseが持っているsource_handlerをMicroStepHandler内で次のフェーズに行くためにSourceContextに渡す。
        let mut context = self.context;
        context.source_handler = self.source_handler;
        MicroStepHandler::new(context)
    }

    pub fn take_one(&mut self) -> Option<SourceReadyEntry> {
        if self.ready_sources.is_empty() {
            None
        } else {
            Some(self.ready_sources.remove(0))
        }
    }

    pub fn fire_and_schedule(&mut self, model: &M, entry: SourceReadyEntry) {
        let now = self.context.current_tick();
        let current_microstep = self.context.current_micro_step();
        let view = self.get_source_view(&entry);

        self.context
            .hook()
            .before_source(now, current_microstep, &view);

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

        self.context
            .hook()
            .after_source(now, current_microstep, &view, computed_next_scheduled_at);
    }

    // @todo ソースをまとめて処理できるやつ

    pub fn discard(&mut self, entry: SourceReadyEntry) {
        let view = self.get_source_view(&entry);

        self.context.hook().discard_source(
            self.context.current_tick(),
            self.context.current_micro_step(),
            &view,
        );
    }
}
