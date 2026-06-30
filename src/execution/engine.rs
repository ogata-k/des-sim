use crate::context::{ExecutorContext, SourceContext};
use crate::event_scheduler::EventScheduler;
use crate::modeling::event::EventPriority;
use crate::modeling::hook::Hook;
use crate::modeling::hook::instance::{HookDelegate, SharedHook};
use crate::modeling::model::Model;
use crate::modeling::source::Source;
use crate::primitive::time::{Duration, SimTime};
use crate::primitive::time::{MicroStepStatus, TickStatus};
use crate::source_handler::SourceHandler;

pub struct Engine<E, M: Model<E>> {
    pub(crate) hook_delegate: HookDelegate<E, M>,
    pub(crate) source_handler: SourceHandler<E, M>,
    pub(crate) event_scheduler: EventScheduler<E>,
}

impl<E, M: Model<E>> Default for Engine<E, M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E, M: Model<E>> Engine<E, M> {
    pub fn new() -> Engine<E, M> {
        Engine {
            hook_delegate: HookDelegate::new(),
            source_handler: SourceHandler::new(),
            event_scheduler: EventScheduler::new(),
        }
    }

    pub fn begin_simulation(mut self, model: &M) -> ExecutorContext<E, M> {
        self.hook_delegate.before_simulation(model);

        // Engineで登録したソースを反映させておく
        self.source_handler.flush_pending();
        {
            // 一時的にSourceContextを作りSourceの初期化をした後にEngineを再構築して元に戻す
            let mut source_context = SourceContext {
                current_tick_status: TickStatus::initialize(),
                current_micro_step_status: MicroStepStatus::initialize(),
                hook_delegate: self.hook_delegate,
                // SourceContextはプロパティはあるが、EventPhaseを作るときのHandlerにするときしかSourceHandlerを持たない。
                // なのでここでもNoneを渡しておく
                source_handler: None,
                event_scheduler: self.event_scheduler,
            };
            self.source_handler.initialize_sources(|source| {
                source_context
                    .hook()
                    .before_initialize_source(model, source.name.as_ref());
                source.source.initialize(&mut source_context, model);
                source_context
                    .hook()
                    .after_initialize_source(model, source.name.as_ref());
            });
            self = Engine {
                hook_delegate: source_context.hook_delegate,
                source_handler: self.source_handler,
                event_scheduler: source_context.event_scheduler,
            };
        }

        // Engineや上のSourceの初期化で登録したイベントを反映させておく
        self.event_scheduler.flush_pending();

        ExecutorContext {
            next_tick_status: TickStatus::initialize(),
            current_tick: SimTime::zero(),
            hook_delegate: self.hook_delegate,
            source_handler: self.source_handler,
            event_scheduler: self.event_scheduler,
        }
    }

    pub fn add_hook<H>(&mut self, hook: H) -> &mut Self
    where
        H: Hook<E, M> + 'static,
    {
        self.hook_delegate.add_hook(hook);
        self
    }

    pub fn add_shared_hook<H>(&mut self, shared_hook: SharedHook<E, M, H>) -> &mut Self
    where
        E: 'static,
        M: 'static,
        H: Hook<E, M> + 'static,
    {
        self.hook_delegate.add_shared_hook(shared_hook);
        self
    }

    pub fn add_source_at<S>(
        &mut self,
        name: &'static str,
        first_fire_time: SimTime,
        source: S,
    ) -> &mut Self
    where
        S: Source<E, M> + 'static,
    {
        self.source_handler
            .add_source(name, first_fire_time, source);
        self
    }

    pub fn schedule_event_at(
        &mut self,
        sim_time: SimTime,
        priority: EventPriority,
        event_payload: E,
    ) -> &mut Self {
        self.event_scheduler
            .schedule(sim_time, Duration::zero(), priority, event_payload);

        self
    }
}
