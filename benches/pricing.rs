use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use quantlab::analytic::black_scholes;
use quantlab::monte_carlo::{monte_carlo_call, monte_carlo_call_multithread};
use quantlab::params::OptionParams;

fn atm() -> OptionParams {
    OptionParams {
        spot: 100.0,
        strike: 100.0,
        time: 1.0,
        rate: 0.05,
        vol: 0.2,
    }
}

fn bench_black_scholes(c: &mut Criterion) {
    let p = atm();
    c.bench_function("black_scholes", |b| b.iter(|| black_scholes(black_box(&p))));
}

fn bench_monte_carlo(c: &mut Criterion) {
    let p = atm();
    let mut group = c.benchmark_group("monte_carlo");
    group.sample_size(10);

    for paths in [1_000_000usize, 10_000_000] {
        group.throughput(Throughput::Elements(paths as u64));

        group.bench_with_input(BenchmarkId::new("single", paths), &paths, |b, &n| {
            b.iter(|| monte_carlo_call(black_box(&p), n, 42))
        });

        group.bench_with_input(BenchmarkId::new("multi", paths), &paths, |b, &n| {
            b.iter(|| monte_carlo_call_multithread(black_box(&p), n, 42))
        });
    }

    group.finish();
}

criterion_group!(benches, bench_black_scholes, bench_monte_carlo);
criterion_main!(benches);