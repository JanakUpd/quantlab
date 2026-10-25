use criterion::{Criterion, criterion_group, criterion_main};
use quantlab::bonds::Bond;
use std::hint::black_box;

fn bench_bonds(c: &mut Criterion) {
    let short = Bond::new(100.0, 0.05, 5.0, 1);
    let long = Bond::new(100.0, 0.06, 30.0, 2);
    let longlong = Bond::new(100.0, 0.06, 5000.0, 2);
    let y = 0.05;

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
