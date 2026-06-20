use crate::execution::engine::SourceContext;
use crate::execution::engine::handler::MicroStepHandler;
use crate::execution::scheduler::EventScheduler;
use crate::execution::utility::{
    MicroStepStatus, SimulationError, SimulationOutput, SimulationResult, TickStatus,
};
use crate::primitive::time::{Duration, SimTime};
use crate::world::hook::{Hook, HookDelegate};
use crate::world::model::Model;
use crate::world::source::SourceHandler;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum ExecutorStatus {
    NoMoreEvent,
    ExistsMoreEvent,
}

// Runner開発者向けのContextなのでUserContextは実装していない
pub struct ExecutorContext<E, M: Model<E>> {
    pub(crate) tick_status: TickStatus,
    pub(crate) current_tick: SimTime,
    pub(crate) hook_delegate: HookDelegate<E>,
    pub(crate) source_handler: SourceHandler<E, M, SourceContext<E, M>>,
    pub(crate) event_scheduler: EventScheduler<E>,
}

impl<E, M: Model<E>> ExecutorContext<E, M> {
    pub fn hook(&self) -> &impl Hook<E> {
        &self.hook_delegate
    }

    pub fn peek_next_tick(&self) -> (ExecutorStatus, TickStatus) {
        let next_event_fired_at = self.event_scheduler.peek_next_time();
        let executor_status = if next_event_fired_at.is_some() {
            ExecutorStatus::ExistsMoreEvent
        } else {
            ExecutorStatus::NoMoreEvent
        };

        (executor_status, self.tick_status)
    }

    pub fn begin_tick(self) -> ActiveExecutorContext<E, M> {
        self.hook_delegate
            .before_tick(self.tick_status.current(), self.tick_status.skipped());

        ActiveExecutorContext {
            // ここからは現在のTickStatus
            current_tick_status: self.tick_status,
            micro_step_status: MicroStepStatus::initialize(),
            hook_delegate: self.hook_delegate,
            source_handler: self.source_handler,
            event_scheduler: self.event_scheduler,
        }
    }

    pub fn end_simulation_as_ok<Err>(self, model: M) -> SimulationResult<M, Err> {
        self.hook().after_simulation(self.current_tick);
        Ok(SimulationOutput::new(self.current_tick, model))
    }

    pub fn end_simulation_as_error<Err>(self, model: M, error: Err) -> SimulationResult<M, Err> {
        self.hook().after_simulation(self.current_tick);
        Err(SimulationError::new(self.current_tick, model, error))
    }
}

pub struct ActiveExecutorContext<E, M: Model<E>> {
    pub(crate) current_tick_status: TickStatus,
    // 現在時刻が確定しているタイミングなのでTickStatusは現在の状態を表すものだが、
    // まだMicroStepは始まっていないのでMicroStepStatusは未来の状態。
    pub(crate) micro_step_status: MicroStepStatus,
    pub(crate) hook_delegate: HookDelegate<E>,
    pub(crate) source_handler: SourceHandler<E, M, SourceContext<E, M>>,
    pub(crate) event_scheduler: EventScheduler<E>,
}

impl<E, M: Model<E>> ActiveExecutorContext<E, M> {
    pub fn hook(&self) -> &impl Hook<E> {
        &self.hook_delegate
    }

    pub fn begin_micro_step(self) -> MicroStepHandler<ActiveExecutorContext<E, M>> {
        self.hook().before_micro_step(
            self.current_tick_status.current(),
            self.micro_step_status.current(),
        );

        MicroStepHandler::new(self)
    }

    pub fn end_tick_with_increment_tick(self) -> ExecutorContext<E, M> {
        let current_tick = self.current_tick_status.current();
        self.hook()
            .after_tick(current_tick, self.micro_step_status.current());
        let next_tick = current_tick + Duration::one();
        let next_tick_status = TickStatus::new(next_tick, Duration::zero());

        ExecutorContext {
            tick_status: next_tick_status,
            current_tick,
            hook_delegate: self.hook_delegate,
            source_handler: self.source_handler,
            event_scheduler: self.event_scheduler,
        }
    }

    pub fn end_tick_with_jump_to_next_tick(self) -> ExecutorContext<E, M> {
        let current_tick = self.current_tick_status.current();
        self.hook()
            .after_tick(current_tick, self.micro_step_status.current());
        let (skipped, next_tick) = match (
            self.source_handler.peek_next_time(),
            self.event_scheduler.peek_next_time(),
        ) {
            (Some(next_scheduled_at), _) | (_, Some(next_scheduled_at)) => (
                next_scheduled_at - current_tick - Duration::one(),
                next_scheduled_at,
            ),
            (_, _) => {
                // 次に発火させるべきものがないので次へ順番に進めておく
                (Duration::zero(), current_tick + Duration::one())
            }
        };
        let next_tick_status = TickStatus::new(next_tick, skipped);

        ExecutorContext {
            tick_status: next_tick_status,
            current_tick,
            hook_delegate: self.hook_delegate,
            source_handler: self.source_handler,
            event_scheduler: self.event_scheduler,
        }
    }

    pub fn discard_remain_micro_step(&mut self) {
        let current_tick = self.current_tick_status.current();
        let ready_sources = self.source_handler.drain_ready(current_tick);
        let ready_events = self.event_scheduler.drain_ready(current_tick);

        self.hook().on_discard_remain_micro_step(
            current_tick,
            self.micro_step_status.current(),
            &ready_sources,
            &ready_events,
        );
    }
}
