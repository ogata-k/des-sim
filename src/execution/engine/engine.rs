use crate::execution::scheduler::EventScheduler;
use crate::primitive::time::{MicroStep, SimTime};
use crate::world::hook::{Hook, HookDelegate};
use crate::world::source::{SourceContext, SourceHandler, SourceView};
use std::sync::Arc;

// @todo 後でModelを注入する予定
// @todo 後でソースを追加したりをEngineContext経由でできるようにするためにEngineにいろいろメソッドを用意するなどする
pub struct Engine<E> {
    time: SimTime,
    hook_delegate: HookDelegate<E>,
    source_handler: SourceHandler<E>,
    event_scheduler: EventScheduler<E>,
}

impl<E> Engine<E> {
    pub fn new() -> Engine<E> {
        Engine {
            time: SimTime::zero(),
            hook_delegate: HookDelegate::new(),
            source_handler: SourceHandler::new(),
            event_scheduler: EventScheduler::new(),
        }
    }

    pub(crate) fn run_ready_sources(&mut self, now: SimTime, micro_step: MicroStep) {
        let mut ready_sources = self.source_handler.drain_ready(now);
        while let Some(ready) = ready_sources.take_next() {
            // 今はSourceViewにはfire_and_schedule前後で変化するようなものは持っていないので同じものを使う。
            let source_view = SourceView::new(ready.0, Arc::clone(&ready.1));

            self.hook_delegate
                .before_source(now, micro_step, &source_view);
            let mut source_context = SourceContext::new(now, &mut self.event_scheduler);

            let next_scheduled_delay =
                self.source_handler
                    .fire_and_schedule(now, &mut source_context, ready.0);
            let next_scheduled_at = next_scheduled_delay.map(|d| now + d);

            self.hook_delegate
                .after_source(now, micro_step, &source_view, next_scheduled_at);
        }
    }
}
