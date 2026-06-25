use crate::context::SourceContext;
use crate::modeling::model::Model;
use crate::primitive::time::Duration;

pub trait Source<E, M: Model<E>>: Send {
    /// シミュレーションを開始する前に行う初期化処理。
    /// ここで登録したイベントが[Duration::zero()]の場合は、[SimTime::zero()]の最初のマイクロステップに実行される。
    fn initialize(&mut self, context: &mut SourceContext<E, M>, model: &M);

    /// 発火したときの処理。戻り値は次の発火時刻。
    fn fire(&mut self, context: &mut SourceContext<E, M>, model: &M) -> Option<Duration>;
}
