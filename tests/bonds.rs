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
// Если не сказано иное, settlement = дата выпуска.

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
        assert!(
            cf[0].0 > b.get_issue_date(),
            "купон в день выпуска или раньше"
        );
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
    let b = bond_a();
    let p = b.price(0.05, b.get_issue_date()).unwrap();
    assert!(approx(p, 99.987876395), "price = {p}");
    assert!((p - 100.0).abs() < 0.05);
}

#[test]
fn bond_b_price() {
    let b = bond_b();
    assert!(approx(
        b.price(0.08, b.get_issue_date()).unwrap(),
        87.339681657
    ));
}

#[test]
fn price_falls_when_yield_rises() {
    let b = bond_a();
    let s = b.get_issue_date();
    assert!(b.price(0.06, s).unwrap() < b.price(0.05, s).unwrap());
    assert!(b.price(0.04, s).unwrap() > b.price(0.05, s).unwrap());
}

#[test]
fn ytm_roundtrip() {
    for b in [bond_a(), bond_b()] {
        let s = b.get_issue_date();
        for y in [-0.02, 0.0, 0.01, 0.05, 0.08, 0.15, 0.50] {
            let p = b.price(y, s).unwrap();
            let found = b.ytm(p, s).expect("ytm should exist");
            assert!((found - y).abs() < 1e-9, "y={y}, found={found}");
        }
    }
}

#[test]
fn ytm_at_market_price_of_par_bond_is_near_coupon() {
    let b = bond_a();
    let y = b.ytm(100.0, b.get_issue_date()).unwrap();
    assert!((y - 0.05).abs() < 1e-3, "y = {y}");
}

#[test]
fn ytm_error_for_impossible_price() {
    let b = bond_a();
    let s = b.get_issue_date();
    assert_eq!(b.ytm(-5.0, s), Err(BondError::InvalidMarketPrice));
    assert_eq!(b.ytm(0.0, s), Err(BondError::InvalidMarketPrice));
    assert_eq!(b.ytm_bisection(-5.0, s), Err(BondError::InvalidMarketPrice));
    // цена выше суммы всех платежей недостижима ни при какой доходности из диапазона
    assert_eq!(b.ytm(1e15, s), Err(BondError::InvalidMarketPrice));
}

#[test]
fn ytm_bisection_roundtrip() {
    for b in [bond_a(), bond_b()] {
        let s = b.get_issue_date();
        for y in [-0.02, 0.0, 0.01, 0.05, 0.08, 0.15, 0.50] {
            let p = b.price(y, s).unwrap();
            let found = b.ytm_bisection(p, s).expect("ytm should exist");
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
        let s = b.get_issue_date();
        for y in [-0.02, 0.0, 0.03, 0.07, 0.15, 0.50] {
            let p = b.price(y, s).unwrap();
            let n = b.ytm(p, s).unwrap();
            let bis = b.ytm_bisection(p, s).unwrap();
            assert!((n - bis).abs() < 1e-9, "y={y}: newton={n}, bisection={bis}");
        }
    }
}

#[test]
fn ytm_zero_coupon() {
    // у бескупонной облигации старт Ньютона = 0, проверяем, что он всё равно сходится
    let z = Bond::zero_coupon(d(2025, 1, 1), 100.0, d(2035, 1, 1));
    let s = z.get_issue_date();
    for y in [0.01, 0.05, 0.20] {
        let p = z.price(y, s).unwrap();
        assert!((z.ytm(p, s).unwrap() - y).abs() < 1e-9);
    }
}

// ---------- Дюрации и DV01 ----------

#[test]
fn bond_a_durations() {
    let b = bond_a();
    let s = b.get_issue_date();
    assert!(approx(b.macaulay_duration(0.05, s).unwrap(), 4.548397478));
    assert!(approx(b.modified_duration(0.05, s).unwrap(), 4.331807122));
    assert!(approx(b.dv01(0.05, s).unwrap(), 0.043312820));
}

#[test]
fn bond_b_durations() {
    let b = bond_b();
    let s = b.get_issue_date();
    assert!(approx(b.macaulay_duration(0.08, s).unwrap(), 7.474607720));
    assert!(approx(b.modified_duration(0.08, s).unwrap(), 6.920933074));
}

#[test]
fn zero_coupon_macaulay_equals_time_to_maturity() {
    // 01.01.2025 → 01.01.2032 = 2556 дней (один високосный 2028-й), 2556 / 365 лет
    let z = Bond::zero_coupon(d(2025, 1, 1), 100.0, d(2032, 1, 1));
    assert!(approx(
        z.macaulay_duration(0.04, z.get_issue_date()).unwrap(),
        2556.0 / 365.0
    ));
}

#[test]
fn modified_duration_matches_numerical_derivative() {
    // dP/dy ≈ (P(y+h) - P(y-h)) / 2h  должно равняться  -D_mod * P
    for b in [bond_a(), bond_b()] {
        let s = b.get_issue_date();
        let y = 0.07;
        let h = 1e-5;
        let p = b.price(y, s).unwrap();
        let numerical = (b.price(y + h, s).unwrap() - b.price(y - h, s).unwrap()) / (2.0 * h);
        let analytical = -b.modified_duration(y, s).unwrap() * p;
        assert!(
            (numerical - analytical).abs() < 1e-4,
            "numerical={numerical}, analytical={analytical}"
        );
    }
}

#[test]
fn convexity_matches_numerical_second_derivative() {
    for b in [bond_a(), bond_b()] {
        let s = b.get_issue_date();
        let y = 0.07;
        let h = 1e-4;
        let p = b.price(y, s).unwrap();
        let numerical =
            (b.price(y + h, s).unwrap() - 2.0 * p + b.price(y - h, s).unwrap()) / (h * h) / p;
        let analytical = b.convexity(y, s).unwrap();
        assert!(
            (numerical - analytical).abs() < 1e-3,
            "numerical={numerical}, analytical={analytical}"
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
    let s = low.get_issue_date();
    assert!(high.macaulay_duration(0.05, s).unwrap() < low.macaulay_duration(0.05, s).unwrap());
}

#[test]
fn shift_table_bond_a_100bp() {
    let b = bond_a();
    let row = &b.shift_table(0.05, &[100.0], b.get_issue_date()).unwrap()[0];
    assert!((row.exact - 95.7738).abs() < 1e-4);
    assert!((row.duration_est - 95.6566).abs() < 1e-4);
    assert!((row.convexity_est - 95.7764).abs() < 1e-4);
}

#[test]
fn duration_always_underestimates_price() {
    let shifts = [-300.0, -100.0, -10.0, 10.0, 100.0, 300.0];
    for b in [bond_a(), bond_b()] {
        for row in b.shift_table(0.05, &shifts, b.get_issue_date()).unwrap() {
            assert!(row.duration_est < row.exact, "shift={}", row.shift_bp);
        }
    }
}

#[test]
fn convexity_reduces_error() {
    let shifts = [-300.0, -100.0, 100.0, 300.0];
    let b = bond_a();
    for row in b.shift_table(0.05, &shifts, b.get_issue_date()).unwrap() {
        let err_d = (row.duration_est - row.exact).abs();
        let err_c = (row.convexity_est - row.exact).abs();
        assert!(err_c < err_d, "shift={}", row.shift_bp);
    }
}

#[test]
fn analytics_matches_individual_methods() {
    for b in [bond_a(), bond_b()] {
        let s = b.get_issue_date();
        for y in [0.01, 0.05, 0.10] {
            let a = b.analytics(y, s).unwrap();
            assert!((a.price - b.price(y, s).unwrap()).abs() < 1e-10);
            assert!((a.macaulay_duration - b.macaulay_duration(y, s).unwrap()).abs() < 1e-10);
            assert!((a.modified_duration - b.modified_duration(y, s).unwrap()).abs() < 1e-10);
            assert!((a.convexity - b.convexity(y, s).unwrap()).abs() < 1e-10);
            assert!((a.dv01 - b.dv01(y, s).unwrap()).abs() < 1e-12);
        }
    }
}

// ---------- Settlement внутри жизни облигации ----------

#[test]
fn settlement_on_coupon_date_excludes_that_coupon() {
    // 15.06.2027 купон уже принадлежит продавцу: остаются 2028, 2029, 2030
    let b = bond_a();
    let s = d(2027, 6, 15);
    assert!(approx(b.price(0.05, s).unwrap(), 99.986633725));
    assert!(approx(b.macaulay_duration(0.05, s).unwrap(), 2.862150157));
}

#[test]
fn settlement_mid_period() {
    let b = bond_a();
    let s = d(2027, 9, 1);
    assert!(approx(b.price(0.05, s).unwrap(), 101.034587181));
    assert!(approx(b.macaulay_duration(0.05, s).unwrap(), 2.648451527));
}

#[test]
fn settlement_one_day_before_maturity() {
    let b = bond_a();
    let s = d(2030, 6, 14);
    assert!(approx(b.price(0.05, s).unwrap(), 104.985965411));
    // единственный платёж через 1 день → дюрация = 1/365
    assert!(approx(b.macaulay_duration(0.05, s).unwrap(), 1.0 / 365.0));
}

#[test]
fn errors_when_settlement_is_not_before_maturity() {
    let b = bond_a();
    for s in [b.get_maturity(), d(2031, 1, 1)] {
        let err = Err(BondError::MaturityBeforeSettlement);
        assert_eq!(b.price(0.05, s), err);
        assert_eq!(b.macaulay_duration(0.05, s), err);
        assert_eq!(b.modified_duration(0.05, s), err);
        assert_eq!(b.dv01(0.05, s), err);
        assert_eq!(b.convexity(0.05, s), err);
        assert_eq!(b.ytm(100.0, s), err);
        assert_eq!(b.ytm_bisection(100.0, s), err);
        assert_eq!(b.clean_price(0.05, s), err);
        assert_eq!(b.ytm_from_clean(100.0, s), err);
        assert_eq!(
            b.analytics(0.05, s),
            Err(BondError::MaturityBeforeSettlement)
        );
        assert_eq!(
            b.shift_table(0.05, &[100.0], s),
            Err(BondError::MaturityBeforeSettlement)
        );
    }
}

#[test]
fn ytm_roundtrip_with_mid_life_settlement() {
    for b in [bond_a(), bond_b()] {
        let s = d(2027, 9, 1);
        for y in [-0.02, 0.0, 0.05, 0.15, 0.50] {
            let p = b.price(y, s).unwrap();
            let found = b.ytm(p, s).expect("ytm should exist");
            assert!((found - y).abs() < 1e-9, "y={y}, found={found}");
        }
    }
}

#[test]
fn analytics_matches_methods_with_mid_life_settlement() {
    let b = bond_b();
    let s = d(2029, 3, 20);
    let a = b.analytics(0.07, s).unwrap();
    assert!((a.price - b.price(0.07, s).unwrap()).abs() < 1e-10);
    assert!((a.macaulay_duration - b.macaulay_duration(0.07, s).unwrap()).abs() < 1e-10);
    assert!((a.convexity - b.convexity(0.07, s).unwrap()).abs() < 1e-10);
}

// ---------- НКД (accrued interest) ----------

#[test]
fn accrued_is_zero_on_issue_date() {
    for b in [bond_a(), bond_b()] {
        assert_eq!(b.accrued(b.get_issue_date()), Ok(0.0));
    }
}

#[test]
fn accrued_is_zero_on_coupon_date() {
    // купон в день settlement уже принадлежит продавцу
    assert_eq!(bond_a().accrued(d(2027, 6, 15)), Ok(0.0));
    assert_eq!(bond_b().accrued(d(2030, 12, 15)), Ok(0.0));
}

#[test]
fn accrued_is_zero_for_zero_coupon() {
    let z = Bond::zero_coupon(d(2025, 1, 1), 100.0, d(2030, 1, 1));
    assert_eq!(z.accrued(d(2026, 1, 1)), Ok(0.0));
    assert_eq!(z.accrued(d(2029, 12, 31)), Ok(0.0));
}

#[test]
fn accrued_is_zero_at_or_after_maturity() {
    let b = bond_a();
    assert_eq!(b.accrued(b.get_maturity()), Ok(0.0));
    assert_eq!(b.accrued(d(2031, 1, 1)), Ok(0.0));
}

#[test]
fn accrued_semi_annual_mid_period() {
    // 15.06.2025 → 15.12.2025 = 183 дня, прошло 78 дней, купон 3.0
    assert!(approx(
        bond_b().accrued(d(2025, 9, 1)).unwrap(),
        3.0 * 78.0 / 183.0
    ));
}

#[test]
fn accrued_annual_uses_actual_period_length() {
    // период 15.06.2027 → 15.06.2028 содержит 29 февраля: 366 дней
    assert!(approx(
        bond_a().accrued(d(2027, 9, 1)).unwrap(),
        5.0 * 78.0 / 366.0
    ));
}

#[test]
fn accrued_day_before_coupon_is_almost_full_coupon() {
    let a = bond_a().accrued(d(2027, 6, 14)).unwrap();
    assert!(a < 5.0 && a > 4.98, "accrued = {a}");
    let b = bond_b().accrued(d(2025, 12, 14)).unwrap();
    assert!(b < 3.0 && b > 2.98, "accrued = {b}");
}

#[test]
fn accrued_grows_within_period_and_resets_at_coupon() {
    let b = bond_b();
    let mut prev = 0.0;
    let mut day = d(2025, 6, 16);
    while day < d(2025, 12, 15) {
        let a = b.accrued(day).unwrap();
        assert!(a > prev, "{day}: {a} <= {prev}");
        prev = a;
        day = day.succ_opt().unwrap();
    }
    assert_eq!(b.accrued(d(2025, 12, 15)), Ok(0.0));
}

#[test]
fn accrued_first_period_counts_from_issue_date() {
    // выпуск 01.09.2025, первый купон 15.12.2025: НКД растёт от даты выпуска
    let b = Bond::frequent(
        d(2025, 9, 1),
        1000.0,
        d(2027, 6, 15),
        BondFrequency::SemiAnnual,
        0.10,
    );
    assert_eq!(b.accrued(d(2025, 9, 1)), Ok(0.0));
    let a = b.accrued(d(2025, 10, 1)).unwrap(); // 30 из 105 дней
    assert!(approx(a, 50.0 * 30.0 / 105.0), "accrued = {a}");
}

#[test]
fn accrued_date_based_is_act_365() {
    let issue = d(2025, 1, 1);
    let b = Bond::date_based(
        issue,
        1000.0,
        issue + chrono::Days::new(3 * 182),
        NonZeroU16::new(182).unwrap(),
        0.071,
    );
    // 18 дней после первого купона (issue + 182)
    let s = issue + chrono::Days::new(182 + 18);
    assert!(approx(b.accrued(s).unwrap(), 1000.0 * 0.071 * 18.0 / 365.0));
    // в день купона 0
    assert_eq!(b.accrued(issue + chrono::Days::new(182)), Ok(0.0));
}

#[test]
fn accrued_end_of_month_schedule() {
    // купоны 28.02 / 31.08: период 28.02.2029 → 31.08.2029 = 184 дня
    let b = Bond::frequent(
        d(2029, 1, 1),
        1000.0,
        d(2030, 8, 31),
        BondFrequency::SemiAnnual,
        0.04,
    );
    let a = b.accrued(d(2029, 5, 31)).unwrap(); // 92 дня из 184 = ровно половина купона 20
    assert!(approx(a, 10.0), "accrued = {a}");
}

#[test]
fn accrued_scales_with_face_and_rate() {
    let mk = |face, rate| {
        Bond::frequent(
            d(2025, 6, 15),
            face,
            d(2030, 6, 15),
            BondFrequency::Annual,
            rate,
        )
    };
    let s = d(2027, 9, 1);
    let base = mk(100.0, 0.05).accrued(s).unwrap();
    assert!(approx(mk(1000.0, 0.05).accrued(s).unwrap(), 10.0 * base));
    assert!(approx(mk(100.0, 0.10).accrued(s).unwrap(), 2.0 * base));
}

// ---------- Чистая цена ----------

#[test]
fn clean_price_bond_a_mid_period() {
    // грязная 101.034587181 − НКД 5·78/366
    let b = bond_a();
    let s = d(2027, 9, 1);
    assert!(approx(b.clean_price(0.05, s).unwrap(), 99.969013411));
    assert!(approx(
        b.price(0.05, s).unwrap() - b.clean_price(0.05, s).unwrap(),
        b.accrued(s).unwrap()
    ));
}

#[test]
fn clean_equals_dirty_on_coupon_date_and_at_issue() {
    for b in [bond_a(), bond_b()] {
        for s in [b.get_issue_date(), d(2027, 6, 15)] {
            assert_eq!(b.clean_price(0.06, s), b.price(0.06, s));
        }
    }
}

#[test]
fn clean_equals_dirty_for_zero_coupon() {
    let z = Bond::zero_coupon(d(2025, 1, 1), 100.0, d(2030, 1, 1));
    let s = d(2027, 3, 3);
    assert_eq!(z.clean_price(0.05, s), z.price(0.05, s));
}

#[test]
fn clean_is_below_dirty_strictly_inside_a_period() {
    for b in [bond_a(), bond_b()] {
        let s = d(2027, 9, 1);
        assert!(b.clean_price(0.05, s).unwrap() < b.price(0.05, s).unwrap());
    }
}

#[test]
fn clean_price_is_continuous_across_coupon_date() {
    // грязная цена падает на величину купона в день выплаты, чистая почти не прыгает
    let b = bond_a();
    let y = 0.05;
    let (before, after) = (d(2027, 6, 14), d(2027, 6, 15));
    let dirty_jump = b.price(y, before).unwrap() - b.price(y, after).unwrap();
    let clean_jump = (b.clean_price(y, before).unwrap() - b.clean_price(y, after).unwrap()).abs();
    assert!(dirty_jump > 4.0, "dirty jump = {dirty_jump}");
    assert!(clean_jump < 0.05, "clean jump = {clean_jump}");
}

#[test]
fn ytm_from_clean_roundtrip() {
    let dates = [d(2025, 6, 15), d(2025, 9, 1), d(2027, 9, 1), d(2029, 3, 20)];
    for b in [bond_a(), bond_b()] {
        for s in dates {
            for y in [-0.02, 0.0, 0.05, 0.12, 0.40] {
                let clean = b.clean_price(y, s).unwrap();
                let found = b.ytm_from_clean(clean, s).expect("ytm should exist");
                assert!((found - y).abs() < 1e-9, "s={s}, y={y}, found={found}");
            }
        }
    }
}

#[test]
fn ytm_from_clean_differs_from_ytm_on_same_number() {
    // тот же числовой ввод как чистая цена даёт большую цену (clean + НКД), т.е. меньшую доходность
    let b = bond_a();
    let s = d(2027, 9, 1);
    let as_dirty = b.ytm(100.0, s).unwrap();
    let as_clean = b.ytm_from_clean(100.0, s).unwrap();
    assert!(as_clean < as_dirty);
}

#[test]
fn ytm_from_clean_error_for_impossible_price() {
    let b = bond_a();
    assert_eq!(
        b.ytm_from_clean(-5.0, d(2027, 9, 1)),
        Err(BondError::InvalidMarketPrice)
    );
}

// ---------- Кэш денежных потоков ----------

#[test]
fn cached_and_uncached_results_are_identical() {
    let dates = [d(2025, 6, 15), d(2027, 9, 1), d(2029, 3, 20)];
    let plain = bond_b();
    let cached = bond_b().with_cache(16);
    for s in dates {
        // дважды: первый вызов — промах кэша, второй — попадание
        for _ in 0..2 {
            assert_eq!(plain.price(0.07, s), cached.price(0.07, s));
            assert_eq!(plain.analytics(0.07, s), cached.analytics(0.07, s));
            assert_eq!(
                plain.macaulay_duration(0.07, s),
                cached.macaulay_duration(0.07, s)
            );
            assert_eq!(plain.convexity(0.07, s), cached.convexity(0.07, s));
            assert_eq!(plain.dv01(0.07, s), cached.dv01(0.07, s));
            assert_eq!(
                plain.shift_table(0.07, &[-100.0, 100.0], s),
                cached.shift_table(0.07, &[-100.0, 100.0], s)
            );
            let p = plain.price(0.07, s).unwrap();
            assert_eq!(plain.ytm(p, s), cached.ytm(p, s));
            assert_eq!(plain.ytm_bisection(p, s), cached.ytm_bisection(p, s));
        }
    }
}

#[test]
fn cached_bond_reports_errors_and_does_not_cache_them() {
    let cached = bond_a().with_cache(16);
    let bad = cached.get_maturity();
    assert_eq!(
        cached.price(0.05, bad),
        Err(BondError::MaturityBeforeSettlement)
    );
    // повторный вызов даёт ту же ошибку, а валидная дата после этого по-прежнему считается
    assert_eq!(
        cached.price(0.05, bad),
        Err(BondError::MaturityBeforeSettlement)
    );
    assert!(approx(
        cached.price(0.05, cached.get_issue_date()).unwrap(),
        99.987876395
    ));
}

#[test]
fn cached_clone_gives_same_results() {
    let cached = bond_a().with_cache(16);
    let copy = cached.clone();
    let s = d(2027, 9, 1);
    assert_eq!(cached.price(0.05, s), copy.price(0.05, s));
}

// ---------- Сверка с реальной ОФЗ ----------

/// ОФЗ-ПД, купон 13% (64.82 ₽ за 182 дня), погашение 06.10.2038.
/// Данные из приложения брокера 04.11.2026: чистая цена 842 ₽ (84.2%), НКД 5.7 ₽, доходность 16.65%.
/// 04.11 был праздником (торги в формате сессии выходного дня, сделки относятся к 05.11),
/// поэтому расчёты по Т+1 приходятся на 06.11.2026: 16 дней после купона 21.10.2026.
#[test]
fn real_ofz_matches_broker_quote() {
    let b = Bond::date_based(
        d(2025, 10, 22),
        1000.0,
        d(2038, 10, 6),
        NonZeroU16::new(182).unwrap(),
        0.13,
    );
    // сетка «погашение минус k·182 дня» попадает на реальный купон 21.10.2026
    let cf = b.cashflows().unwrap();
    assert!(cf.iter().any(|&(date, _)| date == d(2026, 10, 21)));
    // купон в рублях совпадает с биржевым
    assert!((cf[0].1 - 64.82).abs() < 0.01);

    let s = d(2026, 11, 6);
    let accrued = b.accrued(s).unwrap();
    assert!((accrued - 5.7).abs() < 0.05, "accrued = {accrued}");

    let y = b.ytm_from_clean(842.0, s).unwrap();
    assert!((y - 0.1665).abs() < 5e-5, "yield = {y}");
}

/// Облигация Евраза: купон 11.42 ₽ раз в 30 дней (≈13.9% годовых), размещение 27.08.2025,
/// погашение 13.02.2028. Приложение брокера, 05.11.2026: цена 98.76% (987.6 ₽), НКД 6.09 ₽,
/// доходность 16.04%. Расчёты Т+1 приходятся на 06.11.2026 (16 дней после купона 21.10.2026).
#[test]
fn real_corporate_matches_broker_quote() {
    // ставка выводится из купона в рублях: он задан в условиях выпуска, а 13.9% лишь его округление
    let rate = 11.42 * 365.0 / 30.0 / 1000.0;
    let b = Bond::date_based(
        d(2025, 8, 27),
        1000.0,
        d(2028, 2, 13),
        NonZeroU16::new(30).unwrap(),
        rate,
    );
    let cf = b.cashflows().unwrap();
    assert_eq!(cf.len(), 30);
    assert!((cf[0].1 - 11.42).abs() < 1e-9);
    assert!(cf.iter().any(|&(date, _)| date == d(2026, 10, 21)));
    assert!(cf.iter().any(|&(date, _)| date == d(2026, 11, 20)));
    assert_eq!(cf.last().unwrap().0, d(2028, 2, 13));

    let s = d(2026, 11, 6);
    let accrued = b.accrued(s).unwrap();
    assert!((accrued - 6.09).abs() < 0.01, "accrued = {accrued}");

    let y = b.ytm_from_clean(987.6, s).unwrap();
    assert!((y - 0.1604).abs() < 5e-5, "yield = {y}");
}
