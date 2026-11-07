use chrono::NaiveDate;
use criterion::measurement::WallTime;
use criterion::{BatchSize, BenchmarkGroup, Criterion, criterion_group, criterion_main};
use quantlab::bonds::{Bond, BondFrequency};
use std::hint::black_box;

const CACHE_CAPACITY: u64 = 16;
const Y: f64 = 0.05;

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn with_mode(bond: Bond, cached: bool) -> Bond {
    if cached {
        bond.with_cache(CACHE_CAPACITY)
    } else {
        bond
    }
}

fn short_bond(issue: NaiveDate, cached: bool) -> Bond {
    with_mode(
        Bond::frequent(issue, 100.0, d(2030, 6, 15), BondFrequency::Annual, 0.05),
        cached,
    )
}

fn long_bond(issue: NaiveDate, cached: bool) -> Bond {
    with_mode(
        Bond::frequent(
            issue,
            100.0,
            d(2055, 6, 15),
            BondFrequency::SemiAnnual,
            0.06,
        ),
        cached,
    )
}

fn huge_bond(issue: NaiveDate, cached: bool) -> Bond {
    with_mode(
        Bond::frequent(
            issue,
            100.0,
            d(7025, 6, 15),
            BondFrequency::SemiAnnual,
            0.06,
        ),
        cached,
    )
}

/// Fills the cache for the given settlement dates so that warm benches
/// never include a cache miss. For a bond without a cache this is a no-op
/// apart from a few extra price calls.
fn warm(bond: Bond, settlements: &[NaiveDate]) -> Bond {
    for &s in settlements {
        black_box(bond.price(Y, s).unwrap());
    }
    bond
}

/// Warm benches: the Bond is built once, every iteration reuses it.
/// With a cache the cash flows are looked up; without one they are rebuilt
/// on every call.
fn bench_warm(
    g: &mut BenchmarkGroup<WallTime>,
    short: &Bond,
    long: &Bond,
    huge: &Bond,
    s: NaiveDate,
    mid: NaiveDate,
) {
    g.bench_function("price, 5y annual", |b| {
        b.iter(|| black_box(short).price(black_box(Y), black_box(s)))
    });
    g.bench_function("price, 30y semi-annual", |b| {
        b.iter(|| black_box(long).price(black_box(Y), black_box(s)))
    });
    g.bench_function("price, 30y semi-annual, mid-life settlement", |b| {
        b.iter(|| black_box(long).price(black_box(Y), black_box(mid)))
    });

    for (name, bond) in [
        ("5y annual", short),
        ("30y semi-annual", long),
        ("5000y semi-annual", huge),
    ] {
        let p = bond.price(0.07, s).unwrap();
        g.bench_function(format!("ytm (bisection), {name}"), |b| {
            b.iter(|| black_box(bond).ytm_bisection(black_box(p), black_box(s)))
        });
        g.bench_function(format!("ytm (newton), {name}"), |b| {
            b.iter(|| black_box(bond).ytm(black_box(p), black_box(s)))
        });
    }

    g.bench_function("modified duration, 30y", |b| {
        b.iter(|| black_box(long).modified_duration(black_box(Y), black_box(s)))
    });
    g.bench_function("convexity, 30y", |b| {
        b.iter(|| black_box(long).convexity(black_box(Y), black_box(s)))
    });
    g.bench_function("analytics (one pass), 30y", |b| {
        b.iter(|| black_box(long).analytics(black_box(Y), black_box(s)))
    });

    let shifts = [-300.0, -100.0, -10.0, 10.0, 100.0, 300.0];
    g.bench_function("shift table, 6 shifts, 30y", |b| {
        b.iter(|| black_box(long).shift_table(black_box(Y), black_box(&shifts), black_box(s)))
    });
}

/// Cold benches: a fresh Bond for every measured call, so a cache (if enabled)
/// is empty and the first call is always a miss. Setup and drop of the Bond
/// are not part of the measurement (iter_batched_ref).
fn bench_cold(c: &mut Criterion, issue: NaiveDate, s: NaiveDate) {
    let mut g = c.benchmark_group("cold");

    let p5 = short_bond(issue, false).price(0.07, s).unwrap();
    let p30 = long_bond(issue, false).price(0.07, s).unwrap();

    for cached in [false, true] {
        let mode = if cached { "cache miss" } else { "no cache" };

        g.bench_function(format!("price, 5y annual, {mode}"), |b| {
            b.iter_batched_ref(
                || short_bond(issue, cached),
                |bond| bond.price(black_box(Y), black_box(s)),
                BatchSize::SmallInput,
            )
        });
        g.bench_function(format!("price, 30y semi-annual, {mode}"), |b| {
            b.iter_batched_ref(
                || long_bond(issue, cached),
                |bond| bond.price(black_box(Y), black_box(s)),
                BatchSize::SmallInput,
            )
        });
        g.bench_function(format!("ytm (newton), 5y annual, {mode}"), |b| {
            b.iter_batched_ref(
                || short_bond(issue, cached),
                |bond| bond.ytm(black_box(p5), black_box(s)),
                BatchSize::SmallInput,
            )
        });
        g.bench_function(format!("ytm (newton), 30y semi-annual, {mode}"), |b| {
            b.iter_batched_ref(
                || long_bond(issue, cached),
                |bond| bond.ytm(black_box(p30), black_box(s)),
                BatchSize::SmallInput,
            )
        });

        // Whole lifecycle in the timed section: construct, one call, drop.
        // This is where the cost of creating and dropping a cache shows up.
        g.bench_function(
            format!("construct + price + drop, 5y annual, {mode}"),
            |b| b.iter(|| short_bond(black_box(issue), cached).price(black_box(Y), black_box(s))),
        );
        g.bench_function(
            format!("construct + price + drop, 30y semi-annual, {mode}"),
            |b| b.iter(|| long_bond(black_box(issue), cached).price(black_box(Y), black_box(s))),
        );
    }

    g.finish();
}

fn bench_bonds(c: &mut Criterion) {
    let issue = d(2025, 6, 15);
    let s = issue;
    let mid = d(2040, 9, 1); // inside the life of the 30y bond only

    // Independent of the cache: schedule construction itself.
    {
        let short = short_bond(issue, false);
        let long = long_bond(issue, false);
        c.bench_function("cashflows, 5y annual", |b| {
            b.iter(|| black_box(&short).cashflows())
        });
        c.bench_function("cashflows, 30y semi-annual", |b| {
            b.iter(|| black_box(&long).cashflows())
        });
    }

    // Warm, no cache: cash flows are rebuilt on every call.
    {
        let short = short_bond(issue, false);
        let long = long_bond(issue, false);
        let huge = huge_bond(issue, false);
        let mut g = c.benchmark_group("warm, no cache");
        bench_warm(&mut g, &short, &long, &huge, s, mid);
        g.finish();
    }

    // Warm, cached: cash flows come from the cache after the first call.
    {
        let short = warm(short_bond(issue, true), &[s]);
        let long = warm(long_bond(issue, true), &[s, mid]);
        let huge = warm(huge_bond(issue, true), &[s]);
        let mut g = c.benchmark_group("warm, cached");
        bench_warm(&mut g, &short, &long, &huge, s, mid);
        g.finish();
    }

    // Construction cost alone (a cache is allocated in with_cache).
    {
        let mut g = c.benchmark_group("construct");
        for cached in [false, true] {
            let mode = if cached { "cached" } else { "no cache" };
            g.bench_function(format!("30y semi-annual, {mode}"), |b| {
                b.iter(|| long_bond(black_box(issue), cached))
            });
        }
        g.finish();
    }

    bench_cold(c, issue, s);
}

criterion_group!(benches, bench_bonds);
criterion_main!(benches);
