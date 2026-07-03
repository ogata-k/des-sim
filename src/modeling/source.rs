use crate::context::{SourceContext, UserContext};
use crate::modeling::model::Model;
use crate::primitive::time::Duration;

// source_handlerのまま公開するのは名前が悪い上に漏らしたくない情報だらけなので、
// modeling用のSourceと一緒に公開させる。
pub use crate::source_handler::{SourceReadyEntry, SourceView};

pub trait Source<E, M: Model<E>>: Send {
    /// [Source]を登録したときに実行する初期化処理。
    /// シミュレーション開始時・シミュレーション実行中にかかわらず登録したときに実行される。
    /// シミュレーション開始時は[SourceContext]がcontextに、シミュレーション中は[EventContext](crate::context::EventContext)が利用されoる。
    ///
    /// この中で[Duration::zero()]のイベントを登録し、そのイベントでこのSourceを登録するようになっている場合、マイクロステップが無限に続いてしまうので注意が必要。
    fn on_registered(&mut self, context: &mut dyn UserContext<E, M>, model: &M)
    -> Option<Duration>;

    /// 発火したときの処理。戻り値は次の発火時刻。
    fn fire(&mut self, context: &mut SourceContext<E, M>, model: &M) -> Option<Duration>;
}
