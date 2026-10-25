use quantlab::bonds::Bond;

const EPS: f64 = 1e-6;

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < EPS
}

/// Облигация A: номинал 100, купон 5% раз в год, 5 лет.
fn bond_a() -> Bond {
    Bond {
        face: 100.0,
        coupon_rate: 0.05,
        maturity: 5.0,
        freq: 1,
    }
}

/// Облигация B: номинал 100, купон 6% дважды в год, 10 лет.
fn bond_b() -> Bond {
    Bond {
        face: 100.0,
        coupon_rate: 0.06,
        maturity: 10.0,
        freq: 2,
    }
}

// ---------- Денежные потоки ----------

#[test]
fn bond_a_cashflows() {
    assert_eq!(
        bond_a().cashflows(),
        vec![(1.0, 5.0), (2.0, 5.0), (3.0, 5.0), (4.0, 5.0), (5.0, 105.0)]
    );
}

#[test]
fn bond_b_cashflows() {
    let cf = bond_b().cashflows();
    assert_eq!(cf.len(), 20);
    assert_eq!(cf[0], (0.5, 3.0));
    assert_eq!(cf[19], (10.0, 103.0));
}

// ---------- Цена ----------

#[test]
fn bond_a_price_at_par() {
    // купон = доходность => цена = номинал
    assert!(approx(bond_a().price(0.05), 100.0));
}

#[test]
fn bond_b_price() {
    assert!(approx(bond_b().price(0.08), 86.409673655));
}

#[test]
fn price_falls_when_yield_rises() {
    let b = bond_a();
    assert!(b.price(0.06) < b.price(0.05));
    assert!(b.price(0.04) > b.price(0.05));
}

#[test]
fn ytm_roundtrip() {
    for b in [bond_a(), bond_b()] {
        for y in [-0.02, 0.0, 0.01, 0.05, 0.08, 0.15, 0.50] {
            let p = b.price(y);
            let found = b.ytm(p).expect("ytm should exist");
            assert!((found - y).abs() < 1e-9, "y={y}, found={found}");
        }
    }
}

#[test]
fn ytm_at_par_equals_coupon() {
    assert!((bond_a().ytm(100.0).unwrap() - 0.05).abs() < 1e-9);
}

#[test]
fn ytm_none_for_impossible_price() {
    assert!(bond_a().ytm(-5.0).is_none());
}

#[test]
fn ytm_bisection_roundtrip() {
    for b in [bond_a(), bond_b()] {
        for y in [-0.02, 0.0, 0.01, 0.05, 0.08, 0.15, 0.50] {
            let p = b.price(y);
            let found = b.ytm_bisection(p).expect("ytm should exist");
            assert!((found - y).abs() < 1e-9, "y={y}, found={found}");
        }
    }
}

#[test]
fn newton_and_bisection_agree() {
    let long = Bond::new(100.0, 0.06, 30.0, 2);
    for b in [bond_a(), bond_b(), long] {
        for y in [-0.02, 0.0, 0.03, 0.07, 0.15, 0.50] {
            let p = b.price(y);
            let n = b.ytm(p).unwrap();
            let s = b.ytm_bisection(p).unwrap();
            assert!((n - s).abs() < 1e-9, "y={y}: newton={n}, bisection={s}");
        }
    }
}

#[test]
fn ytm_zero_coupon() {
    // у бескупонной облигации старт Ньютона = 0, проверяем, что он всё равно сходится
    let z = Bond::new(100.0, 0.0, 10.0, 1);
    for y in [0.01, 0.05, 0.20] {
        let p = z.price(y);
        assert!((z.ytm(p).unwrap() - y).abs() < 1e-9);
    }
}

// ---------- Дюрации и DV01 ----------

#[test]
fn bond_a_durations() {
    let b = bond_a();
    assert!(approx(b.macaulay_duration(0.05), 4.545950504));
    assert!(approx(b.modified_duration(0.05), 4.329476671));
    assert!(approx(b.dv01(0.05), 0.043294767));
}

#[test]
fn bond_b_durations() {
    let b = bond_b();
    assert!(approx(b.macaulay_duration(0.08), 7.454251784));
    assert!(approx(b.modified_duration(0.08), 7.167549792));
}

#[test]
fn zero_coupon_macaulay_equals_maturity() {
    let z = Bond {
        face: 100.0,
        coupon_rate: 0.0,
        maturity: 7.0,
        freq: 1,
    };
    assert!(approx(z.macaulay_duration(0.04), 7.0));
}

#[test]
fn modified_duration_matches_numerical_derivative() {
    // dP/dy ≈ (P(y+h) - P(y-h)) / 2h  должно равняться  -D_mod * P
    for b in [bond_a(), bond_b()] {
        let y = 0.07;
        let h = 1e-5;
        let p = b.price(y);
        let numerical = (b.price(y + h) - b.price(y - h)) / (2.0 * h);
        let analytical = -b.modified_duration(y) * p;
        assert!(
            (numerical - analytical).abs() < 1e-4,
            "numerical={numerical}, analytical={analytical}"
        );
    }
}

#[test]
fn higher_coupon_means_lower_duration() {
    let low = Bond {
        face: 100.0,
        coupon_rate: 0.02,
        maturity: 10.0,
        freq: 1,
    };
    let high = Bond {
        face: 100.0,
        coupon_rate: 0.10,
        maturity: 10.0,
        freq: 1,
    };
    assert!(high.macaulay_duration(0.05) < low.macaulay_duration(0.05));
}

#[test]
fn shift_table_bond_a_100bp() {
    let row = &bond_a().shift_table(0.05, &[100.0])[0];
    assert!((row.exact - 95.7876).abs() < 1e-4);
    assert!((row.duration_est - 95.6705).abs() < 1e-4);
    assert!((row.convexity_est - 95.7902).abs() < 1e-4);
}

#[test]
fn duration_always_underestimates_price() {
    let shifts = [-300.0, -100.0, -10.0, 10.0, 100.0, 300.0];
    for b in [bond_a(), bond_b()] {
        for row in b.shift_table(0.05, &shifts) {
            assert!(row.duration_est < row.exact, "shift={}", row.shift_bp);
        }
    }
}

#[test]
fn convexity_reduces_error() {
    let shifts = [-300.0, -100.0, 100.0, 300.0];
    for row in bond_a().shift_table(0.05, &shifts) {
        let err_d = (row.duration_est - row.exact).abs();
        let err_c = (row.convexity_est - row.exact).abs();
        assert!(err_c < err_d, "shift={}", row.shift_bp);
    }
}
