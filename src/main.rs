mod analytic;
mod monte_carlo;
mod params;

use crate::analytic::black_scholes;
use crate::monte_carlo::{monte_carlo_call, monte_carlo_call_multithread};
use clap::Parser;
use params::OptionParams;
use std::time::Instant;

fn main() {
    let args = params::Args::parse();

    let p = OptionParams {
        spot: 100.0,
        strike: 100.0,
        time: 1.0,
        rate: 0.05,
        vol: 0.2,
    };

    let (c, put) = black_scholes(&p);
    println!("Black-Scholes:  call = {c:.4}   put = {put:.4}");

    let start = Instant::now();
    let (c, se) = monte_carlo_call(&p, args.paths, 42);
    let single = start.elapsed();
    println!("MC single:      call = {c:.4} ± {se:.4}   ({single:.2?})");

    let start = Instant::now();
    let (c, se) = monte_carlo_call_multithread(&p, args.paths, 42);
    let multi = start.elapsed();
    println!("MC multi:       call = {c:.4} ± {se:.4}   ({multi:.2?})");

    println!(
        "Speedup: {:.2}x",
        single.as_secs_f64() / multi.as_secs_f64()
    );
}
