use des_sim::context::{EventContext, SourceContext};
use des_sim::execution::Engine;
use des_sim::execution::runner::Runner;
use des_sim::execution::runner::instance::StandardRunner;
use des_sim::modeling::event::{Event, EventPriority};
use des_sim::modeling::hook::instance::{ModelSummary, TraceHook};
use des_sim::modeling::model::Model;
use des_sim::modeling::source::Source;
use des_sim::primitive::time::{Duration, SimTime};
use std::collections::VecDeque;
use std::fmt;

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
}

// --- [2. モデルの定義] ---
#[derive(Debug)]
pub struct ServerModel {
    pub name: &'static str,
    pub queue: VecDeque<u32>,
    pub is_busy: bool,
}

// 前提となる Model トレイトの実装
impl Model<MyEvent> for ServerModel {
    fn handle_event(&mut self, context: &mut EventContext<MyEvent, Self>, event: &Event<MyEvent>) {
        match event.payload {
            MyEvent::JobArrived { job_id } => {
                if self.is_busy {
                    // サーバーが処理中ならキューに積む
                    self.queue.push_back(job_id);
                } else {
                    // 空いていれば即座に処理を開始し、5 tick後に完了イベントをセット
                    self.is_busy = true;
                    context.schedule_event(
                        Duration::ticks(5),
                        EventPriority::minimum(),
                        MyEvent::JobProcessed { job_id },
                    );
                }
            }
            MyEvent::JobProcessed { job_id: _ } => {
                // キューに次のジョブがあれば処理、なければアイドルへ
                if let Some(next_id) = self.queue.pop_front() {
                    context.schedule_event(
                        Duration::ticks(5),
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

// ログ視認性を高めるために作った ModelSummary の実装
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
    fn initialize(&mut self, ctx: &mut SourceContext<MyEvent, ServerModel>, _model: &ServerModel) {
        // 最初に一つイベントを登録する
        let job_id = self.next_job_id;
        self.next_job_id += 1;

        ctx.schedule_event(
            Duration::ticks(0),
            EventPriority::minimum(),
            MyEvent::JobArrived { job_id },
        );
    }

    // ソースが発火したときの挙動
    fn fire(
        &mut self,
        ctx: &mut SourceContext<MyEvent, ServerModel>,
        _model: &ServerModel,
    ) -> Option<Duration> {
        let job_id = self.next_job_id;
        self.next_job_id += 1;

        // ジョブが到着したというイベントを今すぐ（あるいはディレイで）スケジュール
        ctx.schedule_event(
            Duration::ticks(0),
            EventPriority::minimum(),
            MyEvent::JobArrived { job_id },
        );

        // 次の発火タイミングを設定（Periodic Combinatorのような動きを自前で表現）
        Some(self.interval)
    }
}

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
            SimTime::zero(),
            JobGenerator {
                next_job_id: 0,
                interval: Duration::ticks(4),
            },
        )
        .add_source(
            "Job Generator x6",
            SimTime::zero(),
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

    let mut runner = StandardRunner::new(true);
    let result = runner.run_do_ticks(engine, model, 60, false);
    print!("\nSimulation Result: {:?}", result);
}
