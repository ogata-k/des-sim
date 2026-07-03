use des_sim::context::{EventContext, SourceContext, UserContext};
use des_sim::execution::Engine;
use des_sim::execution::runner::Runner;
use des_sim::execution::runner::instance::{ParallelModel, ParallelRunner};
use des_sim::modeling::event::{Event, EventPriority};
use des_sim::modeling::hook::instance::{ModelSummary, TraceHook};
use des_sim::modeling::model::Model;
use des_sim::modeling::source::Source;
use des_sim::primitive::time::Duration;
use std::collections::VecDeque;
use std::fmt;
use std::sync::mpsc::Sender;

#[cfg(test)]
mod tests {
    // 最後までサンプルが走りきることをテスト
    #[test]
    fn example_runs() {
        super::main();
    }
}

// --- [1. イベントの定義] ---
#[derive(Debug, Clone)]
pub enum MyEvent {
    JobArrived { job_id: u32 },
    JobProcessed { job_id: u32 },
    JobProcessNext,
}

// --- [2. モデルと非同期コマンドの定義] ---
#[derive(Debug)]
pub struct ServerModel {
    pub name: &'static str,
    pub queue: VecDeque<u32>,
    pub is_busy: bool,
}

/// 非同期スレッドからメインスレッドに通知する、モデルへの状態変更要求（コマンド）
#[derive(Debug)]
pub enum ServerCommand {
    /// サーバーがビジーなのでキューにジョブを積む要求
    EnqueueJob { job_id: u32 },
    /// 次のジョブを処理開始するか、アイドルに戻す要求
    ProcessNextOrIdle,
}

// 同期実行（特定の優先度以上）の際に呼ばれる標準の Model トレイト実装
impl Model<MyEvent> for ServerModel {
    fn handle_event(
        &mut self,
        _context: &mut EventContext<MyEvent, Self>,
        _event: &Event<MyEvent>,
    ) {
        // 同期実行された場合も、ロジックを一貫させるために apply_command を再利用可能ですが、
        // ここでは直接記述、または後述の apply_command にコンテキストを委譲する形を取ります。
        // 今回は非同期実行をメインにするため、元の実装を維持するか、あるいは非同期側へ処理を一本化します。
    }
}

impl ParallelModel<MyEvent, ServerCommand> for ServerModel {
    /// 【非同期実行】スレッドプール上で &self (不変参照) を使って安全に並列計算
    fn handle_event_parallel(&self, event: Event<MyEvent>, tx: Sender<ServerCommand>) {
        // 現在の状態を安全に読み取り、書き換えコマンドをチャンネルに送る
        match event.payload {
            MyEvent::JobArrived { job_id } => {
                // ビジー状態かどうかで計算コストが変わるさまをシミュレート
                if self.is_busy {
                    std::thread::sleep(std::time::Duration::from_millis(700));
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
                tx.send(ServerCommand::EnqueueJob { job_id }).unwrap();
            }
            MyEvent::JobProcessed { job_id: _ } => {
                tx.send(ServerCommand::ProcessNextOrIdle).unwrap();
            }
            MyEvent::JobProcessNext => {
                tx.send(ServerCommand::ProcessNextOrIdle).unwrap();
            }
        }
    }

    /// 【同期実行（メインスレッド）】チャンネルから届いたコマンドを安全に &mut self 適用
    fn apply_command(&mut self, context: &mut EventContext<MyEvent, Self>, command: ServerCommand) {
        match command {
            ServerCommand::EnqueueJob { job_id } => {
                // 同時処理数を制限
                // 想定処理イメージは、フォーク型の待ち行列モデル
                const WORKER_COUNT: usize = 3;
                let can_process_next = self.queue.len() < WORKER_COUNT;
                self.queue.push_back(job_id);
                if !self.is_busy {
                    self.is_busy = true;
                }
                // キューに詰める前の状態で処理待ちがあるならそちらで処理されるので発火は不要
                if can_process_next {
                    context.schedule_event(
                        Duration::ticks(0),
                        EventPriority::minimum(),
                        MyEvent::JobProcessNext,
                    );
                }
            }
            ServerCommand::ProcessNextOrIdle => {
                if let Some(next_id) = self.queue.pop_front() {
                    self.is_busy = true;
                    context.schedule_event(
                        Duration::ticks(2),
                        EventPriority::minimum(),
                        MyEvent::JobProcessed { job_id: next_id },
                    );
                } else {
                    self.is_busy = false;
                }
            }
        }
    }
}

// ログ表示用の ModelSummary の実装
impl ModelSummary for ServerModel {
    fn summary(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerModel")
            .field("name", &self.name)
            .field("queue_len", &self.queue.len())
            .field("busy", &self.is_busy)
            .finish()
    }
}

// --- [3. ソースの定義 (定期的なジョブ生成)] ---
#[derive(Debug)]
pub struct JobGenerator {
    next_job_id: u32,
    interval: Duration,
}

impl Source<MyEvent, ServerModel> for JobGenerator {
    fn on_registered(
        &mut self,
        ctx: &mut dyn UserContext<MyEvent, ServerModel>,
        _model: &ServerModel,
    ) -> Option<Duration> {
        // 最初に一つイベントを登録する
        let job_id = self.next_job_id;
        self.next_job_id += 1;

        ctx.schedule_event(
            Duration::ticks(0),
            EventPriority::minimum(),
            MyEvent::JobArrived { job_id },
        );

        Some(self.interval)
    }

    fn fire(
        &mut self,
        ctx: &mut SourceContext<MyEvent, ServerModel>,
        _model: &ServerModel,
    ) -> Option<Duration> {
        // 非同期処理のありがたみを見るために、多めにイベントを一括登録する。
        for _ in 0..5 {
            let job_id = self.next_job_id;
            self.next_job_id += 1;

            ctx.schedule_event(
                Duration::ticks(0),
                EventPriority::minimum(),
                MyEvent::JobArrived { job_id },
            );
        }

        Some(self.interval)
    }
}

// --- [4. シミュレーションの実行 (main)] ---
fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("trace"))
        .format(|buf, record| {
            use std::io::Write;
            writeln!(
                buf,
                "[{}] {:<5} {}",
                chrono::Local::now().format("%H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .init();

    let mut engine = Engine::new();
    engine
        .add_hook(TraceHook)
        .add_source(
            "Job Generator x4",
            JobGenerator {
                next_job_id: 0,
                interval: Duration::ticks(4),
            },
        )
        .add_source(
            "Job Generator x6",
            JobGenerator {
                next_job_id: 0,
                interval: Duration::ticks(6),
            },
        );

    let model = ServerModel {
        name: "Sample Server",
        queue: Default::default(),
        is_busy: false,
    };

    // すべてのイベントを非同期処理の対象にするため、sync_priority_threshold には EventPriority::maximum() などを設定。
    // (もし特定の重要イベントだけを同期させたい場合は、その優先度を閾値に指定します)
    let mut runner = ParallelRunner::<ServerCommand, _>::new(true, EventPriority::maximum());
    let result = runner.run_do_ticks(engine, model, 60, false);

    print!("\nSimulation Result: {:?}", result);
}
