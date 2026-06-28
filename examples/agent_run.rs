use des_sim::context::{EventContext, SourceContext};
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
    // 最後までサンプルが走りきることをテスト
    #[test]
    fn example_runs() {
        super::main();
    }
}

// =========================================================================
//  定数と環境・ドメイン状態の定義
// =========================================================================

pub const CAR_COUNT: usize = 5; // 車の数
pub const SAFE_DISTANCE: f64 = 15.0; // 安全車間距離 (メートル)
pub const CAR_SPEED: f64 = 10.0; // 車の速度 (メートル / tick)
pub const INTERSECTION_LIMIT: f64 = -10.0; // 交差点手前の停止限界線 (座標値)
pub const TICK_INTERVAL: TimeTick = 1; // 通常の物理更新・周辺監視の間隔 (1 tick)

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SignalColor {
    Green,
    Red,
}

#[derive(Debug)]
pub struct CarState {
    pub id: u64,
    pub current_position: f64, // 負: 交差点手前, 0.0: 交差点, 正: 通過後
    pub is_stopped: bool,
}

#[derive(Debug)]
pub struct TrafficModel {
    pub signal: SignalColor,
    pub cars: HashMap<u64, CarState>,
    pub lane: VecDeque<u64>, // 道路上の車列（先頭[0]から順にIDを格納）
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

    /// 自分の「すぐ前」を走っている車の状態を安全に取得する（世界へのクエリ）
    pub fn get_front_car(&self, my_id: u64) -> Option<&CarState> {
        let my_index = self.lane.iter().position(|&id| id == my_id)?;
        if my_index == 0 {
            None // 自分が先頭車両
        } else {
            let front_car_id = self.lane[my_index - 1];
            self.cars.get(&front_car_id)
        }
    }
}

// インフラの TraceHook 用サマリー実装
impl ModelSummary for TrafficModel {
    fn summary(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[Signal: {:?}] Lane -> ", self.signal)?;
        let car_strs: Vec<String> = self
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
        if car_strs.is_empty() {
            write!(f, "Empty")
        } else {
            write!(f, "{}", car_strs.join(", "))
        }
    }
}

// =========================================================================
// エージェントのライフサイクル管理 (インフラ・ユーティリティ)
// =========================================================================

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

// =========================================================================
// インフラ・イベントループの実装（環境の変化のみを担当）
// =========================================================================

const TOGGLE_TICK_INTERVAL: TimeTick = 15;
pub struct ToggleSignalSource;
impl Source<MyEvent, TrafficModel> for ToggleSignalSource {
    fn initialize(
        &mut self,
        context: &mut SourceContext<MyEvent, TrafficModel>,
        _model: &TrafficModel,
    ) {
        context.schedule_event(
            Duration::ticks(TOGGLE_TICK_INTERVAL),
            EventPriority::minimum(),
            MyEvent::ToggleSignal,
        );
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
            // 信号切り替え（インフラは環境のデータ書き換えと次回の時間予約のみを行う）
            MyEvent::ToggleSignal => {
                self.signal = match self.signal {
                    SignalColor::Green => SignalColor::Red,
                    SignalColor::Red => SignalColor::Green,
                };
            }

            // 車がレーンに到着
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

            // エージェントの自律シナリオ進行
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

// =========================================================================
//  スケジュール指示ステート & 自律制御ロジック
// =========================================================================

/// エージェントからインフラへの「次ステップの予約方法」の指示書
#[derive(Debug)]
enum SchedulingDirective {
    /// 【等間隔前進/監視】通常走行中、または変化のない停止維持。固定時間（1 tick）の後に再度自分を起こしてくれ。
    ScheduleNextTick,
    /// 【マイクロステップ割り込み】ちょうど状況が動き出した（青信号/前方クリア）。
    /// 時刻はそのまま（0 tick遅延）で、イベントキューの『次の最小ステップ』として最前列に並び直させろ。
    InterruptImmediate,
    /// 【物理時間消費】「交差点を渡る」などのアクションのために、指定された時間（3 tick）だけ、
    /// 何もせずじっと時間を消費（待機）する指示（SimPyの yield env.timeout(3) を完全エミュレート）
    SpendPhysicalTime(Duration),
}

pub fn create_car_scenario(car_id: u64, start_delay: Duration) -> MyEvent {
    let continuation = AgentContinuation::new(move |c| MyEvent::Resume {
        car_id,
        ticket: AgentActionTicket::issue(c),
    })
    // 🔹 接近ループの開始（最初の遅延を設定してタイムラインに乗せる）
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
            // 接近処理が終わったので道路の車列（管理キュー）から自分を除外。
            model.lane.retain(|&id| id != car_id);
        },
    );

    MyEvent::Resume {
        car_id,
        ticket: AgentActionTicket::issue(continuation),
    }
}

/// エージェントの指示をデータとして受け取り、適切な遅延（delay）とタグでタイムラインを編み直すディスパッチャ
fn dispatch_smooth_approach(
    car_id: u64,
    _context: &mut EventContext<MyEvent, TrafficModel>,
    model: &mut TrafficModel,
    future_steps: &mut VecDeque<AgentStep<MyEvent, TrafficModel>>,
) {
    let mut next_tag = "approach_loop";

    // ドメイン層の物理評価を呼び出し、エージェントからの「スケジューリング指示」を仰ぐ
    let delay = match evaluate_approach_step(car_id, model) {
        SchedulingDirective::ScheduleNextTick => Duration::ticks(TICK_INTERVAL),
        SchedulingDirective::InterruptImmediate => {
            Duration::zero() // ⚡ 0 tick 遅延（マイクロステップ割り込み）
        }
        SchedulingDirective::SpendPhysicalTime(duration) => {
            // SimPyの `yield env.timeout(3)` と全く同じ体験。
            // 指定された時間（3 tick）の遅延を設定し、目覚めた時のタスク（状態）を「通過処理」へ進める。
            println!(
                "  ⏳ 車 {} は交差点を物理的に渡るため、ここから {} tick 分、時間を消費します",
                car_id,
                duration.as_ticks()
            );
            next_tag = "passing_execution";
            duration
        }
    };

    // 指示された delay と次状態（tag）を設定して、自分自身を未来の予定の先頭に積み直す
    future_steps.push_front(AgentStep {
        tag: next_tag,
        delay,
        priority: EventPriority::minimum(),
        logic: Box::new(move |context, model, future_steps| {
            // 未来の時間軸で目覚めたとき、自分が「通過実行ステート」に達していれば、通過を確定させる
            if next_tag == "passing_execution" {
                execute_passing_step(car_id, model, future_steps);
            } else {
                // まだ接近中であれば、アプローチのループを継続
                dispatch_smooth_approach(car_id, context, model, future_steps);
            }
        }),
    });
}

/// 【ドメイン層】エージェントの自律的な物理位置更新 ＆ 周辺環境の評価
fn evaluate_approach_step(car_id: u64, model: &mut TrafficModel) -> SchedulingDirective {
    // 周囲の環境（前方車両）をスキャンし、進める限界ライン（target_limit）を算出
    let front_car_info = model
        .get_front_car(car_id)
        .map(|f| (f.id, f.current_position, f.is_stopped));
    let mut target_limit = INTERSECTION_LIMIT;

    if let Some((_, front_pos, _)) = front_car_info {
        let safe_stop_pos = front_pos - SAFE_DISTANCE;
        if safe_stop_pos > target_limit {
            target_limit = safe_stop_pos; // 前方が詰まっているならそこが限界
        }
    }

    if let Some(car) = model.cars.get_mut(&car_id) {
        // 【停止中のエージェントの自律復帰判定】
        if car.is_stopped {
            // 停止線で赤信号待ち：信号が緑になったかを自分で検知
            if car.current_position == INTERSECTION_LIMIT && model.signal == SignalColor::Green {
                car.is_stopped = false;
                println!(
                    "  🟢 [自律発進] 車 {} が青信号への変化を検知しました (Pos: {:.1})",
                    car_id, car.current_position
                );
                return SchedulingDirective::InterruptImmediate; // ⚡ 0 tick で即座に次の一手を評価
            }

            // 前方の渋滞で数珠繋ぎ停止：前の車が動いて安全車間距離以上に空間が開いたかを自分で検知
            let current_distance = front_car_info
                .map(|(_, f_pos, _)| f_pos - car.current_position)
                .unwrap_or(f64::MAX);
            if current_distance > SAFE_DISTANCE + 1.5 {
                car.is_stopped = false;
                println!(
                    "  🚗 [自律追従再開] 車 {} が前方空間の広がりを検知しました (Pos: {:.1})",
                    car_id, car.current_position
                );
                return SchedulingDirective::InterruptImmediate; // ⚡ 同様に 0 tick で即座に再起動
            }

            return SchedulingDirective::ScheduleNextTick;
        }

        // 3. 巡航中の物理前進シミュレーション（負の座標系から交差点 0.0 へ向かって進む）
        let next_pos = car.current_position + CAR_SPEED;

        // 4. 進んだ仮の先が、限界ラインを超えてしまう（値が制限以上になる）かチェック
        if next_pos >= target_limit {
            car.current_position = target_limit;

            // 【赤信号停止】限界ラインが「停止線」であり、かつ環境が赤信号なら、自律的にその場で停止
            if target_limit == INTERSECTION_LIMIT && model.signal == SignalColor::Red {
                car.is_stopped = true;
                println!(
                    "  🚥 [赤信号認知] 車 {} が停止線に到達。信号が赤のため自律停止します (Pos: {:.1})",
                    car_id, car.current_position
                );
                return SchedulingDirective::ScheduleNextTick;
            }

            // 【渋滞停止】限界ラインが「前の車」かつ前の車が止まっているなら、数珠繋ぎ停止
            if let Some((front_id, front_pos, true)) = front_car_info
                && (front_pos - car.current_position).abs() <= SAFE_DISTANCE + 1.5
            {
                car.is_stopped = true;
                println!(
                    "  🍁 [渋滞認知] 車 {} が前方の車 {} の後ろに到達。数珠繋ぎ停止します (Pos: {:.1})",
                    car_id, front_id, car.current_position
                );
                return SchedulingDirective::ScheduleNextTick;
            }

            // 限界ラインに達したが、青信号であるか前の車が動いているなら、停止線をクリア！
            println!(
                "  🏁 車 {} が停止線をクリア！交差点の横断を開始します (Pos: {:.1})",
                car_id, car.current_position
            );

            // ここで「物理時間を 3 tick 消費して渡れ」という命令をインフラに指示する
            SchedulingDirective::SpendPhysicalTime(Duration::ticks(3))
        } else {
            // 5. まだ限界ラインよりも手前を等速巡航中。信号の色に関係なくひたすら前進
            car.current_position = next_pos;
            println!(
                "  🚘 車 {} が交差点に向かって前進中... (Pos: {:.1} -> 停止予定線: {:.1})",
                car_id, car.current_position, target_limit
            );
            SchedulingDirective::ScheduleNextTick
        }
    } else {
        SchedulingDirective::ScheduleNextTick
    }
}

/// 【ドメイン層】交差点の物理通過確定アクション
fn execute_passing_step(
    car_id: u64,
    model: &mut TrafficModel,
    future_steps: &mut VecDeque<AgentStep<MyEvent, TrafficModel>>,
) {
    if let Some(car) = model.cars.get_mut(&car_id) {
        // 【最後の物理安全ガード】通過するまさにその瞬間に、万が一赤信号なら緊急停止
        if model.signal == SignalColor::Red {
            car.is_stopped = true;
            println!(
                "  🚨 [通過直前赤信号] 車 {} は赤信号のため、交差点直前で緊急停止しました (Pos: {:.1})",
                car_id, car.current_position
            );

            // 車はまだ交差点内なので消滅しないよう、0 tick遅延で接近ループ（信号待ち判定）に引き戻す
            future_steps.push_front(AgentStep {
                tag: "approach_loop",
                delay: Duration::zero(), // 即座に並び直す
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
            "  ✨ 車 {} が交差点を無事に通過しました！ (Pos: {:.1})",
            car_id, car.current_position
        );
    }
}

// =========================================================================
// 集計
// =========================================================================

/// その時刻の車の状態を集めるコレクター
pub struct LaneStateCollector {
    // TimeTickから集計数を逆引きできる形で持っておく
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

        let skip_count = skipped_duration.as_ticks();

        if skip_count == 0 {
            return;
        }

        // スキップされた分を補填
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
        let car_strs: Vec<String> = model
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
        // ロックに失敗した場合はおとなしくパニックしてもらう
        self.collector.lock().unwrap().push(car_strs);
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

    fn before_initialize_source(&self, _model: &TrafficModel, _name: &str) {}

    fn after_initialize_source(&self, _model: &TrafficModel, _name: &str) {}

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

// =========================================================================
// シミュレーション
// =========================================================================

fn main() {
    // Hook以外に詳細な情報を集めるためにprintしているので、見やすくして粒度を合わせるためにinfo以上にする。
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

    // 車が順に到着してくる
    for i in 0..CAR_COUNT {
        // １秒遅れで到着するとする
        engine.schedule_event_at(
            SimTime::new(i),
            EventPriority::minimum(),
            // 車のIDは1-origin
            MyEvent::SpawnCar {
                car_id: i as u64 + 1,
            },
        );
    }

    let lane_state_collector = SharedHook::new(LaneStateCollector::new());
    engine
        .add_hook(TraceHook)
        .add_shared_hook(lane_state_collector.clone())
        // 0 tick 時点：信号はGreen。TOGGLE_TICK_INTERVAL tick 目にRedになるようにセット
        .add_source(
            "toggle signal",
            SimTime::new(TOGGLE_TICK_INTERVAL),
            ToggleSignalSource,
        );

    let mut runner = StandardRunner::new(true);

    println!("=== 複数台 渋滞シミュレーション開始 ===");
    let result = runner.run(engine, model, |model, _, tick_status| {
        // ある程度（10 ticks）進んだ状態で車がいなくなっていたら終了
        tick_status.is_done_ticks(false, 10) && model.lane.is_empty()
    });
    println!("=== シミュレーション終了 ===");
    println!("結果：{:?}", result);
    println!(
        "各時間終了時のレーン状態：\n{}",
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
