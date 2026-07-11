use crate::execution::phase::UncheckedActiveExecutor;
use crate::execution::strategy::{ContinueStrategy, ContinuousStrategyResult};
use crate::modeling::model::Model;

/// 指定されたマイクロステップ上限に達したらそのtick内で処理すべきイベントなどがまだ残っている場合は破棄して継続する継続戦略
pub struct LimitDiscardStrategy {
    limit_micro_step_count: u64,
}

impl<E, M: Model<E>, RunnerError> ContinueStrategy<E, M, RunnerError> for LimitDiscardStrategy {
    type Err = RunnerError;

    fn handle_micro_step_continue(
        &mut self,
        model: &M,
        unchecked: UncheckedActiveExecutor<E, M>,
    ) -> ContinuousStrategyResult<E, M, Self::Err> {
        let current_micro_step = unchecked.current_micro_step();

        // 上限未達
        if current_micro_step.value() < self.limit_micro_step_count {
            return Ok(unchecked.into_active_executor());
        }

        let mut next_active = unchecked.into_active_executor();
        // まだ継続するので現在のtickで残っている処理すべきイベントをすべて破棄
        next_active.discard_remain_micro_step(model);
        Ok(next_active)
    }
}

impl LimitDiscardStrategy {
    pub fn new(limit_micro_step_count: u64) -> Self {
        LimitDiscardStrategy {
            limit_micro_step_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{ActiveExecutorContext, EventContext};
    use crate::event_scheduler::EventScheduler;
    use crate::modeling::event::Event;
    use crate::modeling::hook::instance::HookDelegate;
    use crate::primitive::time::{MicroStep, MicroStepStatus, TickStatus};
    use crate::source_handler::SourceHandler;
    use std::convert::Infallible;

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    enum TestEvent {}

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

    // UncheckedActiveExecutor のヘルパー生成関数
    fn create_unchecked_executor(
        micro_step_val: u64,
    ) -> UncheckedActiveExecutor<TestEvent, TestModel> {
        let active_context = ActiveExecutorContext {
            current_tick_status: TickStatus::initialize(),
            next_micro_step_status: MicroStepStatus::new(MicroStep::new(micro_step_val + 1)),
            hook_delegate: HookDelegate::new(),
            source_handler: SourceHandler::new(),
            event_scheduler: EventScheduler::new(),
        };
        UncheckedActiveExecutor::new(active_context, MicroStep::new(micro_step_val))
    }

    #[test]
    fn test_limit_discard_strategy_under_limit() {
        let model = TestModel;
        // 上限を 5 に設定
        let mut strategy = LimitDiscardStrategy::new(5);

        // 現在のマイクロステップが 4 (上限 5 未満)
        let unchecked = create_unchecked_executor(4);
        let result: ContinuousStrategyResult<TestEvent, TestModel, Infallible> =
            strategy.handle_micro_step_continue(&model, unchecked);

        // 上限未満なので discard は走らずそのまま Ok
        assert!(result.is_ok());
    }

    #[test]
    fn test_limit_discard_strategy_reach_limit() {
        let model = TestModel;
        // 上限を 5 に設定
        let mut strategy = LimitDiscardStrategy::new(5);

        // 現在のマイクロステップが 5 (上限に到達)
        let unchecked = create_unchecked_executor(5);
        let result: ContinuousStrategyResult<TestEvent, TestModel, Infallible> =
            strategy.handle_micro_step_continue(&model, unchecked);

        // 上限に達しても Err にはならず、内部で discard された上で Ok となる
        assert!(result.is_ok());
    }
}
