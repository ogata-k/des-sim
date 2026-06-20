use crate::context::SourceContext;
use crate::modeling::model::Model;
use crate::primitive::time::Duration;

pub trait Source<E, M: Model<E>>: Send {
    /// 発火したときの処理。戻り値は次の発火時刻。
    fn fire(&mut self, context: &mut SourceContext<E, M>, model: &M) -> Option<Duration>;
}

// @todo デフォルトで用意されているとうれしい定期実行ソースのベースとかを用意する
