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
