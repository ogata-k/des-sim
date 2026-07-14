use des_sim::modeling::sampler::instance::{ExponentialSampler, NormalSampler, UniformSampler};
use des_sim::modeling::sampler::{CombinatorExt, DurationSampler};
use des_sim::primitive::time::{Duration, SimTime};
use std::cell::Cell;
use std::rc::Rc;

#[cfg(test)]
mod tests {
    // Verifies that the sample simulation completes execution successfully.
    #[test]
    fn example_runs() {
        super::main();
    }
}

fn main() {
    // Define individual sampler components.
    let jitter1 = UniformSampler::new(-5.0, 2.0).unwrap();
    let jitter2 = UniformSampler::new(-2.0, 2.0).unwrap();
    let jitter3 = NormalSampler::new(-2.0, 2.0).unwrap();

    // Helper to create a server model with specific jitter characteristics.
    fn create_server(jitter: Box<dyn DurationSampler>) -> impl DurationSampler {
        NormalSampler::new(3.0, 2.0)
            .unwrap()
            // Lambda = 0.05
            .delay(ExponentialSampler::new(0.05).unwrap().boxed())
            .map(|_, _, d| d * 1.05)
            // Server differentiation based on start-up jitter.
            .jitter(jitter)
    }

    // Build the redundant configuration (Aggregate).
    // Shared state used to track which server provided the fastest response.
    let selected_index = Rc::new(Cell::new(None::<usize>));
    let index_ref = Rc::clone(&selected_index);

    // Aggregate three servers in parallel, selecting the minimum duration result.
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

    // Final execution sampler: ensures non-negative results with a fallback floor of 5 ticks.
    let mut final_sampler =
        redundant_service.ensure_non_negative(3, |_rng, _now| Duration::ticks(5));

    // Execution loop.
    let mut rng = rand::rng();
    for i in 0..100 {
        let duration = final_sampler.sample(&mut rng, SimTime::zero());
        println!(
            " {:<3}: Final duration: {:?} | Selected server index: {}",
            i + 1,
            duration.to_duration(),
            selected_index.get().unwrap()
        );
    }
}
