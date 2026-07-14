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
    JobProcessNext,
}

#[derive(Debug)]
pub struct ServerModel {
    pub name: &'static str,
    pub queue: VecDeque<u32>,
    pub is_busy: bool,
}

/// Represents state-change commands sent from asynchronous worker threads to the main simulation thread.
#[derive(Debug)]
pub enum ServerCommand {
    /// Request to enqueue a job because the server is busy.
    EnqueueJob { job_id: u32 },
    /// Request to either process the next queued job or transition to idle.
    ProcessNextOrIdle,
}

// Standard Model trait implementation for synchronous execution (at specific priority levels).
impl Model<MyEvent> for ServerModel {
    fn handle_event(
        &mut self,
        _context: &mut EventContext<MyEvent, Self>,
        _event: &Event<MyEvent>,
    ) {
        // Implementation can be unified with apply_command if synchronous logic is needed.
    }
}

impl ParallelModel<MyEvent, ServerCommand> for ServerModel {
    /// "Asynchronous Execution" Safely performs parallel calculations using &self (immutable reference) on the thread pool.
    fn handle_event_parallel(&self, event: Event<MyEvent>, sender: Sender<ServerCommand>) {
        // Safely inspect the current state and send mutation commands through the channel.
        match event.payload {
            MyEvent::JobArrived { job_id } => {
                // Simulate varying computational costs based on server load.
                if self.is_busy {
                    std::thread::sleep(std::time::Duration::from_millis(700));
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
                sender.send(ServerCommand::EnqueueJob { job_id }).unwrap();
            }
            MyEvent::JobProcessed { job_id: _ } => {
                sender.send(ServerCommand::ProcessNextOrIdle).unwrap();
            }
            MyEvent::JobProcessNext => {
                sender.send(ServerCommand::ProcessNextOrIdle).unwrap();
            }
        }
    }

    /// "Synchronous Execution (Main Thread)" Safely applies commands received via the channel to &mut self.
    fn apply_command(&mut self, context: &mut EventContext<MyEvent, Self>, command: ServerCommand) {
        match command {
            ServerCommand::EnqueueJob { job_id } => {
                // Limit the number of concurrent processes.
                const WORKER_COUNT: usize = 3;
                let can_process_next = self.queue.len() < WORKER_COUNT;
                self.queue.push_back(job_id);

                if !self.is_busy {
                    self.is_busy = true;
                }

                // If there is capacity to process, trigger the next event.
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

// Implementation of ModelSummary for logging purposes.
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
        context: &mut dyn UserContext<MyEvent, ServerModel>,
        _model: &ServerModel,
    ) -> Option<Duration> {
        // Register the initial event.
        let job_id = self.next_job_id;
        self.next_job_id += 1;

        context.schedule_event(
            Duration::ticks(0),
            EventPriority::minimum(),
            MyEvent::JobArrived { job_id },
        );

        Some(self.interval)
    }

    fn fire(
        &mut self,
        context: &mut SourceContext<MyEvent, ServerModel>,
        _model: &ServerModel,
    ) -> Option<Duration> {
        // Batch register multiple events to demonstrate the value of asynchronous processing.
        for _ in 0..5 {
            let job_id = self.next_job_id;
            self.next_job_id += 1;

            context.schedule_event(
                Duration::ticks(0),
                EventPriority::minimum(),
                MyEvent::JobArrived { job_id },
            );
        }

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

    // Set sync_priority_threshold to EventPriority::maximum() to force all events to be processed asynchronously.
    let mut runner = ParallelRunner::new(true, EventPriority::maximum());
    let result = runner.run_do_ticks(engine, model, 60, false);

    println!("\nSimulation Result: {:?}", result);
}
