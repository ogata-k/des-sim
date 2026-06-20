pub mod instance;

use crate::context::ExecutorStatus;
use crate::execution::SimulationResult;
use crate::execution::engine::Engine;
use crate::modeling::model::Model;
use crate::primitive::time::{TickStatus, TimeTick};

pub trait Runner<E, M: Model<E>> {
    type Err: std::fmt::Debug;

    /// 0-originで時刻を刻みながらシミュレーションを回すrunner。
    /// 0 tickは必ず最初に呼び出される。
    ///
    /// # Arguments
    ///
    /// * `engine` - シミュレーションエンジン
    /// * `model` - 初期のモデル状態
    /// * `should_stop` - 毎Tick呼び出される停止条件クロージャ。
    ///   シグネチャは `FnMut(&M, ExecutorStatus, TickStatus) -> bool` で、各引数は以下の通り：
    ///   * `&M` - 今処理を開始しようとしているtickの時点でのモデル
    ///   * `ExecutorStatus` - 今処理を開始しようとしているtickの時点での実行状態
    ///   * `TickStatus` - 今処理を開始しようとしているtickの状態
    fn run<F>(
        &mut self,
        engine: Engine<E, M>,
        model: M,
        should_stop: F,
    ) -> SimulationResult<M, Self::Err>
    where
        // 内部で３回エラーが起きたら終了するみたいなことをやるためにFnMutとしている。
        F: FnMut(
            &M,             /* current model on the tick */
            ExecutorStatus, /* status of scheduled on executor on the tick */
            TickStatus,     /* next handle tick status */
        ) -> bool;

    /// runと同じだが、tickを進めるのにEnterキーの入力が必要のrunメソッド。
    /// デバッグ用。
    fn interactive_debug_run<F>(
        &mut self,
        engine: Engine<E, M>,
        model: M,
        mut should_stop: F,
    ) -> SimulationResult<M, Self::Err>
    where
        // 内部で３回エラーが起きたら終了するみたいなことをやるためにFnMutとしている。
        F: FnMut(
            &M,             /* current model on the tick */
            ExecutorStatus, /* status of scheduled on executor on the tick */
            TickStatus,     /* next handle tick status */
        ) -> bool,
    {
        use std::io::{self, Write};

        // 元の `run` メソッドに、Enter待機ロジックを挟んだクロージャを流し込む
        self.run(engine, model, |model, executor_status, tick_status| {
            // 1. まず本来の終了判定をチェック（本来の条件で終わるならデバッグも終了）
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

            // 3. ユーザーがEnterを押すまでブロック（待機）
            let mut input = String::new();
            let _ = io::stdin().read_line(&mut input);

            false // falseを返すことで、ループを抜けずに次のTickの処理への突入を許可
        })
    }

    /// 指定した時間間隔durationだけウェイトを挟みながら自動進行するrunメソッド。
    /// CUI/GUIでのアニメーション描画や、リアルタイム風のログ監視に最適。
    fn run_playback<F>(
        &mut self,
        engine: Engine<E, M>,
        model: M,
        duration: std::time::Duration,
        mut should_stop: F,
    ) -> SimulationResult<M, Self::Err>
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

    /// tick_count分の時間が経過するまでシミュレーションを処理して終了するrunメソッド。
    /// [Runner]が時間をスキップ（ジャンプ）させることがあるため、ジャンプ発生時は
    /// 指定された閾値をまたいだ最初の実処理を終えたタイミングで終了する。
    fn run_do_ticks(
        &mut self,
        engine: Engine<E, M>,
        model: M,
        tick_count: TimeTick,
    ) -> SimulationResult<M, Self::Err> {
        self.run(
            engine,
            model,
            |_, _, next_handle_tick_status: TickStatus| {
                // Runnerによってはスキップされてしまう可能性があるため、ちゃんと処理した時間で判定する。
                // previous()+1となっているのは、tick_count=2のときに、0 tick → 1 tick → 2 tickと来たときに2 tickの処理開始時に止めるため。
                // そして、同じくtick_count=2のときに、0 tick →skip→ 5 tick → 6 tickと来た時にも6 tickの処理開始時に止めるため。
                next_handle_tick_status.previous().as_tick_value() + 1 >= tick_count
            },
        )
    }

    /// 処理するイベントがなくなったら終了するrunメソッド。
    fn run_until_idle(&mut self, engine: Engine<E, M>, model: M) -> SimulationResult<M, Self::Err> {
        self.run(engine, model, |_, executor_status: ExecutorStatus, _| {
            executor_status == ExecutorStatus::NoMoreEvent
        })
    }

    /// モデルが特定の条件に達したら終了するrunメソッド。
    fn run_until_model_condition<F>(
        &mut self,
        engine: Engine<E, M>,
        model: M,
        mut should_stop_model_condition: F,
    ) -> SimulationResult<M, Self::Err>
    where
        F: FnMut(&M /* current model on the tick */) -> bool,
    {
        self.run(engine, model, |model: &M, _, _| {
            should_stop_model_condition(model)
        })
    }
}
