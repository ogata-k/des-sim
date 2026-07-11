use crate::execution::phase::UncheckedActiveExecutor;
use crate::execution::strategy::{ContinueStrategy, ContinuousStrategyResult};
use crate::modeling::model::Model;
use std::marker::PhantomData;

/// [Runner](crate::execution::runner::Runner)のデフォルトの挙動のままの継続戦略
#[derive(Clone)]
pub struct AlwaysContinueStrategy<E, M>(PhantomData<(E, M)>);

impl<E, M> Default for AlwaysContinueStrategy<E, M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E, M: Model<E>, RunnerError> ContinueStrategy<E, M, RunnerError>
    for AlwaysContinueStrategy<E, M>
{
    type Err = RunnerError;

    fn handle_micro_step_continue(
        &mut self,
        _model: &M,
        unchecked: UncheckedActiveExecutor<E, M>,
    ) -> ContinuousStrategyResult<E, M, Self::Err> {
        Ok(unchecked.into_active_executor())
    }
}

impl<E, M> AlwaysContinueStrategy<E, M> {
    pub fn new() -> Self {
        Self(PhantomData)
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

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    enum TestEvent {}

    #[derive(Debug, Clone)]
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

    #[test]
    fn test_always_continue_strategy_default_and_new() {
        // new と default でインスタンス化できることを確認
        let _strategy_new = AlwaysContinueStrategy::<TestEvent, TestModel>::new();
        let _strategy_default = AlwaysContinueStrategy::<TestEvent, TestModel>::default();
    }

    #[test]
    fn test_always_continue_strategy_handle_continue() {
        let model = TestModel;

        // UncheckedActiveExecutor を構築するための ActiveExecutorContext の準備
        let active_context = ActiveExecutorContext {
            current_tick_status: TickStatus::initialize(),
            next_micro_step_status: MicroStepStatus::initialize(),
            hook_delegate: HookDelegate::new(),
            source_handler: SourceHandler::new(),
            event_scheduler: EventScheduler::new(),
        };

        // テスト用の UncheckedActiveExecutor を生成
        let current_micro_step = MicroStep::zero();
        let unchecked_executor = UncheckedActiveExecutor::new(active_context, current_micro_step);

        let mut strategy = AlwaysContinueStrategy::<TestEvent, TestModel>::new();

        // ダミーのエラー型として()を指定して戦略を実行
        let result: ContinuousStrategyResult<TestEvent, TestModel, ()> =
            strategy.handle_micro_step_continue(&model, unchecked_executor);

        // AlwaysContinueStrategy は常に Ok(ActiveExecutorContext) を返すことを検証
        assert!(result.is_ok());

        let returned_context = result.map_err(|e| e.1).unwrap();
        // 内部のコンテキストが維持されていることを確認
        assert_eq!(
            returned_context.current_tick_status.current(),
            crate::primitive::time::SimTime::zero()
        );
    }
}
