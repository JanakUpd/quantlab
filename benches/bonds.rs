use chrono::NaiveDate;
use criterion::{Criterion, criterion_group, criterion_main};
use quantlab::bonds::{Bond, BondFrequency};
use std::hint::black_box;

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn bench_bonds(c: &mut Criterion) {
    let issue = d(2025, 6, 15);
    let short = Bond::frequent(issue, 100.0, d(2030, 6, 15), BondFrequency::Annual, 0.05);
    let long = Bond::frequent(
        issue,
        100.0,
        d(2055, 6, 15),
        BondFrequency::SemiAnnual,
        0.06,
    );
    let longlong = Bond::frequent(
        issue,
        100.0,
        d(7025, 6, 15),
        BondFrequency::SemiAnnual,
        0.06,
    );
    let y = 0.05;

    c.bench_function("cashflows, 5y annual", |b| {
        b.iter(|| black_box(&short).cashflows())
    });
    c.bench_function("cashflows, 30y semi-annual", |b| {
        b.iter(|| black_box(&long).cashflows())
    });

    c.bench_function("price, 5y annual", |b| {
        b.iter(|| black_box(&short).price(black_box(y)))
    });
    c.bench_function("price, 30y semi-annual", |b| {
        b.iter(|| black_box(&long).price(black_box(y)))
    });

    let bonds = [
        ("5y annual", &short),
        ("30y semi-annual", &long),
        ("5000y semi-annual", &longlong),
    ];
    for (name, bond) in bonds {
        let p = bond.price(0.07);
        c.bench_function(&format!("ytm (bisection), {name}"), |b| {
            b.iter(|| black_box(bond).ytm_bisection(black_box(p)))
        });
        c.bench_function(&format!("ytm (newton), {name}"), |b| {
            b.iter(|| black_box(bond).ytm(black_box(p)))
        });
    }

    c.bench_function("modified duration, 30y", |b| {
        b.iter(|| black_box(&long).modified_duration(black_box(y)))
    });
    c.bench_function("convexity, 30y", |b| {
        b.iter(|| black_box(&long).convexity(black_box(y)))
    });

    let shifts = [-300.0, -100.0, -10.0, 10.0, 100.0, 300.0];
    c.bench_function("shift table, 6 shifts, 30y", |b| {
        b.iter(|| black_box(&long).shift_table(black_box(y), black_box(&shifts)))
    });
}

criterion_group!(benches, bench_bonds);
criterion_main!(benches);
