use crate::modeling::sampler::{DurationSampler, PendingDuration};
use crate::primitive::time::SimTime;
use rand::Rng;

pub struct AggregateBuilder {
    samplers: Vec<Box<dyn DurationSampler>>,
}

impl AggregateBuilder {
    pub fn from_sampler(sampler: impl DurationSampler + 'static) -> Self {
        AggregateBuilder {
            samplers: vec![Box::new(sampler)],
        }
    }

    pub fn add_sampler(mut self, sampler: impl DurationSampler + 'static) -> Self {
        self.samplers.push(Box::new(sampler));
        self
    }

    pub fn build<F>(self, f: F) -> AggregateSampler<F>
    where
        F: FnMut(&mut dyn Rng, SimTime, Vec<f64>) -> f64,
    {
        AggregateSampler {
            samplers: self.samplers,
            f,
        }
    }
}

pub struct AggregateSampler<F>
where
    F: FnMut(&mut dyn Rng, SimTime, Vec<f64>) -> f64,
{
    samplers: Vec<Box<dyn DurationSampler>>,
    f: F,
}

impl<F> DurationSampler for AggregateSampler<F>
where
    F: FnMut(&mut dyn Rng, SimTime, Vec<f64>) -> f64,
{
    fn sample(&mut self, rng: &mut dyn Rng, current_tick: SimTime) -> PendingDuration {
        let mut sampled_list = Vec::new();
        for sampler in &mut self.samplers {
            let sampled = sampler.sample(rng, current_tick);
            sampled_list.push(sampled.raw_value());
        }

        PendingDuration::new((self.f)(rng, current_tick, sampled_list))
    }
}

impl<F> AggregateSampler<F>
where
    F: FnMut(&mut dyn Rng, SimTime, Vec<f64>) -> f64,
{
    pub fn new(samplers: impl IntoIterator<Item = Box<dyn DurationSampler>>, f: F) -> Self {
        let samplers: Vec<_> = samplers.into_iter().collect();

        assert!(!samplers.is_empty());

        AggregateSampler { samplers, f }
    }
}
