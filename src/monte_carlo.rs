use crate::params::OptionParams;
use rand::{RngExt, SeedableRng, rngs::StdRng};
use rand_distr::StandardNormal;
use rayon::prelude::*;

fn simulate_chunk(p: &OptionParams, paths: usize, seed: u64) -> (f64, f64) {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut sum = 0.0;
    let mut sum_sq = 0.0;
    let diffusion = p.vol * p.time.sqrt();
    let shift = (p.rate - p.vol.powi(2) / 2.0) * p.time;
    for _ in 0..paths {
        let z: f64 = rng.sample(StandardNormal);
        let s_t = p.spot * (shift + diffusion * z).exp();
        let payoff = (s_t - p.strike).max(0.0);
        sum += payoff;
        sum_sq += payoff * payoff;
    }

    (sum, sum_sq)
}

pub fn monte_carlo_call(p: &OptionParams, paths: usize, seed: u64) -> (f64, f64) {
    let n = paths as f64;
    let discount = (-p.rate * p.time).exp();
    let (sum, sum_sq) = simulate_chunk(p, paths, seed);
    let mean = sum / n;
    let std = (sum_sq / n - mean * mean).sqrt();

    (discount * mean, discount * std / n.sqrt())
}

pub fn monte_carlo_call_multithread(p: &OptionParams, paths: usize, seed: u64) -> (f64, f64) {
    const CHUNK: usize = 65_536;
    let chunks = paths.div_ceil(CHUNK);
    let parts: Vec<(f64, f64)> = (0..chunks)
        .into_par_iter()
        .map(|i| {
            let len = CHUNK.min(paths - i * CHUNK);
            simulate_chunk(p, len, seed.wrapping_add(i as u64))
        })
        .collect();

    let (sum, sum_sq) = parts.iter().fold((0.0, 0.0), |a, b| (a.0 + b.0, a.1 + b.1));
    let n = paths as f64;
    let discount = (-p.rate * p.time).exp();
    let mean = sum / n;
    let std = (sum_sq / n - mean * mean).sqrt();

    (discount * mean, discount * std / n.sqrt())
}
