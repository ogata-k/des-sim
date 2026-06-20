use crate::primitive::time::Duration;
use crate::world::context::SourceContext;

pub trait Source<E, M, SC: SourceContext<E>>: Send {
    /// 発火したときの処理。戻り値は次の発火時刻。
    fn fire(&mut self, context: &mut SC, model: &M) -> Option<Duration>;
}

// @todo デフォルトで用意されているとうれしい定期実行ソースのベースとかを用意する
