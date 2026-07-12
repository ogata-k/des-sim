use des_sim::context::{EventContext, ExecutorStatus, UserContext};
use des_sim::execution::Engine;
use des_sim::execution::runner::Runner;
use des_sim::execution::runner::instance::StandardRunner;
use des_sim::modeling::event::{Event, EventPriority};
use des_sim::modeling::model::Model;
use des_sim::modeling::sampler::DurationSampler;
use des_sim::modeling::sampler::instance::ExponentialSampler;
use des_sim::primitive::time::{Duration, SimTime, TickStatus};
use rand::prelude::*;
use rand::rng;
use std::collections::VecDeque;
use std::sync::Arc;

#[cfg(test)]
mod tests {
    // 最後までサンプルが走りきることをテスト
    #[test]
    fn example_runs() {
        super::main();
    }
}

// =========================================================================
// 1. M/M/1 待ち行列シミュレーション ドメインロジック
// =========================================================================

#[derive(Debug, Clone, Copy)]
pub enum QueueEvent {
    Arrival,
    Departure,
}

#[derive(Debug, Clone)]
struct Customer {
    arrival_time: SimTime,
}

#[derive(Debug, Clone)]
pub enum RngMode {
    Fixed(u64), // シード値を保持
    Random,     // ランダム生成であることを示すフラグ
}

#[derive(Debug)]
pub struct QueueModel {
    queue: VecDeque<Customer>,
    arrival_sampler: ExponentialSampler,
    service_sampler: ExponentialSampler,
    rng: StdRng,
    pub total_stay_duration: Duration,
    pub total_served_customers: usize,
    pub time_weighted_queue_length: f64,
    pub last_event_time: SimTime,
}

impl QueueModel {
    pub fn new(arrival_rate: f64, service_rate: f64, rng_mode: RngMode) -> Self {
        let rng = match rng_mode {
            RngMode::Fixed(seed) => StdRng::seed_from_u64(seed),
            RngMode::Random => StdRng::from_rng(&mut rng()),
        };

        Self {
            queue: VecDeque::new(),
            arrival_sampler: ExponentialSampler::new(arrival_rate).unwrap(),
            service_sampler: ExponentialSampler::new(service_rate).unwrap(),
            rng,
            total_stay_duration: Duration::zero(),
            total_served_customers: 0,
            time_weighted_queue_length: 0.0,
            last_event_time: SimTime::zero(),
        }
    }

    pub fn update_statistics(&mut self, current_time: SimTime) {
        if current_time < self.last_event_time {
            return;
        }
        let delta_t = (current_time - self.last_event_time).as_time_tick() as f64;
        let current_count = self.queue.len() as f64;

        self.time_weighted_queue_length += current_count * delta_t;
        self.last_event_time = current_time;
    }
}

impl Model<QueueEvent> for QueueModel {
    fn handle_event(
        &mut self,
        context: &mut EventContext<QueueEvent, Self>,
        event: &Event<QueueEvent>,
    ) {
        let now = context.current_tick();
        let limit_time = SimTime::from_ticks(200_000);

        if now > limit_time {
            return;
        }

        self.update_statistics(now);

        match event.payload {
            QueueEvent::Arrival => {
                let customer = Customer { arrival_time: now };
                self.queue.push_back(customer);

                if self.queue.len() == 1 {
                    let service_duration = self.service_sampler.sample(&mut self.rng, now);
                    context.schedule_event(
                        service_duration.to_duration(),
                        EventPriority::minimum(),
                        QueueEvent::Departure,
                    );
                }

                let next_arrival_duration = self.arrival_sampler.sample(&mut self.rng, now);
                context.schedule_event(
                    next_arrival_duration.to_duration(),
                    EventPriority::minimum(),
                    QueueEvent::Arrival,
                );
            }
            QueueEvent::Departure => {
                if let Some(served_customer) = self.queue.pop_front() {
                    let stay_duration = now - served_customer.arrival_time;
                    self.total_stay_duration += stay_duration;
                    self.total_served_customers += 1;
                }

                if !self.queue.is_empty() {
                    let service_duration = self.service_sampler.sample(&mut self.rng, now);
                    context.schedule_event(
                        service_duration.to_duration(),
                        EventPriority::minimum(),
                        QueueEvent::Departure,
                    );
                }
            }
        }
    }
}

// =========================================================================
// 2. メイン関数 (並列バッチの実行とテーブル集計)
// =========================================================================

#[allow(unused)]
struct SimulationReport {
    scenario_idx: usize,
    total_customers: usize,
    rho_theoretical: f64,
    lambda_theoretical: f64,
    lambda_sim: f64,
    l_theoretical: f64,
    l_sim: f64,
    w_theoretical: f64,
    w_sim: f64,
    calculated_l_via_little: f64, // λ_sim × W_sim から計算したL
    l_error_percent: f64,
}

fn main() {
    let service_rate = 0.10;
    let arrival_rates = vec![0.02, 0.04, 0.06, 0.08, 0.09];
    let count = arrival_rates.len();
    let simulation_end_ticks = 200_000;

    let arrival_rates_arc = Arc::new(arrival_rates);

    let engine_builder = |_index: usize| {
        let mut engine = Engine::new();
        engine.schedule_event_at(
            SimTime::zero(),
            EventPriority::minimum(),
            QueueEvent::Arrival,
        );
        engine
    };

    let arrival_rates_for_model = Arc::clone(&arrival_rates_arc);
    let model_builder = move |index: usize| {
        let lambda = arrival_rates_for_model[index];
        QueueModel::new(lambda, service_rate, RngMode::Random)
    };

    let should_stop = move |_m: &QueueModel, _status: ExecutorStatus, tick: TickStatus| {
        tick.is_done_ticks(false, simulation_end_ticks)
    };

    let runner = StandardRunner::new(true);

    println!("⚡ {} 件の待ち行列シナリオを並列で実行中...", count);

    let batch_results =
        (&runner).run_batch_parallel(count, engine_builder, model_builder, should_stop);

    let reports: Vec<SimulationReport> = batch_results
        .into_iter()
        .enumerate()
        .map(|(index, result)| {
            assert!(result.is_ok());
            let mut final_result = result.map_err(|e| format!("{:?}", e)).unwrap();
            let final_model = final_result.model_mut();

            let end_time = SimTime::from_ticks(simulation_end_ticks);

            if final_model.last_event_time < end_time {
                final_model.update_statistics(end_time);
            }

            let total_ticks = simulation_end_ticks as f64;
            let lambda_theoretical = arrival_rates_arc[index];
            let rho_theoretical = lambda_theoretical / service_rate;

            // キューに残っている仕掛品の滞在時間を end_time 時点で足し合わせる
            let mut total_stay_ticks = final_model.total_stay_duration.as_time_tick() as f64;
            for customer in &final_model.queue {
                total_stay_ticks += (end_time - customer.arrival_time).as_time_tick() as f64;
            }

            // 総客数 ＝ 退去済みの人数 ＋ 現在残っている人数
            let total_customers = final_model.total_served_customers + final_model.queue.len();

            // 実測値 (Simulation) の計算
            let l_sim = final_model.time_weighted_queue_length / total_ticks;
            let lambda_sim = total_customers as f64 / total_ticks;
            let w_sim = total_stay_ticks / total_customers as f64;

            // リトルの法則検証用: λ_sim × W_sim
            let calculated_l_via_little = lambda_sim * w_sim;
            assert!((l_sim - calculated_l_via_little).abs() < 0.0001);

            // 理論値 (Theoretical) の M/M/1 公式計算
            let l_theoretical = rho_theoretical / (1.0 - rho_theoretical);
            let w_theoretical = l_theoretical / lambda_theoretical;
            let l_error_percent = ((l_sim - l_theoretical).abs() / l_theoretical) * 100.0;

            SimulationReport {
                scenario_idx: index + 1,
                total_customers,
                rho_theoretical,
                lambda_theoretical,
                lambda_sim,
                l_theoretical,
                l_sim,
                w_theoretical,
                w_sim,
                calculated_l_via_little,
                l_error_percent,
            }
        })
        .collect();

    // 5. テーブルテキスト形式でのコンソール出力
    println!(
        "\n========================================================================================================================"
    );
    println!(" 📊 M/M/1 待ち行列 並列バッチシミュレーション結果一覧表 (リトルの法則の検証)");
    println!(
        "========================================================================================================================"
    );
    println!(
        "{:<4} | {:<7} | {:<21} | {:<21} | {:<7} : {:<7} | {:<13} | {:<7}",
        "No",
        "ρ(理論)",
        "λ(理論) : λ(実測)",
        "W(理論) : W(実測)",
        "L(理論)",
        "L(実測)",
        "λ_sim×W_sim",
        "誤差"
    );
    println!(
        "------------------------------------------------------------------------------------------------------------------------"
    );
    for r in reports {
        println!(
            "{:<4} | {:<7.3} | {:<10.4} : {:<9.4} | {:<10.2} : {:<9.2} | {:<7.4} : {:<7.4} | {:<13.4} | {:<6.3}%",
            r.scenario_idx,
            r.rho_theoretical,
            r.lambda_theoretical,
            r.lambda_sim,
            r.w_theoretical,
            r.w_sim,
            r.l_theoretical,
            r.l_sim,
            r.calculated_l_via_little,
            r.l_error_percent
        );
    }
    println!(
        "========================================================================================================================\n"
    );
    println!(
        "💡 結論: 「L(実測)」列と、そのすぐ右隣の「λ_sim×W_sim」列の数値が100%完全に一致していることが分かります。"
    );
    println!(
        "         これにより、今回の有限時間サンプリングにおいてリトルの法則 ($L = \\lambda W$) が完璧に成立することが実証されました。"
    );
}
