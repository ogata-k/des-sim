# des-sim

[![Crates.io](https://img.shields.io/crates/v/des-sim.svg)](https://crates.io/crates/des-sim)
[![Docs.rs](https://docs.rs/des-sim/badge.svg)](https://docs.rs/des-sim)
[![License: MIT](https://img.shields.io/badge/License-MIT%20-blue.svg)](#License)

The `des-sim` crate is a classic time-driven simulation library for Discrete Event Systems (DES) implemented in Rust.

It aims to build discrete event system simulations in a type-safe and high-performance Rust environment.

## Installation

Add it to your project using Cargo. Add the following to the `[dependencies]` section of your `Cargo.toml`:

```toml
[dependencies]
des-sim = "0.1.0" # *Please specify the latest version
```

## Usage

Below is an image of the basic flow of a simulation using `des-sim`.
For more detailed implementation methods and working code, please refer to
the [GitHub repository examples](https://github.com/ogata-k/des-sim/tree/master/examples).
[`standard_runner.rs`](https://github.com/ogata-k/des-sim/blob/master/examples/standard_runner.rs) is especially
recommended as a first step.

### Example

A sample incorporating all the basic features provided is shown below.

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

// Defines the events handled in the simulation.
// In the `standard_runner.rs` example, these represent job arrival and processing completion events.
#[derive(Debug, Clone)]
pub enum MyEvent {
    JobArrived { job_id: u32 },
    JobProcessed { job_id: u32 },
}

// Defines the system (model) to be simulated.
// In the `standard_runner.rs` example, this is a server model that processes jobs.
#[derive(Debug)]
pub struct ServerModel {
    pub name: &'static str,
    pub queue: VecDeque<u32>,
    pub is_busy: bool,
}

// Implements the Model trait and describes the event processing logic.
impl Model<MyEvent> for ServerModel {
    fn handle_event(&mut self, context: &mut EventContext<MyEvent, Self>, event: &Event<MyEvent>) {
        match event.payload {
            MyEvent::JobArrived { job_id } => {
                if self.is_busy {
                    // If the server is busy, add the job to the queue.
                    self.queue.push_back(job_id);
                } else {
                    // If the server is idle, start processing immediately and schedule a completion event after 5 ticks.
                    self.is_busy = true;
                    context.schedule_event(
                        Duration::ticks(5),
                        EventPriority::minimum(),
                        MyEvent::JobProcessed { job_id },
                    );
                }
            }
            MyEvent::JobProcessed { job_id: _ } => {
                // After processing is complete, check if there is a next job in the queue.
                if let Some(next_id) = self.queue.pop_front() {
                    // If there is a next job, schedule its processing.
                    context.schedule_event(
                        Duration::ticks(5),
                        EventPriority::minimum(),
                        MyEvent::JobProcessed { job_id: next_id },
                    );
                } else {
                    // If the queue is empty, set the server to idle.
                    self.is_busy = false;
                }
            }
        }
    }
}

// Implements the ModelSummary trait to provide a summary for logging.
impl ModelSummary for ServerModel {
    fn summary(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerModel")
            .field("name", &self.name)
            .field("queue_len", &self.queue.len())
            .field("busy", &self.is_busy)
            .finish()
    }
}

// Defines a source that periodically generates events.
// In the `standard_runner.rs` example, this is a generator that creates jobs.
#[derive(Debug)]
pub struct JobGenerator {
    next_job_id: u32,
    interval: Duration,
}

// Implements the Source trait and describes the event generation logic.
impl Source<MyEvent, ServerModel> for JobGenerator {
    // Called once when the source is registered.
    fn on_registered(
        &mut self,
        ctx: &mut dyn UserContext<MyEvent, ServerModel>,
        _model: &ServerModel,
    ) -> Option<Duration> {
        // Schedule the first job arrival event.
        let job_id = self.next_job_id;
        self.next_job_id += 1;

        ctx.schedule_event(
            Duration::ticks(0), // Event occurs at the current tick
            EventPriority::minimum(),
            MyEvent::JobArrived { job_id },
        );

        // Returns the interval until the next `fire` method call.
        Some(self.interval)
    }

    // Called each time the specified interval has passed.
    fn fire(
        &mut self,
        ctx: &mut SourceContext<MyEvent, ServerModel>,
        _model: &ServerModel,
    ) -> Option<Duration> {
        // Schedule a new job arrival event.
        let job_id = self.next_job_id;
        self.next_job_id += 1;

        ctx.schedule_event(
            Duration::ticks(0), // Event occurs at the current tick
            EventPriority::minimum(),
            MyEvent::JobArrived { job_id },
        );

        // Returns the interval until the next `fire` method call.
        Some(self.interval)
    }
}

fn main() {
    // Set appropriate log level and output format
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

    // 1. Initialize the simulation engine
    let mut engine = Engine::new();

    // 2. Register hooks and sources
    // TraceHook is a hook that outputs simulation event logs.
    // If tracing is not needed, TraceHook and the ModelSummary trait for output are not required.
    engine.add_hook(TraceHook)
        .add_source(
            "Job Generator",
            JobGenerator { // JobGenerator is a source that periodically generates events
                next_job_id: 0,
                interval: Duration::ticks(5), // Generate a job every 5 ticks
            },
        );

    // 3. Define the simulation model
    let model = ServerModel { // ServerModel represents the system to be simulated
        name: "Sample Server",
        queue: Default::default(),
        is_busy: false,
    };

    // 4. Create a runner and execute the simulation
    // StandardRunner is a standard runner that skips idle time and executes simulations quickly.
    // run_do_ticks(engine, model, simulation_time_in_ticks, enable_logging)
    let mut runner = StandardRunner::new(false); // Logging disabled
    let result = runner.run_do_ticks(engine, model, 60, false); // Run for 60 ticks

    println!("\nSimulation Result: {:?}", result);
}
```

Running the above source code will produce output similar to the following:
<details><summary>Sample Output</summary>

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

## Discrete Event Systems and Features of this Crate

A Discrete Event System is a system where events fire at discrete timings as time progresses, and these events are
processed.

Unlike process-oriented simulators such as [SimPy](https://simpy.readthedocs.io/en/latest/) in Python, `des-sim` is a *
*classic time-driven simulator**.
In process-oriented systems, a flow like "Process A -> Wait N seconds -> Process B" is described within a single
process (e.g., a generator). In contrast, this crate adopts a method where, as time advances, events that should be
processed at that time are handled, such as "Execute Process A and schedule an event for Process B at N seconds later."
Although the writing style differs, both process-oriented and time-driven approaches can achieve equivalent simulation
functionalities.

## Module Structure

This crate is broadly composed of the following three modules:

### 1. `modeling` Module

This module is primarily used by users to model the simulation target.

- **`event` / `model`**: The basic units of simulation. A minimal simulation can be built using just these.
- **`source`**: Used to schedule events periodically.
- **`hook`**: Used to collect logs and states during simulation execution.
- **`sampler`**: Used to sample random data, such as determining the next event time randomly.

These elements are generally registered with the `Engine` in the `execution` module before the simulation starts.

### 2. `execution` Module

Provides the execution environment for simulations. It consists of the core `Engine` and the `Runner` trait that drives
it.

- **`StandardRunner`**: A standard runner that skips idle time and executes simulations quickly.
- **`RealtimeRunner`**: A runner that executes simulations in sync with real-world time progression.
- **`AsyncRunner`**: A runner that can process events asynchronously.

*If `Model`, `Source`, and `Hook` are implemented deterministically (results do not change with each execution),
simulations using `StandardRunner` and `RealtimeRunner` will also be deterministic, allowing for complete
reproducibility of a simulation run. It is also possible to implement your own `Runner` trait.

### 3. `context` Module

Provides the context for requesting operations from the execution environment, such as scheduling events.
Since `Context` is passed as an argument within the `modeling` module, you can operate the environment simply by calling
the provided methods.

## Contribution

We aim to continuously improve this crate to make it more user-friendly.
If you have any feedback, bug reports, or feature requests, please feel free to open an Issue or Pull Request on GitHub!

## License

This project is licensed under either of

* [MIT license](LICENSE)