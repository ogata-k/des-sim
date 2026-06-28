use des_sim::modeling::sampler::instance::{ExponentialSampler, NormalSampler, UniformSampler};
use des_sim::modeling::sampler::{CombinatorExt, DurationSampler};
use des_sim::primitive::time::{Duration, SimTime};
use std::cell::Cell;
use std::rc::Rc;

#[cfg(test)]
mod tests {
    // 最後までサンプルが走りきることをテスト
    #[test]
    fn example_runs() {
        super::main();
    }
}

fn main() {
    // 1. 個別のサンプラーコンポーネントを定義
    let jitter1 = UniformSampler::new(-5.0, 2.0).unwrap();
    let jitter2 = UniformSampler::new(-2.0, 2.0).unwrap();
    let jitter3 = NormalSampler::new(-2.0, 2.0).unwrap();

    fn create_server(jitter: Box<dyn DurationSampler>) -> impl DurationSampler {
        NormalSampler::new(3.0, 2.0)
            .unwrap()
            // λ=0.05
            .delay(ExponentialSampler::new(0.05).unwrap().boxed())
            .map(|_, _, d| d * 1.05)
            // 各サーバーの違いは受付開始までの揺らぎだけ
            .jitter(jitter)
    }

    // 2. 冗長構成（Aggregate）の構築
    // 共有可能な状態を作って記録するためにクローン
    let selected_index = Rc::new(Cell::new(None::<usize>));
    let index_ref = Rc::clone(&selected_index); // クロージャ用にクローン

    // 3つのサーバーを並列で受付開始し、一番早い（min）結果を採用
    let redundant_service = create_server(jitter1.boxed())
        .aggregate_builder()
        .add_sampler(create_server(jitter2.boxed()).boxed())
        .add_sampler(create_server(jitter3.boxed()).boxed())
        .build(move |_, _, durations| {
            let (idx, &val) = durations
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .unwrap();
            index_ref.set(Some(idx));
            val
        });

    // 3. 最終的な実行用サンプラー
    // 最後に非負チェックを挟む安全なパイプラインの完成
    let mut final_sampler =
        redundant_service.ensure_non_negative(3, |_rng, _now| Duration::ticks(5));

    // 4. 実行
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
