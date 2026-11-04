use chrono::NaiveDate;
use quantlab::bonds::{Bond, BondError, BondFrequency};
use std::num::NonZeroU16;

const EPS: f64 = 1e-6;

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < EPS
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

/// Облигация A: номинал 100, купон 5% раз в год, 15.06.2025 → 15.06.2030.
fn bond_a() -> Bond {
    Bond::frequent(
        d(2025, 6, 15),
        100.0,
        d(2030, 6, 15),
        BondFrequency::Annual,
        0.05,
    )
}

/// Облигация B: номинал 100, купон 6% дважды в год, 15.06.2025 → 15.06.2035.
fn bond_b() -> Bond {
    Bond::frequent(
        d(2025, 6, 15),
        100.0,
        d(2035, 6, 15),
        BondFrequency::SemiAnnual,
        0.06,
    )
}

// Эталонные значения посчитаны независимо (Python):
// цена = Σ CF / (1 + y)^(дни / 365), дюрации и выпуклость — по той же схеме.

// ---------- Денежные потоки ----------

#[test]
fn bond_a_cashflows() {
    assert_eq!(
        bond_a().cashflows().unwrap(),
        vec![
            (d(2026, 6, 15), 5.0),
            (d(2027, 6, 15), 5.0),
            (d(2028, 6, 15), 5.0),
            (d(2029, 6, 15), 5.0),
            (d(2030, 6, 15), 105.0),
        ]
    );
}

#[test]
fn bond_b_cashflows() {
    let cf = bond_b().cashflows().unwrap();
    assert_eq!(cf.len(), 20);
    assert_eq!(cf[0], (d(2025, 12, 15), 3.0));
    assert_eq!(cf[19], (d(2035, 6, 15), 103.0));
}

#[test]
fn cashflow_dates_strictly_increase_and_follow_issue() {
    for b in [bond_a(), bond_b()] {
        let cf = b.cashflows().unwrap();
        assert!(cf[0].0 > b.issue_date, "купон в день выпуска или раньше");
        assert!(cf.windows(2).all(|w| w[0].0 < w[1].0));
    }
}

#[test]
fn cashflows_end_of_month_does_not_drift() {
    let b = Bond::frequent(
        d(2029, 1, 1),
        1000.0,
        d(2030, 8, 31),
        BondFrequency::SemiAnnual,
        0.04,
    );
    let dates: Vec<_> = b.cashflows().unwrap().iter().map(|x| x.0).collect();
    assert_eq!(
        dates,
        vec![
            d(2029, 2, 28),
            d(2029, 8, 31),
            d(2030, 2, 28),
            d(2030, 8, 31)
        ]
    );
}

#[test]
fn cashflows_first_period_is_short_when_issued_mid_period() {
    let b = Bond::frequent(
        d(2025, 9, 1),
        1000.0,
        d(2027, 6, 15),
        BondFrequency::SemiAnnual,
        0.10,
    );
    let cf = b.cashflows().unwrap();
    assert_eq!(cf[0].0, d(2025, 12, 15));
    assert_eq!(cf.len(), 4);
}

#[test]
fn cashflows_date_based_has_exact_day_intervals() {
    let issue = d(2025, 1, 1);
    let b = Bond::date_based(
        issue,
        1000.0,
        issue + chrono::Days::new(3 * 182),
        NonZeroU16::new(182).unwrap(),
        0.071,
    );
    let cf = b.cashflows().unwrap();
    assert_eq!(cf.len(), 3);
    assert!(cf.windows(2).all(|w| (w[1].0 - w[0].0).num_days() == 182));
    // купон = номинал * ставка * 182 / 365
    assert!(approx(cf[0].1, 1000.0 * 0.071 * 182.0 / 365.0));
    assert!(approx(cf[2].1, 1000.0 + 1000.0 * 0.071 * 182.0 / 365.0));
}

#[test]
fn cashflows_zero_coupon_is_single_payment() {
    let z = Bond::zero_coupon(d(2025, 1, 1), 1000.0, d(2028, 1, 1));
    assert_eq!(z.cashflows().unwrap(), vec![(d(2028, 1, 1), 1000.0)]);
}

#[test]
fn cashflows_rejects_maturity_not_after_issue() {
    for maturity in [d(2025, 1, 1), d(2024, 1, 1)] {
        let b = Bond::zero_coupon(d(2025, 1, 1), 1000.0, maturity);
        assert_eq!(b.cashflows(), Err(BondError::MaturityBeforeIssue));
    }
}

// ---------- Цена ----------

#[test]
fn bond_a_price_close_to_par() {
    // купон = доходность, но високосный 2028-й чуть сдвигает сроки платежей,
    // поэтому цена не ровно 100, а рядом
    let p = bond_a().price(0.05);
    assert!(approx(p, 99.987876395), "price = {p}");
    assert!((p - 100.0).abs() < 0.05);
}

#[test]
fn bond_b_price() {
    assert!(approx(bond_b().price(0.08), 87.339681657));
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
fn ytm_at_market_price_of_par_bond_is_near_coupon() {
    let y = bond_a().ytm(100.0).unwrap();
    assert!((y - 0.05).abs() < 1e-3, "y = {y}");
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
    let long = Bond::frequent(
        d(2025, 6, 15),
        100.0,
        d(2055, 6, 15),
        BondFrequency::SemiAnnual,
        0.06,
    );
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
    let z = Bond::zero_coupon(d(2025, 1, 1), 100.0, d(2035, 1, 1));
    for y in [0.01, 0.05, 0.20] {
        let p = z.price(y);
        assert!((z.ytm(p).unwrap() - y).abs() < 1e-9);
    }
}

// ---------- Дюрации и DV01 ----------

#[test]
fn bond_a_durations() {
    let b = bond_a();
    assert!(approx(b.macaulay_duration(0.05), 4.548397478));
    assert!(approx(b.modified_duration(0.05), 4.331807122));
    assert!(approx(b.dv01(0.05), 0.043312820));
}

#[test]
fn bond_b_durations() {
    let b = bond_b();
    assert!(approx(b.macaulay_duration(0.08), 7.474607720));
    assert!(approx(b.modified_duration(0.08), 6.920933074));
}

#[test]
fn zero_coupon_macaulay_equals_time_to_maturity() {
    // 01.01.2025 → 01.01.2032 = 2556 дней (один високосный 2028-й), 2556 / 365 лет
    let z = Bond::zero_coupon(d(2025, 1, 1), 100.0, d(2032, 1, 1));
    assert!(approx(z.macaulay_duration(0.04), 2556.0 / 365.0));
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
fn convexity_matches_numerical_second_derivative() {
    for b in [bond_a(), bond_b()] {
        let y = 0.07;
        let h = 1e-4;
        let p = b.price(y);
        let numerical = (b.price(y + h) - 2.0 * p + b.price(y - h)) / (h * h) / p;
        assert!(
            (numerical - b.convexity(y)).abs() < 1e-3,
            "numerical={numerical}, analytical={}",
            b.convexity(y)
        );
    }
}

#[test]
fn higher_coupon_means_lower_duration() {
    let low = Bond::frequent(
        d(2025, 1, 1),
        100.0,
        d(2035, 1, 1),
        BondFrequency::Annual,
        0.02,
    );
    let high = Bond::frequent(
        d(2025, 1, 1),
        100.0,
        d(2035, 1, 1),
        BondFrequency::Annual,
        0.10,
    );
    assert!(high.macaulay_duration(0.05) < low.macaulay_duration(0.05));
}

#[test]
fn shift_table_bond_a_100bp() {
    let row = &bond_a().shift_table(0.05, &[100.0])[0];
    assert!((row.exact - 95.7738).abs() < 1e-4);
    assert!((row.duration_est - 95.6566).abs() < 1e-4);
    assert!((row.convexity_est - 95.7764).abs() < 1e-4);
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

#[test]
fn analytics_matches_individual_methods() {
    for b in [bond_a(), bond_b()] {
        for y in [0.01, 0.05, 0.10] {
            let a = b.analytics(y);
            assert!((a.price - b.price(y)).abs() < 1e-10);
            assert!((a.modified_duration - b.modified_duration(y)).abs() < 1e-10);
            assert!((a.convexity - b.convexity(y)).abs() < 1e-10);
            assert!((a.dv01 - b.dv01(y)).abs() < 1e-12);
        }
    }
}
