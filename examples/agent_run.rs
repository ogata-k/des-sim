use des_sim::context::{EventContext, SourceContext, UserContext};
use des_sim::execution::Engine;
use des_sim::execution::runner::Runner;
use des_sim::execution::runner::instance::StandardRunner;
use des_sim::modeling::agent::{AgentActionTicket, AgentContinuation, AgentStep};
use des_sim::modeling::event::{Event, EventPriority};
use des_sim::modeling::hook::Hook;
use des_sim::modeling::hook::instance::{ModelSummary, SharedHook, TraceHook};
use des_sim::modeling::model::Model;
use des_sim::modeling::source::{Source, SourceReadyEntry, SourceView};
use des_sim::primitive::time::{Duration, MicroStep, SimTime, TimeTick};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::rc::Rc;
use std::sync::Mutex;

#[cfg(test)]
mod tests {
    // Check run completeness
    #[test]
    fn example_runs() {
        super::main();
    }
}

pub const CAR_COUNT: usize = 5; // Total number of cars
pub const SAFE_DISTANCE: f64 = 15.0; // Safe following distance (meters)
pub const CAR_SPEED: f64 = 10.0; // Car speed (meters per tick)
pub const INTERSECTION_LIMIT: f64 = -10.0; // Stopping limit line before the intersection (coordinate value)
pub const TICK_INTERVAL: TimeTick = 1; // Normal physical update/peripheral monitoring interval (1 tick)

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SignalColor {
    Green,
    Red,
}

#[derive(Debug)]
pub struct CarState {
    pub id: u64,
    pub current_position: f64, // Negative: Before the intersection, 0.0: At the intersection entrance, Positive: After passing
    pub is_stopped: bool,
}

#[derive(Debug)]
pub struct TrafficModel {
    pub signal: SignalColor,
    pub cars: HashMap<u64, CarState>,
    pub lane: VecDeque<u64>, // Vehicle convoy on the road (IDs are stored in order from the front [0])
}

impl Default for TrafficModel {
    fn default() -> Self {
        Self::new()
    }
}

impl TrafficModel {
    pub fn new() -> Self {
        Self {
            signal: SignalColor::Green,
            cars: HashMap::new(),
            lane: VecDeque::new(),
        }
    }

    /// Get the status of the car directly in front
    pub fn get_front_car(&self, my_id: u64) -> Option<&CarState> {
        let my_index = self.lane.iter().position(|&id| id == my_id)?;
        if my_index == 0 {
            None // I am the lead vehicle
        } else {
            let front_car_id = self.lane[my_index - 1];
            self.cars.get(&front_car_id)
        }
    }
}

// Summary for TraceHook
impl ModelSummary for TrafficModel {
    fn summary(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[Signal: {:?}] Lane -> ", self.signal)?;
        let car_strings: Vec<String> = self
            .lane
            .iter()
            .filter_map(|id| {
                self.cars.get(id).map(|c| {
                    format!(
                        "#{} ({:.1}m, {})",
                        c.id,
                        c.current_position,
                        if c.is_stopped { "Stop" } else { "Run" }
                    )
                })
            })
            .collect();
        if car_strings.is_empty() {
            write!(f, "Empty")
        } else {
            write!(f, "{}", car_strings.join(", "))
        }
    }
}

#[derive(Debug)]
pub enum MyEvent {
    ToggleSignal,
    SpawnCar {
        car_id: u64,
    },
    Resume {
        car_id: u64,
        ticket: AgentActionTicket<MyEvent, TrafficModel>,
    },
}

impl MyEvent {
    pub fn peek_tag(&self) -> Option<&'static str> {
        match self {
            MyEvent::Resume { ticket, .. } => ticket.inspect(|c| c.peek_next_step_tag()).flatten(),
            _ => None,
        }
    }
}

const TOGGLE_TICK_INTERVAL: TimeTick = 15;
pub struct ToggleSignalSource;
impl Source<MyEvent, TrafficModel> for ToggleSignalSource {
    fn on_registered(
        &mut self,
        context: &mut dyn UserContext<MyEvent, TrafficModel>,
        _model: &TrafficModel,
    ) -> Option<Duration> {
        context.schedule_event(
            Duration::ticks(TOGGLE_TICK_INTERVAL),
            EventPriority::minimum(),
            MyEvent::ToggleSignal,
        );

        Some(Duration::ticks(TOGGLE_TICK_INTERVAL))
    }

    fn fire(
        &mut self,
        context: &mut SourceContext<MyEvent, TrafficModel>,
        _model: &TrafficModel,
    ) -> Option<Duration> {
        context.schedule_event(
            Duration::ticks(TOGGLE_TICK_INTERVAL),
            EventPriority::minimum(),
            MyEvent::ToggleSignal,
        );
        Some(Duration::ticks(TOGGLE_TICK_INTERVAL))
    }
}

impl Model<MyEvent> for TrafficModel {
    fn handle_event(&mut self, context: &mut EventContext<MyEvent, Self>, event: &Event<MyEvent>) {
        match &event.payload {
            // Signal switching (rewrites environment data and reserves next event)
            MyEvent::ToggleSignal => {
                self.signal = match self.signal {
                    SignalColor::Green => SignalColor::Red,
                    SignalColor::Red => SignalColor::Green,
                };
            }

            // Car arrives in lane
            MyEvent::SpawnCar { car_id } => {
                self.cars.insert(
                    *car_id,
                    CarState {
                        id: *car_id,
                        current_position: -100.0,
                        is_stopped: false,
                    },
                );
                self.lane.push_back(*car_id);
                context.schedule_event(
                    Duration::zero(),
                    EventPriority::minimum(),
                    create_car_scenario(*car_id, Duration::ticks(5)),
                )
            }

            // Agent autonomous scenario resume progression
            MyEvent::Resume {
                car_id: _car_id,
                ticket,
            } => {
                if let Some(continuation) = ticket.execute() {
                    continuation.execute_and_schedule(context, self);
                }
            }
        }
    }
}

/// Instructions from the agent to the infrastructure on how to reserve the next step
#[derive(Debug)]
enum SchedulingDirective {
    /// "Evenly spaced advance/monitoring": During normal driving or maintaining a stop without any change. Wake yourself up again after a fixed time (1 tick).
    ScheduleNextTick,
    /// "Microstep interrupt": The situation has just started to move (green light/forward clear).
    /// Keep the time as it is (0 tick delay) and rearrange it to the front row as the "next minimum step" in the event queue.
    InterruptImmediate,
    /// "Physical time consumption": For actions such as "crossing an intersection", for a specified time (3 ticks),
    /// Instruction to do nothing and spend time (wait) (completely emulates SimPy's yield env.timeout(3))
    SpendPhysicalTime(Duration),
}
pub fn create_car_scenario(car_id: u64, start_delay: Duration) -> MyEvent {
    let continuation = AgentContinuation::new(move |c| MyEvent::Resume {
        car_id,
        ticket: AgentActionTicket::issue(c),
    })
    // 🔹 Start of the approach loop (set the first delay and put it on the timeline)
    .then_after(
        "approach_loop",
        start_delay,
        EventPriority::minimum(),
        move |context, model, future_steps| {
            dispatch_smooth_approach(car_id, context, model, future_steps);
        },
    )
    .then_after(
        "passing_executed",
        Duration::ticks(1),
        EventPriority::minimum(),
        move |_context, model, _future_steps| {
            // Once the approach process is complete, remove yourself from the road convoy (management queue).
            model.lane.retain(|&id| id != car_id);
        },
    );

    MyEvent::Resume {
        car_id,
        ticket: AgentActionTicket::issue(continuation),
    }
}

/// A dispatcher that receives agent instructions as data and reweaves the timeline with appropriate delays and tags.
fn dispatch_smooth_approach(
    car_id: u64,
    _context: &mut EventContext<MyEvent, TrafficModel>,
    model: &mut TrafficModel,
    future_steps: &mut VecDeque<AgentStep<MyEvent, TrafficModel>>,
) {
    let mut next_tag = "approach_loop";

    // Calls the physical evaluation of the domain layer and asks for "scheduling instructions" from the agent
    let delay = match evaluate_approach_step(car_id, model) {
        SchedulingDirective::ScheduleNextTick => Duration::ticks(TICK_INTERVAL),
        SchedulingDirective::InterruptImmediate => {
            Duration::zero() // 0 tick delay (microstep interrupt)
        }
        SchedulingDirective::SpendPhysicalTime(duration) => {
            // Exactly the same experience as SimPy's `yield env.timeout(3)`.
            // Set a delay of the specified time (3 ticks) and advance the task (state) when it wakes up to "passing processing".
            println!(
                "  ⏳ The car {} spends {} ticks from here to physically cross the intersection",
                car_id,
                duration.as_time_tick()
            );
            next_tag = "passing_execution";
            duration
        }
    };

    // Repositions itself at the top of the future schedule by setting the indicated delay and next state (tag)
    future_steps.push_front(AgentStep {
        tag: next_tag,
        delay,
        priority: EventPriority::minimum(),
        logic: Box::new(move |context, model, future_steps| {
            // When you wake up in the future timeline, if you have reached the "transit execution state", confirm the passage.
            if next_tag == "passing_execution" {
                execute_passing_step(car_id, model, future_steps);
            } else {
                // If still approaching, continue approach loop
                dispatch_smooth_approach(car_id, context, model, future_steps);
            }
        }),
    });
}

/// Autonomous physical location update of agent & evaluation of surrounding environment
fn evaluate_approach_step(car_id: u64, model: &mut TrafficModel) -> SchedulingDirective {
    // Scan the surrounding environment (car in front) and calculate the limit line (target_limit) to proceed.
    let front_car_info = model
        .get_front_car(car_id)
        .map(|f| (f.id, f.current_position, f.is_stopped));
    let mut target_limit = INTERSECTION_LIMIT;

    if let Some((_, front_pos, _)) = front_car_info {
        let safe_stop_pos = front_pos - SAFE_DISTANCE;
        if safe_stop_pos > target_limit {
            target_limit = safe_stop_pos; // If the front is congested, that is the limit.
        }
    }

    if let Some(car) = model.cars.get_mut(&car_id) {
        // [Autonomous recovery judgment for stopped agents]
        if car.is_stopped {
            // Waiting at a red light at the stop line: Detects if the signal has turned green.
            if car.current_position == INTERSECTION_LIMIT && model.signal == SignalColor::Green {
                car.is_stopped = false;
                println!(
                    "  🟢 [Autonomous Start] Car {} detected a change to green light (Pos: {:.1})",
                    car_id, car.current_position
                );
                return SchedulingDirective::InterruptImmediate; // ⚡ Immediately evaluate the next move with 0 ticks
            }

            // Chain-reaction stop due to traffic congestion ahead: Detects if the car in front has moved and a safe following distance has opened up.
            let current_distance = front_car_info
                .map(|(_, f_pos, _)| f_pos - car.current_position)
                .unwrap_or(f64::MAX);
            if current_distance > SAFE_DISTANCE + 1.5 {
                car.is_stopped = false;
                println!(
                    "  🚗 [Autonomous Follow-up Resumed] Car {} detected an opening in the space ahead (Pos: {:.1})",
                    car_id, car.current_position
                );
                return SchedulingDirective::InterruptImmediate; // ⚡ Similarly, restart immediately with 0 ticks
            }

            return SchedulingDirective::ScheduleNextTick;
        }

        // 3. Physical forward simulation during cruising (moving from negative coordinates towards intersection 0.0)
        let next_pos = car.current_position + CAR_SPEED;

        // 4. Check if the hypothetical advanced position exceeds the limit line (value becomes greater than or equal to the limit)
        if next_pos >= target_limit {
            car.current_position = target_limit;

            // [Red light stop] If the limit line is the "stop line" and the signal is red, stop autonomously at that spot.
            if target_limit == INTERSECTION_LIMIT && model.signal == SignalColor::Red {
                car.is_stopped = true;
                println!(
                    "  🚥 [Red Light Detected] Car {} reached the stop line. Stopping autonomously due to red light (Pos: {:.1})",
                    car_id, car.current_position
                );
                return SchedulingDirective::ScheduleNextTick;
            }

            // [Traffic jam stop] If the limit line is the "car in front" and the car in front is stopped, stop in a chain reaction.
            if let Some((front_id, front_pos, true)) = front_car_info
                && (front_pos - car.current_position).abs() <= SAFE_DISTANCE + 1.5
            {
                car.is_stopped = true;
                println!(
                    "  🍁 [Traffic Jam Detected] Car {} reached behind car {}. Stopping in a chain reaction (Pos: {:.1})",
                    car_id, front_id, car.current_position
                );
                return SchedulingDirective::ScheduleNextTick;
            }

            // If the limit line is reached, but the signal is green or the car in front is moving, clear the stop line!
            println!(
                "  🏁 Car {} cleared the stop line! Starting to cross the intersection (Pos: {:.1})",
                car_id, car.current_position
            );

            // Here, instruct the infrastructure to "spend 3 ticks of physical time to cross".
            SchedulingDirective::SpendPhysicalTime(Duration::ticks(3))
        } else {
            // 5. Still cruising at constant speed before the limit line. Proceeding straight regardless of signal color.
            car.current_position = next_pos;
            println!(
                "  🚘 Car {} is moving towards the intersection... (Pos: {:.1} -> Expected stop line: {:.1})",
                car_id, car.current_position, target_limit
            );
            SchedulingDirective::ScheduleNextTick
        }
    } else {
        SchedulingDirective::ScheduleNextTick
    }
}

/// Action to confirm physical passage through an intersection
fn execute_passing_step(
    car_id: u64,
    model: &mut TrafficModel,
    future_steps: &mut VecDeque<AgentStep<MyEvent, TrafficModel>>,
) {
    if let Some(car) = model.cars.get_mut(&car_id) {
        // [Last physical safety guard] If the signal is red at the very moment of passing, emergency stop.
        if model.signal == SignalColor::Red {
            car.is_stopped = true;
            println!(
                "  🚨 [Red Light Just Before Passing] Car {} emergency stopped just before the intersection due to red light (Pos: {:.1})",
                car_id, car.current_position
            );

            // Since the car is still within the intersection, to prevent it from disappearing,
            // pull it back into the approach loop (signal waiting judgment) with a 0-tick delay.
            future_steps.push_front(AgentStep {
                tag: "approach_loop",
                delay: Duration::zero(), // Immediately re-queue
                priority: EventPriority::minimum(),
                logic: Box::new(move |context, model, future_steps| {
                    dispatch_smooth_approach(car_id, context, model, future_steps);
                }),
            });
            return;
        }

        car.current_position = 50.0;
        car.is_stopped = false;
        println!(
            "  ✨ Car {} successfully passed the intersection! (Pos: {:.1})",
            car_id, car.current_position
        );
    }
}

/// Collector that gathers car states at each time step
pub struct LaneStateCollector {
    // Stores the aggregated count in a way that allows reverse lookup from TimeTick
    pub collector: Rc<Mutex<Vec<Vec<String>>>>,
}

impl Hook<MyEvent, TrafficModel> for LaneStateCollector {
    fn before_simulation(&self, _model: &TrafficModel) {}

    fn after_simulation(&self, _model: &TrafficModel, _end_tick: SimTime) {}

    fn before_tick(
        &self,
        _model: &TrafficModel,
        _current_tick: SimTime,
        skipped_duration: Duration,
    ) {
        let mut collector = self.collector.lock().unwrap();

        let skip_count = skipped_duration.as_time_tick();

        if skip_count == 0 {
            return;
        }

        // Fill in the skipped duration
        let fill = collector.last().cloned().unwrap_or_default();

        for _ in 0..skip_count {
            collector.push(fill.clone());
        }
    }

    fn after_tick(
        &self,
        model: &TrafficModel,
        _current_tick: SimTime,
        _last_micro_step: MicroStep,
    ) {
        let car_strings: Vec<String> = model
            .lane
            .iter()
            .filter_map(|id| {
                model.cars.get(id).map(|c| {
                    format!(
                        "#{} ({:.1}m, {})",
                        c.id,
                        c.current_position,
                        if c.is_stopped { "Stop" } else { "Run" }
                    )
                })
            })
            .collect();
        // If locking fails, it's okay to panic.
        self.collector.lock().unwrap().push(car_strings);
    }

    fn before_micro_step(
        &self,
        _model: &TrafficModel,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
    ) {
    }

    fn after_micro_step(
        &self,
        _model: &TrafficModel,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
    ) {
    }

    fn on_discard_remain_micro_step(
        &self,
        _model: &TrafficModel,
        _current_tick: SimTime,
        _first_discarded_micro_step: MicroStep,
        _discarded_sources: &[SourceReadyEntry],
        _discarded_events: &[Event<MyEvent>],
    ) {
    }

    fn before_register_source(&self, _model: &TrafficModel, _name: &str) {}

    fn after_register_source(&self, _model: &TrafficModel, _name: &str) {}

    fn before_source_phase(
        &self,
        _model: &TrafficModel,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
    ) {
    }

    fn before_source(
        &self,
        _model: &TrafficModel,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _source_view: &SourceView,
    ) {
    }

    fn after_source(
        &self,
        _model: &TrafficModel,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _source_view: &SourceView,
        _computed_next_fire: Option<SimTime>,
    ) {
    }

    fn cancel_source(
        &self,
        _model: &TrafficModel,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _scheduled_at: SimTime,
        _source_view: &SourceView,
    ) {
    }

    fn discard_source(
        &self,
        _model: &TrafficModel,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _source_view: &SourceView,
    ) {
    }

    fn after_source_phase(
        &self,
        _model: &TrafficModel,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
    ) {
    }

    fn before_event_phase(
        &self,
        _model: &TrafficModel,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
    ) {
    }

    fn before_event(
        &self,
        _model: &TrafficModel,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _event: &Event<MyEvent>,
    ) {
    }

    fn after_event(
        &self,
        _model: &TrafficModel,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _event: &Event<MyEvent>,
    ) {
    }

    fn cancel_event(
        &self,
        _model: &TrafficModel,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _scheduled_at: SimTime,
        _event: &Event<MyEvent>,
    ) {
    }

    fn discard_event(
        &self,
        _model: &TrafficModel,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _event: &Event<MyEvent>,
    ) {
    }

    fn after_event_phase(
        &self,
        _model: &TrafficModel,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
    ) {
    }
}

impl Default for LaneStateCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl LaneStateCollector {
    pub fn new() -> LaneStateCollector {
        LaneStateCollector {
            collector: Rc::new(Mutex::new(Vec::new())),
        }
    }
}

fn main() {
    // To collect more detailed information than just hooks, and to align the granularity for better readability, set the log level to info or higher.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
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

    let model = TrafficModel::new();
    let mut engine = Engine::new();

    // Cars arrive sequentially
    for i in 0..CAR_COUNT {
        // Assume they arrive with a 1-second delay
        engine.schedule_event_at(
            SimTime::from_ticks(i),
            EventPriority::minimum(),
            // Car ID is 1-origin
            MyEvent::SpawnCar {
                car_id: i as u64 + 1,
            },
        );
    }

    let lane_state_collector = SharedHook::new(LaneStateCollector::new());
    engine
        .add_hook(TraceHook)
        .add_shared_hook(lane_state_collector.clone())
        // At 0 ticks: signal is Green. Set to turn Red at TOGGLE_TICK_INTERVAL ticks.
        .add_source("toggle signal", ToggleSignalSource);

    let mut runner = StandardRunner::new(true);

    println!("=== Starting multi-car traffic jam simulation ===");
    let result = runner.run(engine, model, |model, _, tick_status| {
        // End if cars are gone after a certain period (10 ticks)
        tick_status.is_done_ticks(false, 10) && model.lane.is_empty()
    });
    println!("=== Simulation finished ===");
    println!("Result: {:?}", result);
    println!(
        "Lane state at the end of each time step:\n{}",
        lane_state_collector
            .get_ref()
            .collector
            .lock()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(t, c)| format!("time: {:<5}: {}", t, c.join(" ")))
            .collect::<Vec<String>>()
            .join("\n")
    );
}
