use des_sim::context::{EventContext, SourceContext, UserContext};
use des_sim::execution::Engine;
use des_sim::execution::runner::Runner;
use des_sim::execution::runner::instance::StandardRunner;
use des_sim::modeling::event::{Event, EventPriority};
use des_sim::modeling::hook::instance::{InteractiveStepHook, ModelSummary, TraceHook};
use des_sim::modeling::model::Model;
use des_sim::modeling::source::Source;
use des_sim::primitive::time::{Duration, TickStatus};
use std::collections::VecDeque;
use std::fmt;

#[cfg(test)]
mod tests {
    // Verifies that the sample simulation completes execution successfully.
    #[test]
    fn example_runs() {
        super::main();
    }
}

#[derive(Debug, Clone)]
pub enum MyEvent {
    JobArrived { job_id: u32 },
    JobProcessed { job_id: u32 },
}

#[derive(Debug)]
pub struct ServerModel {
    pub name: &'static str,
    pub queue: VecDeque<u32>,
    pub is_busy: bool,
}

// Implementation of the Model trait for event processing logic.
impl Model<MyEvent> for ServerModel {
    fn handle_event(&mut self, context: &mut EventContext<MyEvent, Self>, event: &Event<MyEvent>) {
        match event.payload {
            MyEvent::JobArrived { job_id } => {
                if self.is_busy {
                    // Enqueue the job if the server is currently busy.
                    self.queue.push_back(job_id);
                } else {
                    // Start processing immediately and schedule completion after 5 ticks.
                    self.is_busy = true;
                    context.schedule_event(
                        Duration::ticks(5),
                        EventPriority::minimum(),
                        MyEvent::JobProcessed { job_id },
                    );
                }
            }
            MyEvent::JobProcessed { job_id: _ } => {
                // If there are queued jobs, process the next one; otherwise, set server to idle.
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

// Implementation of ModelSummary for clear logging output.
impl ModelSummary for ServerModel {
    fn summary(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerModel")
            .field("name", &self.name)
            .field("queue_len", &self.queue.len())
            .field("busy", &self.is_busy)
            .finish()
    }
}

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
        // Register the initial event.
        let job_id = self.next_job_id;
        self.next_job_id += 1;

        ctx.schedule_event(
            Duration::ticks(0),
            EventPriority::minimum(),
            MyEvent::JobArrived { job_id },
        );

        Some(self.interval)
    }

    // Behavior when the source fires.
    fn fire(
        &mut self,
        ctx: &mut SourceContext<MyEvent, ServerModel>,
        _model: &ServerModel,
    ) -> Option<Duration> {
        let job_id = self.next_job_id;
        self.next_job_id += 1;

        // Schedule the job arrival event.
        ctx.schedule_event(
            Duration::ticks(0),
            EventPriority::minimum(),
            MyEvent::JobArrived { job_id },
        );

        // Schedule the next fire event.
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
        .add_hook(InteractiveStepHook)
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

    let include_zero_tick = true;
    let tick_count = 60;
    let mut runner = StandardRunner::new(true);

    // Run the simulation with a custom stop condition.
    let result = runner.run(
        engine,
        model,
        |_, _, next_handle_tick_status: TickStatus| {
            next_handle_tick_status.is_done_ticks(include_zero_tick, tick_count)
        },
    );

    println!("\nSimulation Result: {:?}", result);
}
