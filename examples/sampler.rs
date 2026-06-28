use des_sim::modeling::sampler::instance::{ExponentialSampler, NormalSampler, UniformSampler};
use des_sim::modeling::sampler::{CombinatorExt, DurationSampler};
use des_sim::primitive::time::{Duration, SimTime};
use std::cell::Cell;
use std::rc::Rc;

fn main() {
    // 1. 個別のサンプラーコンポーネントを定義
    let base_task = NormalSampler::new(3.0, 2.0).unwrap();
    let network_delay = ExponentialSampler::new(0.05).unwrap(); // λ=0.05
    let jitter1 = UniformSampler::new(-5.0, 2.0).unwrap();
    let jitter2 = UniformSampler::new(-2.0, 2.0).unwrap();
    let jitter3 = NormalSampler::new(-2.0, 2.0).unwrap();

    // 2. パイプラインの構築
    // サーバー1台分の処理ロジックを定義
    // 各サーバーの違いは受付開始までの揺らぎだけ
    let base_server_logic = base_task
        .with_delay(network_delay) // 遅延を足す
        .map(|_, _, d| d * 1.05); // 内部オーバーヘッドを5%加算
    let server_logic1 = base_server_logic.clone().with_jitter(jitter1); // ゆらぎを加える
    let server_logic2 = base_server_logic.clone().with_jitter(jitter2); // ゆらぎを加える
    let server_logic3 = base_server_logic.clone().with_jitter(jitter3); // ゆらぎを加える

    // 3. 冗長構成（Aggregate）の構築
    // 共有可能な状態を作って記録するためにクローン
    let selected_index = Rc::new(Cell::new(None::<usize>));
    let index_ref = Rc::clone(&selected_index); // クロージャ用にクローン

    // 3つのサーバーを並列で受付開始し、一番早い（min）結果を採用
    let redundant_service = server_logic1 // サーバー1
        .aggregate_builder()
        .add_sampler(server_logic2) // サーバー2
        .add_sampler(server_logic3) // サーバー3
        .build(move |_, _, durations| {
            // インデックス付きで最小値を探索
            let (idx, &val) = durations
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .unwrap();

            // 外側の変数を書き換える
            // Cell を使って値を更新する
            index_ref.set(Some(idx));
            val
        });

    // 4. 最終的な実行用サンプラー
    // 最後に非負チェックを挟む安全なパイプラインの完成
    let mut final_sampler = redundant_service.non_negative(3, |_rng, _now| Duration::ticks(10));

    // 実行
    let mut rng = rand::rng();
    for i in 0..100 {
        let duration = final_sampler.sample(&mut rng, SimTime::zero());
        println!(
            " {:<3}: Final duration: {:?} with use server {}",
            i + 1,
            duration.to_duration(),
            selected_index.get().unwrap()
        );
    }
}
