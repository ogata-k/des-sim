use crate::execution::scheduler::EventScheduler;
use crate::execution::utility::{MicroStepStatus, TickStatus};
use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::world::context::{SourceContext as SourceContextTrait, UserContext};
use crate::world::event::Priority;
use crate::world::hook::{Hook, HookDelegate};
use crate::world::model::Model;
use crate::world::source::SourceHandler;

pub struct SourceContext<E, M: Model<E>> {
    pub(crate) current_tick_status: TickStatus,
    pub(crate) current_micro_step_status: MicroStepStatus,
    pub(crate) hook_delegate: HookDelegate<E>,
    // SourceContextはSourceを詰めなおすときに発火させてから詰めなおす都合上、SourceContextを持っているとライフタイムの問題が発生する。
    // そのため、MicroStepHandlerに渡す時だけSourceContextをSourcePhaseから奪い取る形で実装されている。
    pub(crate) source_handler: Option<SourceHandler<E, M, Self>>,
    pub(crate) event_scheduler: EventScheduler<E>,
}

impl<E, M: Model<E>> UserContext<E> for SourceContext<E, M> {
    fn current_tick(&self) -> SimTime {
        self.current_tick_status.current()
    }

    fn current_micro_step(&self) -> MicroStep {
        self.current_micro_step_status.current()
    }

    fn hook(&self) -> &impl Hook<E> {
        &self.hook_delegate
    }
}

impl<E, M: Model<E>> SourceContextTrait<E> for SourceContext<E, M> {
    fn schedule_event(&mut self, delay: Duration, priority: Priority, event_payload: E) {
        self.event_scheduler
            .schedule(self.current_tick(), delay, priority, event_payload);
    }
}
