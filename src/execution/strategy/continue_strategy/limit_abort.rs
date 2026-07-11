use crate::execution::phase::UncheckedActiveExecutor;
use crate::execution::strategy::{ContinueStrategy, ContinuousStrategyResult};
use crate::modeling::model::Model;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum LimitAbortStrategyError<RunnerError> {
    Runner(RunnerError),
    LimitExceeded {
        limit_micro_step_count: u64,
        limit_micro_step_exceeded_count: usize,
    },
}

impl<RunnerError: Display> Display for LimitAbortStrategyError<RunnerError> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LimitAbortStrategyError::Runner(e) => {
                write!(f, "{}", e)
            }
            LimitAbortStrategyError::LimitExceeded {
                limit_micro_step_count,
                limit_micro_step_exceeded_count: limit_micro_step_exhausted_count,
            } => {
                write!(
                    f,
                    "The maximum number of {} microsteps has been reached {} times.",
                    limit_micro_step_count, limit_micro_step_exhausted_count
                )
            }
        }
    }
}

impl<RunnerError: std::error::Error> std::error::Error for LimitAbortStrategyError<RunnerError> {}

/// 指定された上限突破回数を超えてマイクロステップ上限に達したらエラーとする継続戦略
/// マイクロステップ上限に達してもallow_error_countを超えるまでは、エラーにならずそのtick内で処理すべきイベントなどがまだ残っている場合は破棄して継続する。
/// 超えた場合は、エラーとして中断する。
pub struct LimitAbortStrategy {
    limit_micro_step_count: u64,
    allow_error_count: usize,
    error_count: usize,
}

impl<E, M: Model<E>, RunnerError> ContinueStrategy<E, M, RunnerError> for LimitAbortStrategy {
    type Err = LimitAbortStrategyError<RunnerError>;

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

        self.error_count += 1;
        if self.error_count <= self.allow_error_count {
            // 上限達成がまだ許容範囲
            let mut next_active = unchecked.into_active_executor();
            // まだ継続するので現在のtickで残っている処理すべきイベントをすべて破棄
            next_active.discard_remain_micro_step(model);
            Ok(next_active)
        } else {
            let mut next_active = unchecked.into_active_executor();
            // もう継続しないが、現在のtickをきれいに終わらせるためにも残っている処理する予定だったイベントをすべて破棄
            next_active.discard_remain_micro_step(model);
            Err((
                next_active,
                LimitAbortStrategyError::LimitExceeded {
                    limit_micro_step_count: self.limit_micro_step_count,
                    limit_micro_step_exceeded_count: self.error_count,
                },
            ))
        }
    }
}

impl LimitAbortStrategy {
    pub fn new(limit_micro_step_count: u64, allow_error_count: usize) -> Self {
        LimitAbortStrategy {
            limit_micro_step_count,
            allow_error_count,
            error_count: 0,
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

    // 各テストで使い回す UncheckedActiveExecutor のヘルパー生成関数
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
    fn test_limit_abort_strategy_under_limit() {
        let model = TestModel;
        // 上限を 10、許容回数を 1 に設定
        let mut strategy = LimitAbortStrategy::new(10, 1);

        // 現在のマイクロステップが 9 (上限 10 未満)
        let unchecked = create_unchecked_executor(9);
        let result: ContinuousStrategyResult<
            TestEvent,
            TestModel,
            LimitAbortStrategyError<String>,
        > = strategy.handle_micro_step_continue(&model, unchecked);

        // 上限未満なら無条件で Ok
        assert!(result.is_ok());
        assert_eq!(strategy.error_count, 0);
    }

    #[test]
    fn test_limit_abort_strategy_reach_limit_within_allowance() {
        let model = TestModel;
        // 上限を 10、許容回数を 1 に設定
        let mut strategy = LimitAbortStrategy::new(10, 1);

        // 1回目の上限到達 (現在のマイクロステップが 10)
        let unchecked = create_unchecked_executor(10);
        let result: ContinuousStrategyResult<
            TestEvent,
            TestModel,
            LimitAbortStrategyError<String>,
        > = strategy.handle_micro_step_continue(&model, unchecked);

        // 許容回数 (1) 以内なので Ok で継続する
        assert!(result.is_ok());
        assert_eq!(strategy.error_count, 1);
    }

    #[test]
    fn test_limit_abort_strategy_exceed_allowance_aborts() {
        let model = TestModel;
        // 上限を 10、許容回数を 1 に設定
        let mut strategy = LimitAbortStrategy::new(10, 1);

        // 1回目の上限到達（許容範囲内）
        let unchecked_1 = create_unchecked_executor(10);
        let _: ContinuousStrategyResult<TestEvent, TestModel, LimitAbortStrategyError<String>> =
            strategy.handle_micro_step_continue(&model, unchecked_1);

        // 2回目の上限到達（許容範囲超過）
        let unchecked_2 = create_unchecked_executor(10);
        let result: ContinuousStrategyResult<
            TestEvent,
            TestModel,
            LimitAbortStrategyError<String>,
        > = strategy.handle_micro_step_continue(&model, unchecked_2);

        // 許容回数を超えたため Err になる
        assert!(result.is_err());
        assert_eq!(strategy.error_count, 2);

        // エラー内容の検証
        let (_context, error) = result.err().unwrap();
        match error {
            LimitAbortStrategyError::LimitExceeded {
                limit_micro_step_count,
                limit_micro_step_exceeded_count,
            } => {
                assert_eq!(limit_micro_step_count, 10);
                assert_eq!(limit_micro_step_exceeded_count, 2);
            }
            _ => panic!("Expected LimitAbortStrategyError::LimitExceeded"),
        }
    }

    #[test]
    fn test_error_display() {
        // エラーの Display 実装の挙動確認
        let error: LimitAbortStrategyError<String> = LimitAbortStrategyError::LimitExceeded {
            limit_micro_step_count: 5,
            limit_micro_step_exceeded_count: 3,
        };
        let message = format!("{}", error);
        assert_eq!(
            message,
            "The maximum number of 5 microsteps has been reached 3 times."
        );

        let runner_error = LimitAbortStrategyError::Runner("internal error".to_string());
        assert_eq!(format!("{}", runner_error), "internal error");
    }
}
