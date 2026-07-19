# des-sim

[![Crates.io](https://img.shields.io/crates/v/des-sim.svg)](https://crates.io/crates/des-sim)
[![Docs.rs](https://docs.rs/des-sim/badge.svg)](https://docs.rs/des-sim)
[![License: MIT](https://img.shields.io/badge/License-MIT%20-blue.svg)](#ライセンス)

`des-sim`クレートは、Rustで実装された離散事象システム（Discrete Event System: DES）のための古典的な時間駆動型シミュレーションライブラリです。

離散事象システムのシミュレーションを、Rustの型安全かつ高速な環境で構築することを目的としています。

## インストール

Cargo を使用してプロジェクトに追加します。`Cargo.toml` の `[dependencies]` に以下を追記してください。

```toml
[dependencies]
des-sim = "0.1.0" # ※最新のバージョンを指定してください
```

## 使い方

以下は、`des-sim`を用いたシミュレーションの基本的な流れのイメージです。
より詳細な実装方法や動作するコードについては、[GitHubリポジリのexamples](https://github.com/ogata-k/des-sim/tree/master/examples)
をご覧ください。特に[`standard_runner.rs`](https://github.com/ogata-k/des-sim/blob/master/examples/standard_runner.rs)
が最初のステップとしておすすめです。

### サンプル

提供している基本的な機能をすべて盛り込んだサンプルは下記のようなコードになります。

```rust
use des_sim::context::{EventContext, SourceContext, UserContext};
use des_sim::execution::{Engine, runner::instance::StandardRunner};
use des_sim::execution::runner::Runner;
use des_sim::modeling::event::{Event, EventPriority};
use des_sim::modeling::hook::instance::{ModelSummary, TraceHook};
use des_sim::modeling::model::Model;
use des_sim::modeling::source::Source;
use des_sim::primitive::time::Duration;
use std::collections::VecDeque;
use std::fmt;

// シミュレーションで扱うイベントを定義します。
// `standard_runner.rs`の例では、ジョブの到着と処理完了を表すイベントです。
#[derive(Debug, Clone)]
pub enum MyEvent {
    JobArrived { job_id: u32 },
    JobProcessed { job_id: u32 },
}

// シミュレーション対象のシステム（モデル）を定義します。
// `standard_runner.rs`の例では、ジョブを処理するサーバーモデルです。
#[derive(Debug)]
pub struct ServerModel {
    pub name: &'static str,
    pub queue: VecDeque<u32>,
    pub is_busy: bool,
}

// Modelトレイトを実装し、イベント処理ロジックを記述します。
impl Model<MyEvent> for ServerModel {
    fn handle_event(&mut self, context: &mut EventContext<MyEvent, Self>, event: &Event<MyEvent>) {
        match event.payload {
            MyEvent::JobArrived { job_id } => {
                if self.is_busy {
                    // サーバーがビジーの場合、ジョブをキューに追加します。
                    self.queue.push_back(job_id);
                } else {
                    // サーバーがアイドル状態の場合、すぐに処理を開始し、5ティック後に完了イベントをスケジュールします。
                    self.is_busy = true;
                    context.schedule_event(
                        Duration::ticks(5),
                        EventPriority::minimum(),
                        MyEvent::JobProcessed { job_id },
                    );
                }
            }
            MyEvent::JobProcessed { job_id: _ } => {
                // 処理が完了したら、キューに次のジョブがあるか確認します。
                if let Some(next_id) = self.queue.pop_front() {
                    // 次のジョブがあれば、その処理をスケジュールします。
                    context.schedule_event(
                        Duration::ticks(5),
                        EventPriority::minimum(),
                        MyEvent::JobProcessed { job_id: next_id },
                    );
                } else {
                    // キューが空であれば、サーバーをアイドル状態にします。
                    self.is_busy = false;
                }
            }
        }
    }
}

// ModelSummaryトレイトを実装し、ログ出力時の要約を提供します。
impl ModelSummary for ServerModel {
    fn summary(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerModel")
            .field("name", &self.name)
            .field("queue_len", &self.queue.len())
            .field("busy", &self.is_busy)
            .finish()
    }
}

// イベントを定期的に生成するソースを定義します。
// `standard_runner.rs`の例では、ジョブを生成するジェネレータです。
#[derive(Debug)]
pub struct JobGenerator {
    next_job_id: u32,
    interval: Duration,
}

// Sourceトレイトを実装し、イベント生成ロジックを記述します。
impl Source<MyEvent, ServerModel> for JobGenerator {
    // ソースが登録された際に一度だけ呼び出されます。
    fn on_registered(
        &mut self,
        ctx: &mut dyn UserContext<MyEvent, ServerModel>,
        _model: &ServerModel,
    ) -> Option<Duration> {
        // 最初のジョブ到着イベントをスケジュールします。
        let job_id = self.next_job_id;
        self.next_job_id += 1;

        ctx.schedule_event(
            Duration::ticks(0), // 現在のティックでイベントを発生
            EventPriority::minimum(),
            MyEvent::JobArrived { job_id },
        );

        // 次にfireメソッドが呼び出されるまでの間隔を返します。
        Some(self.interval)
    }

    // intervalで指定された時間が経過するたびに呼び出されます。
    fn fire(
        &mut self,
        ctx: &mut SourceContext<MyEvent, ServerModel>,
        _model: &ServerModel,
    ) -> Option<Duration> {
        // 新しいジョブ到着イベントをスケジュールします。
        let job_id = self.next_job_id;
        self.next_job_id += 1;

        ctx.schedule_event(
            Duration::ticks(0), // 現在のティックでイベントを発生
            EventPriority::minimum(),
            MyEvent::JobArrived { job_id },
        );

        // 次にfireメソッドが呼び出されるまでの間隔を返します。
        Some(self.interval)
    }
}

fn main() {
    // 適切なログレベルと出力形式を設定
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("trace"))
        .format(|buf, record| {
            use std::io::Write;
            writeln!(
                buf,
                "{}",
                record.args()
            )
        })
        .init();

    // 1. シミュレーションエンジンの初期化
    let mut engine = Engine::new();

    // 2. フックやソースの登録
    // TraceHookはシミュレーションのイベントログを出力するフックです。
    // トレースの必要がなければTraceHookと出力用トレイトのModelSummaryの実装は不要です。
    engine.add_hook(TraceHook)
        .add_source(
            "Job Generator",
            JobGenerator { // JobGeneratorはイベントを定期的に生成するソースです
                next_job_id: 0,
                interval: Duration::ticks(5), // 5ティックごとにジョブを生成
            },
        );

    // 3. シミュレーションモデルの定義
    let model = ServerModel { // ServerModelはシミュレーション対象のシステムを表すモデルです
        name: "Sample Server",
        queue: Default::default(),
        is_busy: false,
    };

    // 4. ランナーの生成とシミュレーションの実行
    // StandardRunnerは処理しない時間をスキップし、高速にシミュレーションを実行する標準的なランナーです。
    // run_do_ticks(engine, model, 実行時間(ticks), ログ出力有無)
    let mut runner = StandardRunner::new(false); // ログ出力は無効
    let result = runner.run_do_ticks(engine, model, 60, false); // 60ティック実行

    println!("\nSimulation Result: {:?}", result);
}
```

上記ソースコードを実行すると下記のような出力が確認できます。
<details><summary>サンプルの出力</summary>

```text
--- [SIMULATION START] Time: SimTime(0) ---
  model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
> Start Register Source: Job Generator
  model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
< After Register Source: Job Generator
  model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  >>> Tick at SimTime(0) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 0 at SimTime(0)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Source Phase at SimTime(0) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Source Phase finished at SimTime(0) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Event Phase at SimTime(0) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      + Event firing | Event: Event { event_id: EventId(0), priority: EventPriority(0), payload: JobArrived { job_id: 0 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      - Event finished | Event: Event { event_id: EventId(0), priority: EventPriority(0), payload: JobArrived { job_id: 0 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(0) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(0)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(0) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(1) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(1)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(1) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(1) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(1) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(1) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(1)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(1) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(2) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(2)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(2) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(2) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(2) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(2) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(2)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(2) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(3) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(3)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(3) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(3) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(3) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(3) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(3)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(3) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(4) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(4)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(4) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(4) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(4) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(4) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(4)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(4) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(5) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(5)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(5) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Source firing | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Source finished (next: after Some(SimTime(10)) tick) | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(5) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(5) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Event firing | Event: Event { event_id: EventId(1), priority: EventPriority(0), payload: JobProcessed { job_id: 0 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Event finished | Event: Event { event_id: EventId(1), priority: EventPriority(0), payload: JobProcessed { job_id: 0 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Event Phase finished at SimTime(5) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 0 at SimTime(5)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 1 at SimTime(5)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Source Phase at SimTime(5) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Source Phase finished at SimTime(5) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Event Phase at SimTime(5) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      + Event firing | Event: Event { event_id: EventId(2), priority: EventPriority(0), payload: JobArrived { job_id: 1 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      - Event finished | Event: Event { event_id: EventId(2), priority: EventPriority(0), payload: JobArrived { job_id: 1 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(5) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 1 at SimTime(5)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(5) finished (last μSteps: 1)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(6) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(6)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(6) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(6) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(6) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(6) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(6)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(6) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(7) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(7)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(7) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(7) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(7) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(7) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(7)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(7) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(8) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(8)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(8) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(8) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(8) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(8) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(8)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(8) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(9) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(9)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(9) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(9) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(9) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(9) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(9)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(9) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(10) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(10)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(10) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Source firing | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Source finished (next: after Some(SimTime(15)) tick) | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(10) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(10) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Event firing | Event: Event { event_id: EventId(3), priority: EventPriority(0), payload: JobProcessed { job_id: 1 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Event finished | Event: Event { event_id: EventId(3), priority: EventPriority(0), payload: JobProcessed { job_id: 1 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Event Phase finished at SimTime(10) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 0 at SimTime(10)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 1 at SimTime(10)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Source Phase at SimTime(10) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Source Phase finished at SimTime(10) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Event Phase at SimTime(10) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      + Event firing | Event: Event { event_id: EventId(4), priority: EventPriority(0), payload: JobArrived { job_id: 2 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      - Event finished | Event: Event { event_id: EventId(4), priority: EventPriority(0), payload: JobArrived { job_id: 2 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(10) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 1 at SimTime(10)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(10) finished (last μSteps: 1)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(11) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(11)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(11) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(11) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(11) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(11) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(11)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(11) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(12) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(12)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(12) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(12) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(12) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(12) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(12)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(12) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(13) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(13)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(13) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(13) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(13) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(13) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(13)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(13) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(14) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(14)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(14) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(14) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(14) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(14) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(14)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(14) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(15) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(15)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(15) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Source firing | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Source finished (next: after Some(SimTime(20)) tick) | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(15) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(15) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Event firing | Event: Event { event_id: EventId(5), priority: EventPriority(0), payload: JobProcessed { job_id: 2 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Event finished | Event: Event { event_id: EventId(5), priority: EventPriority(0), payload: JobProcessed { job_id: 2 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Event Phase finished at SimTime(15) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 0 at SimTime(15)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 1 at SimTime(15)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Source Phase at SimTime(15) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Source Phase finished at SimTime(15) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Event Phase at SimTime(15) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      + Event firing | Event: Event { event_id: EventId(6), priority: EventPriority(0), payload: JobArrived { job_id: 3 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      - Event finished | Event: Event { event_id: EventId(6), priority: EventPriority(0), payload: JobArrived { job_id: 3 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(15) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 1 at SimTime(15)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(15) finished (last μSteps: 1)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(16) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(16)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(16) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(16) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(16) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(16) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(16)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(16) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(17) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(17)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(17) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(17) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(17) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(17) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(17)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(17) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(18) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(18)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(18) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(18) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(18) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(18) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(18)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(18) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(19) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(19)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(19) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(19) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(19) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(19) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(19)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(19) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(20) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(20)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(20) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Source firing | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Source finished (next: after Some(SimTime(25)) tick) | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(20) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(20) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Event firing | Event: Event { event_id: EventId(7), priority: EventPriority(0), payload: JobProcessed { job_id: 3 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Event finished | Event: Event { event_id: EventId(7), priority: EventPriority(0), payload: JobProcessed { job_id: 3 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Event Phase finished at SimTime(20) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 0 at SimTime(20)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 1 at SimTime(20)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Source Phase at SimTime(20) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Source Phase finished at SimTime(20) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Event Phase at SimTime(20) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      + Event firing | Event: Event { event_id: EventId(8), priority: EventPriority(0), payload: JobArrived { job_id: 4 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      - Event finished | Event: Event { event_id: EventId(8), priority: EventPriority(0), payload: JobArrived { job_id: 4 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(20) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 1 at SimTime(20)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(20) finished (last μSteps: 1)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(21) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(21)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(21) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(21) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(21) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(21) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(21)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(21) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(22) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(22)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(22) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(22) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(22) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(22) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(22)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(22) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(23) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(23)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(23) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(23) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(23) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(23) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(23)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(23) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(24) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(24)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(24) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(24) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(24) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(24) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(24)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(24) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(25) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(25)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(25) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Source firing | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Source finished (next: after Some(SimTime(30)) tick) | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(25) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(25) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Event firing | Event: Event { event_id: EventId(9), priority: EventPriority(0), payload: JobProcessed { job_id: 4 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Event finished | Event: Event { event_id: EventId(9), priority: EventPriority(0), payload: JobProcessed { job_id: 4 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Event Phase finished at SimTime(25) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 0 at SimTime(25)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 1 at SimTime(25)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Source Phase at SimTime(25) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Source Phase finished at SimTime(25) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Event Phase at SimTime(25) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      + Event firing | Event: Event { event_id: EventId(10), priority: EventPriority(0), payload: JobArrived { job_id: 5 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      - Event finished | Event: Event { event_id: EventId(10), priority: EventPriority(0), payload: JobArrived { job_id: 5 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(25) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 1 at SimTime(25)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(25) finished (last μSteps: 1)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(26) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(26)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(26) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(26) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(26) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(26) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(26)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(26) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(27) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(27)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(27) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(27) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(27) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(27) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(27)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(27) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(28) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(28)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(28) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(28) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(28) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(28) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(28)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(28) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(29) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(29)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(29) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(29) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(29) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(29) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(29)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(29) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(30) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(30)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(30) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Source firing | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Source finished (next: after Some(SimTime(35)) tick) | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(30) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(30) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Event firing | Event: Event { event_id: EventId(11), priority: EventPriority(0), payload: JobProcessed { job_id: 5 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Event finished | Event: Event { event_id: EventId(11), priority: EventPriority(0), payload: JobProcessed { job_id: 5 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Event Phase finished at SimTime(30) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 0 at SimTime(30)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 1 at SimTime(30)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Source Phase at SimTime(30) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Source Phase finished at SimTime(30) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Event Phase at SimTime(30) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      + Event firing | Event: Event { event_id: EventId(12), priority: EventPriority(0), payload: JobArrived { job_id: 6 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      - Event finished | Event: Event { event_id: EventId(12), priority: EventPriority(0), payload: JobArrived { job_id: 6 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(30) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 1 at SimTime(30)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(30) finished (last μSteps: 1)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(31) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(31)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(31) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(31) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(31) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(31) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(31)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(31) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(32) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(32)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(32) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(32) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(32) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(32) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(32)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(32) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(33) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(33)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(33) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(33) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(33) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(33) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(33)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(33) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(34) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(34)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(34) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(34) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(34) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(34) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(34)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(34) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(35) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(35)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(35) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Source firing | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Source finished (next: after Some(SimTime(40)) tick) | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(35) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(35) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Event firing | Event: Event { event_id: EventId(13), priority: EventPriority(0), payload: JobProcessed { job_id: 6 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Event finished | Event: Event { event_id: EventId(13), priority: EventPriority(0), payload: JobProcessed { job_id: 6 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Event Phase finished at SimTime(35) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 0 at SimTime(35)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 1 at SimTime(35)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Source Phase at SimTime(35) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Source Phase finished at SimTime(35) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Event Phase at SimTime(35) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      + Event firing | Event: Event { event_id: EventId(14), priority: EventPriority(0), payload: JobArrived { job_id: 7 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      - Event finished | Event: Event { event_id: EventId(14), priority: EventPriority(0), payload: JobArrived { job_id: 7 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(35) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 1 at SimTime(35)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(35) finished (last μSteps: 1)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(36) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(36)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(36) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(36) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(36) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(36) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(36)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(36) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(37) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(37)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(37) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(37) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(37) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(37) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(37)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(37) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(38) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(38)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(38) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(38) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(38) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(38) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(38)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(38) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(39) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(39)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(39) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(39) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(39) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(39) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(39)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(39) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(40) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(40)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(40) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Source firing | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Source finished (next: after Some(SimTime(45)) tick) | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(40) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(40) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Event firing | Event: Event { event_id: EventId(15), priority: EventPriority(0), payload: JobProcessed { job_id: 7 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Event finished | Event: Event { event_id: EventId(15), priority: EventPriority(0), payload: JobProcessed { job_id: 7 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Event Phase finished at SimTime(40) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 0 at SimTime(40)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 1 at SimTime(40)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Source Phase at SimTime(40) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Source Phase finished at SimTime(40) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Event Phase at SimTime(40) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      + Event firing | Event: Event { event_id: EventId(16), priority: EventPriority(0), payload: JobArrived { job_id: 8 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      - Event finished | Event: Event { event_id: EventId(16), priority: EventPriority(0), payload: JobArrived { job_id: 8 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(40) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 1 at SimTime(40)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(40) finished (last μSteps: 1)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(41) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(41)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(41) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(41) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(41) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(41) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(41)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(41) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(42) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(42)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(42) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(42) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(42) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(42) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(42)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(42) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(43) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(43)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(43) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(43) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(43) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(43) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(43)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(43) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(44) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(44)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(44) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(44) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(44) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(44) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(44)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(44) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(45) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(45)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(45) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Source firing | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Source finished (next: after Some(SimTime(50)) tick) | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(45) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(45) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Event firing | Event: Event { event_id: EventId(17), priority: EventPriority(0), payload: JobProcessed { job_id: 8 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Event finished | Event: Event { event_id: EventId(17), priority: EventPriority(0), payload: JobProcessed { job_id: 8 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Event Phase finished at SimTime(45) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 0 at SimTime(45)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 1 at SimTime(45)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Source Phase at SimTime(45) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Source Phase finished at SimTime(45) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Event Phase at SimTime(45) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      + Event firing | Event: Event { event_id: EventId(18), priority: EventPriority(0), payload: JobArrived { job_id: 9 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      - Event finished | Event: Event { event_id: EventId(18), priority: EventPriority(0), payload: JobArrived { job_id: 9 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(45) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 1 at SimTime(45)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(45) finished (last μSteps: 1)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(46) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(46)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(46) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(46) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(46) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(46) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(46)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(46) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(47) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(47)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(47) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(47) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(47) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(47) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(47)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(47) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(48) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(48)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(48) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(48) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(48) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(48) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(48)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(48) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(49) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(49)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(49) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(49) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(49) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(49) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(49)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(49) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(50) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(50)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(50) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Source firing | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Source finished (next: after Some(SimTime(55)) tick) | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(50) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(50) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Event firing | Event: Event { event_id: EventId(19), priority: EventPriority(0), payload: JobProcessed { job_id: 9 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Event finished | Event: Event { event_id: EventId(19), priority: EventPriority(0), payload: JobProcessed { job_id: 9 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Event Phase finished at SimTime(50) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 0 at SimTime(50)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 1 at SimTime(50)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Source Phase at SimTime(50) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Source Phase finished at SimTime(50) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Event Phase at SimTime(50) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      + Event firing | Event: Event { event_id: EventId(20), priority: EventPriority(0), payload: JobArrived { job_id: 10 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      - Event finished | Event: Event { event_id: EventId(20), priority: EventPriority(0), payload: JobArrived { job_id: 10 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(50) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 1 at SimTime(50)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(50) finished (last μSteps: 1)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(51) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(51)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(51) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(51) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(51) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(51) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(51)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(51) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(52) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(52)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(52) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(52) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(52) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(52) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(52)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(52) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(53) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(53)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(53) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(53) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(53) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(53) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(53)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(53) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(54) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(54)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(54) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(54) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(54) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(54) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(54)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(54) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(55) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(55)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(55) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Source firing | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Source finished (next: after Some(SimTime(60)) tick) | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(55) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(55) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Event firing | Event: Event { event_id: EventId(21), priority: EventPriority(0), payload: JobProcessed { job_id: 10 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Event finished | Event: Event { event_id: EventId(21), priority: EventPriority(0), payload: JobProcessed { job_id: 10 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Event Phase finished at SimTime(55) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 0 at SimTime(55)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 1 at SimTime(55)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Source Phase at SimTime(55) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Source Phase finished at SimTime(55) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Event Phase at SimTime(55) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      + Event firing | Event: Event { event_id: EventId(22), priority: EventPriority(0), payload: JobArrived { job_id: 11 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      - Event finished | Event: Event { event_id: EventId(22), priority: EventPriority(0), payload: JobArrived { job_id: 11 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(55) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 1 at SimTime(55)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(55) finished (last μSteps: 1)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(56) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(56)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(56) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(56) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(56) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(56) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(56)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(56) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(57) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(57)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(57) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(57) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(57) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(57) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(57)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(57) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(58) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(58)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(58) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(58) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(58) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(58) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(58)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(58) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(59) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(59)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(59) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(59) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(59) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(59) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(59)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(59) finished (last μSteps: 0)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  >>> Tick at SimTime(60) (skipped: 0 ticks)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 0 at SimTime(60)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Source Phase at SimTime(60) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Source firing | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Source finished (next: after Some(SimTime(65)) tick) | Source: SourceView { source_id: SourceId(0), name: "Job Generator" }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Source Phase finished at SimTime(60) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    > Event Phase at SimTime(60) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      + Event firing | Event: Event { event_id: EventId(23), priority: EventPriority(0), payload: JobProcessed { job_id: 11 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
      - Event finished | Event: Event { event_id: EventId(23), priority: EventPriority(0), payload: JobProcessed { job_id: 11 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Event Phase finished at SimTime(60) (μStep: 0)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 0 at SimTime(60)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
  [μStep: 1 at SimTime(60)] START
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Source Phase at SimTime(60) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    < Source Phase finished at SimTime(60) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
    > Event Phase at SimTime(60) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      + Event firing | Event: Event { event_id: EventId(24), priority: EventPriority(0), payload: JobArrived { job_id: 12 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: false }
      - Event finished | Event: Event { event_id: EventId(24), priority: EventPriority(0), payload: JobArrived { job_id: 12 } }
        model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
    < Event Phase finished at SimTime(60) (μStep: 1)
      model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  [μStep: 1 at SimTime(60)] END
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
  <<< Tick at SimTime(60) finished (last μSteps: 1)
    model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }
--- [SIMULATION END] Time: SimTime(60) ---
  model summary: ServerModel { name: "Sample Server", queue_len: 0, busy: true }

Simulation Result: Ok(SimulationOutput { time: SimTime(60), model: ServerModel { name: "Sample Server", queue: [], is_busy: true } })
```

</details>

## 離散事象システムと本クレートの特徴

離散事象システムとは、時間の発展に伴いイベントが離散的なタイミングで発火し、それを処理していくシステムのことです。

Pythonにおける[SimPy](https://simpy.readthedocs.io/en/latest/)のようなプロセス型シミュレーターとは異なり、`des-sim`は*
*古典的な時間駆動型シミュレーター**です。
プロセス型では「処理A → N秒待つ →
処理B」といった流れを一つのプロセス処理（ジェネレータ等）の中で記述しますが、本クレートでは「処理Aを実行し、N秒後の時間に処理Bのためのイベントをスケジュールする」というように、時間を進めながらその時間で処理すべきイベントを処理していく方式を採用しています。
書き味が異なるだけで、プロセス型も時間駆動型も実現できるシミュレーションの機能は同等です。

## モジュール構成

本クレートは、大きく分けて以下の3つのモジュールで構成されています。

### 1. `modeling` モジュール

利用者が主に触れる、シミュレーション対象をモデリングするためのモジュールです。

- **`event` / `model`**: シミュレーションの基本単位。これらを用いるだけで最低限のシミュレーションが可能です。
- **`source`**: 定期的にイベントをスケジュールするのに利用します。
- **`hook`**: シミュレーション実行時のログや状態を収集するのに利用します。
- **`sampler`**: 次のイベント時間をランダムに決めるなど、ランダムデータをサンプリングするのに利用します。

これらの要素は、基本的にはシミュレーション開始前に`execution`モジュールの`Engine`に登録して使用します。

### 2. `execution` モジュール

シミュレーションの実行環境を提供します。コアとなる`Engine`と、それを動かす`Runner`トレイトから構成されます。

- **`StandardRunner`**: 処理しない時間をスキップし、高速にシミュレーションを回す標準的なランナー。
- **`RealtimeRunner`**: 現実の時間経過に合わせてシミュレーションを実行するランナー。
- **`AsyncRunner`**: イベントの処理を非同期で行うことができるランナー。

※ `Model`、`Source`、`Hook`が決定論的（実行のたびに結果が変わらない）に実装されていれば、`StandardRunner`と`RealtimeRunner`
を用いたシミュレーションも決定論的となり、一度行ったシミュレーションの完全な再現が可能になります。自前の`Runner`
トレイトを実装することも可能です。

### 3. `context` モジュール

イベントのスケジュールなど、実行環境に対して処理を依頼する際のコンテキストを提供します。
`modeling`モジュール内で引数として`Context`が渡されるため、提供されるメソッドを呼び出すだけで環境を操作できます。

## 貢献

より使いやすいクレートを目指して継続的に改善を行っていきたいと考えています。
利用していただいた感想、バグ報告、機能要望などがございましたら、ぜひGitHubにてIssueやPull Requestをお寄せください！

## ライセンス

This project is licensed under either of

* [MIT license](LICENSE)