pub mod instance;

use crate::context::ExecutorStatus;
use crate::execution::SimulationResult;
use crate::execution::engine::Engine;
use crate::execution::strategy::ContinueStrategy;
use crate::modeling::model::Model;
use crate::primitive::time::{TickStatus, TimeTick};

/// シミュレーションの実行制御を司るトレイト。
///
/// このトレイトの核となる `run` メソッドを実装することで、
/// さまざまな実行モード（デバッグ、再生ウェイト、並列バッチなど）を実現できます。
pub trait Runner<E, M: Model<E>, CS: ContinueStrategy<E, M, Self::Err>> {
    /// シミュレーション実行中に発生し得る、実装固有のエラー型
    type Err: std::fmt::Debug;

    /// 0-originで時刻を刻みながらシミュレーションを回す、コアとなる基本実行メソッド。
    /// 0 tickは必ず最初に呼び出されます。
    ///
    /// # Arguments
    ///
    /// * `engine` - シミュレーションエンジン（イベントキューやコンテキストの管理者）
    /// * `model` - 初期のモデル状態
    /// * `should_stop` - 毎Tickの処理開始前に呼び出される停止条件クロージャ。
    ///   内部で特定のエラーが複数回起きたら終了する、といった状態管理を可能にするため `FnMut` となっています。
    ///   * `&M` - 今処理を開始しようとしているTickの時点でのモデル状態
    ///   * `ExecutorStatus` - 今処理を開始しようとしているTickの時点でのエグゼキュータの実行状態
    ///   * `TickStatus` - 今処理を開始しようとしているTickの時刻状態
    fn run<F>(
        &mut self,
        engine: Engine<E, M>,
        model: M,
        should_stop: F,
    ) -> SimulationResult<M, CS::Err>
    where
        F: FnMut(
            &M,             /* current model on the tick */
            ExecutorStatus, /* status of scheduled on executor on the tick */
            TickStatus,     /* next handle tick status */
        ) -> bool;

    /// 1 Tick進めるごとにユーザーの Enter キー入力を待機するデバッグ用メソッド。
    ///
    /// コンソール上で現在のシミュレーション時刻やエグゼキュータの状態を、
    /// 人間が目で追いながらステップ実行（紙芝居進行）することができます。
    fn interactive_debug_run<F>(
        &mut self,
        engine: Engine<E, M>,
        model: M,
        mut should_stop: F,
    ) -> SimulationResult<M, CS::Err>
    where
        F: FnMut(&M, ExecutorStatus, TickStatus) -> bool,
    {
        use std::io::{self, Write};

        // 元の `run` メソッドに、Enter待機ロジックをインジェクションしたクロージャを流し込むだけ
        self.run(engine, model, |model, executor_status, tick_status| {
            // 1. 本来の終了判定をチェック（本来の条件で終わるならデバッグもここで終了）
            if should_stop(model, executor_status, tick_status) {
                println!("\n [Debug] シミュレーションが終了条件に達したため、停止します。");
                return true;
            }

            // 2. 終了しないで次のTickに進む場合、情報をコンソールに表示して待機
            println!("\n================ [Interactive Debug] ================");
            println!(
                "  直前に完了した時刻 (previous) : {:?}",
                tick_status.previous()
            );
            println!(
                "  これから処理する時刻 (current) : {:?}",
                tick_status.current()
            );
            println!("  エグゼキュータの状態 (status)  : {:?}", executor_status);
            println!("--------------------------------------------------------");
            print!(" [Enter] を押すと次のTickの処理（フェーズ実行）を開始します... ");
            let _ = io::stdout().flush(); // プロンプトを確実に表示させる

            // 3. ユーザーがEnterを押すまでスレッドをブロック（待機）
            let mut input = String::new();
            let _ = io::stdin().read_line(&mut input);

            false // falseを返すことで、次のTickの処理への突入を許可
        })
    }

    /// 指定した時間間隔（ウェイト）を挟みながら自動進行する再生用メソッド。
    ///
    /// CUI/GUIでのリアルタイムなアニメーション描画や、
    /// 人間がログを監視しやすい速度でシミュレーションを「眺める」際に最適です。
    fn run_playback<F>(
        &mut self,
        engine: Engine<E, M>,
        model: M,
        duration: std::time::Duration,
        mut should_stop: F,
    ) -> SimulationResult<M, CS::Err>
    where
        F: FnMut(&M, ExecutorStatus, TickStatus) -> bool,
    {
        self.run(engine, model, |model, exec_status, tick_status| {
            if should_stop(model, exec_status, tick_status) {
                return true;
            }
            // 1 Tick の処理が終わるごとに、指定時間だけスレッドをスリープさせる
            std::thread::sleep(duration);
            false
        })
    }

    /// 指定されたTick数が経過するまでシミュレーションを進行して終了するメソッド。
    ///
    /// 高速化のために `Runner` が時間をスキップ（ジャンプ）させることがあるため、
    /// ジャンプ発生時は、指定された閾値をまたいだ最初の実処理を終えたタイミングで安全に終了します。
    fn run_do_ticks(
        &mut self,
        engine: Engine<E, M>,
        model: M,
        tick_count: TimeTick,
        include_zero_tick: bool,
    ) -> SimulationResult<M, CS::Err> {
        self.run(
            engine,
            model,
            |_, _, next_handle_tick_status: TickStatus| {
                next_handle_tick_status.is_done_ticks(include_zero_tick, tick_count)
            },
        )
    }

    /// 処理すべきイベント（キュー）が完全に空になったら自動で終了するメソッド。
    fn run_until_idle(&mut self, engine: Engine<E, M>, model: M) -> SimulationResult<M, CS::Err> {
        self.run(engine, model, |_, executor_status: ExecutorStatus, _| {
            executor_status == ExecutorStatus::NoMoreEvent
        })
    }

    /// モデルの状態（ドメインロジック）が特定の条件を満たしたら終了するメソッド。
    ///
    /// 「特定のキューがパンクした」「目標の生産数に達した」など、
    /// モデル内部の数値をトリガーにした終了判定をシンプルに記述できます。
    fn run_until_model_condition<F>(
        &mut self,
        engine: Engine<E, M>,
        model: M,
        mut should_stop_model_condition: F,
    ) -> SimulationResult<M, CS::Err>
    where
        F: FnMut(&M) -> bool,
    {
        self.run(engine, model, |model: &M, _, _| {
            should_stop_model_condition(model)
        })
    }

    /// 【マルチスレッド版】
    /// 条件を変えたシミュレーションを指定回数並列実行することができるメソッド。
    /// 必要であれば、初期モデルを微調整して初期化することもできます。
    /// 全CPUコアをフル稼働させてモンテカルロ法やパラメトリックスタディを爆速で実行します。
    ///
    /// # Arguments
    ///
    /// * `count` - 実行するシミュレーションの総試行回数。
    /// * `engine_builder` - 各スレッドで独立した `Engine` を量産するためのファクトリ関数（不変クロージャ）。
    ///   引数として現在の「実行インデックス（0〜count-1）」が渡されるため、インデックスに応じた
    ///   異なる乱数シードや設定を持つ Engine をスレッドセーフに動的生成できます。
    /// * `model_builder` - 初期のモデル状態
    /// * `should_stop` - 毎Tick呼び出される停止条件クロージャ。
    fn run_batch_parallel<MF, EF, F>(
        &self,
        count: usize,
        engine_builder: EF,
        model_builder: MF,
        should_stop: F,
    ) -> Vec<SimulationResult<M, CS::Err>>
    where
        Self: Clone + Sync,
        // 並列スレッドから同時に何回でも安全に呼び出せるよう、FnMut ではなく不変の Fn に制限
        EF: Fn(usize) -> Engine<E, M> + Send + Sync,
        M: Send + Sync,
        // 並列スレッドから同時に何回でも安全に呼び出せるよう、FnMut ではなく不変の Fn に制限
        MF: Fn(usize) -> M + Send + Sync,
        F: FnMut(&M, ExecutorStatus, TickStatus) -> bool + Clone + Send + Sync,
        Self::Err: Send,
        CS::Err: Send,
    {
        use rayon::prelude::*;

        // 結果のインデックスをengine_builderに入れるため、インデックス配列をイテレートしてあつめる。
        (0..count)
            .into_par_iter()
            .map(|index| {
                let mut local_runner = self.clone();
                let local_engine = engine_builder(index);
                let local_model = model_builder(index);

                local_runner.run(local_engine, local_model, should_stop.clone())
            })
            .collect()
    }
}
