use quantlab::options::analytic::black_scholes;
use quantlab::options::option::{OptionContract, OptionError, OptionKind, OptionType};

fn contract(
    spot: f64,
    strike: f64,
    time: f64,
    rate: f64,
    vol: f64,
    dividend_yield: f64,
    option_type: OptionType,
) -> OptionContract {
    OptionContract {
        spot,
        strike,
        time,
        rate,
        vol,
        dividend_yield,
        option_type,
        option_kind: OptionKind::European,
    }
}

fn atm(option_type: OptionType) -> OptionContract {
    contract(100.0, 100.0, 1.0, 0.05, 0.2, 0.0, option_type)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-4
}

/// Analytical price for the contract's own type (call or put).
fn bs_price(p: &OptionContract) -> f64 {
    let (call, put) = black_scholes(p).unwrap();
    match p.option_type {
        OptionType::Call => call,
        OptionType::Put => put,
    }
}

// ---------- payoff ----------

#[test]
fn payoff_call_and_put() {
    assert_eq!(OptionType::Call.payoff(120.0, 100.0), 20.0);
    assert_eq!(OptionType::Call.payoff(80.0, 100.0), 0.0);
    assert_eq!(OptionType::Put.payoff(80.0, 100.0), 20.0);
    assert_eq!(OptionType::Put.payoff(120.0, 100.0), 0.0);
}

// ---------- Black-Scholes ----------

#[test]
fn reference_one_year() {
    let (c, p) = black_scholes(&atm(OptionType::Call)).unwrap();
    assert!(close(c, 10.4506), "call = {c}");
    assert!(close(p, 5.5735), "put = {p}");
}

#[test]
fn reference_half_year() {
    let p = contract(100.0, 100.0, 0.5, 0.05, 0.2, 0.0, OptionType::Call);
    let (c, put) = black_scholes(&p).unwrap();
    assert!(close(c, 6.8887), "call = {c}");
    assert!(close(put, 4.4197), "put = {put}");
}

#[test]
fn reference_with_dividend_yield() {
    let p = contract(100.0, 100.0, 1.0, 0.05, 0.2, 0.02, OptionType::Call);
    let (c, put) = black_scholes(&p).unwrap();
    assert!(close(c, 9.2270), "call = {c}");
    assert!(close(put, 6.3301), "put = {put}");
}

#[test]
fn put_call_parity_with_dividends() {
    for p in [
        contract(100.0, 90.0, 0.5, 0.03, 0.25, 0.0, OptionType::Call),
        contract(50.0, 70.0, 2.0, 0.01, 0.4, 0.0, OptionType::Call),
        contract(100.0, 100.0, 1.0, 0.05, 0.2, 0.03, OptionType::Call),
        contract(80.0, 95.0, 1.5, 0.04, 0.3, 0.02, OptionType::Call),
    ] {
        let (c, put) = black_scholes(&p).unwrap();
        let rhs = p.spot * (-p.dividend_yield * p.time).exp() - p.strike * (-p.rate * p.time).exp();
        assert!(close(c - put, rhs), "C - P = {}, expected {rhs}", c - put);
    }
}

#[test]
fn dividends_lower_calls_and_raise_puts() {
    let no_div = black_scholes(&atm(OptionType::Call)).unwrap();
    let with_div = black_scholes(&contract(
        100.0,
        100.0,
        1.0,
        0.05,
        0.2,
        0.03,
        OptionType::Call,
    ))
    .unwrap();
    assert!(with_div.0 < no_div.0, "call should get cheaper");
    assert!(with_div.1 > no_div.1, "put should get more expensive");
}

// ---------- Monte Carlo ----------

/// Cases cover call and put, with and without dividends, ITM / ATM / OTM.
fn mc_cases() -> Vec<OptionContract> {
    vec![
        atm(OptionType::Call),
        atm(OptionType::Put),
        contract(100.0, 100.0, 1.0, 0.05, 0.2, 0.03, OptionType::Call),
        contract(100.0, 100.0, 1.0, 0.05, 0.2, 0.03, OptionType::Put),
        contract(100.0, 90.0, 0.5, 0.03, 0.25, 0.02, OptionType::Put),
        contract(100.0, 110.0, 2.0, 0.04, 0.3, 0.0, OptionType::Call),
    ]
}

#[test]
fn monte_carlo_matches_black_scholes() {
    for p in mc_cases() {
        let bs = bs_price(&p);
        let (mc, se) = p.monte_carlo(400_000, 7).unwrap();
        assert!(
            (mc - bs).abs() < 4.0 * se,
            "{:?}: mc = {mc}, bs = {bs}, se = {se}",
            p.option_type
        );
    }
}

#[test]
fn multithread_matches_black_scholes() {
    for p in mc_cases() {
        let bs = bs_price(&p);
        let (mc, se) = p.monte_carlo_multithread(2_500_000, 7).unwrap();
        assert!(
            (mc - bs).abs() < 4.0 * se,
            "{:?}: mc = {mc}, bs = {bs}, se = {se}",
            p.option_type
        );
    }
}

#[test]
fn multithread_is_reproducible() {
    let p = atm(OptionType::Put);
    let a = p.monte_carlo_multithread(2_500_000, 42).unwrap();
    let b = p.monte_carlo_multithread(2_500_000, 42).unwrap();
    assert_eq!(a, b);
}

#[test]
fn different_seeds_give_different_estimates() {
    let p = atm(OptionType::Call);
    let a = p.monte_carlo(100_000, 1).unwrap();
    let b = p.monte_carlo(100_000, 2).unwrap();
    assert_ne!(a.0, b.0);
}

// ---------- errors ----------

#[test]
fn monte_carlo_rejects_american() {
    let mut p = atm(OptionType::Put);
    p.option_kind = OptionKind::American;
    assert!(matches!(
        p.monte_carlo(1_000, 1),
        Err(OptionError::UnsupportedKind)
    ));
    assert!(matches!(
        p.monte_carlo_multithread(1_000, 1),
        Err(OptionError::UnsupportedKind)
    ));
}

#[test]
fn monte_carlo_rejects_zero_paths() {
    let p = atm(OptionType::Call);
    assert!(matches!(
        p.monte_carlo(0, 1),
        Err(OptionError::InvalidPaths)
    ));
    assert!(matches!(
        p.monte_carlo_multithread(0, 1),
        Err(OptionError::InvalidPaths)
    ));
}

#[test]
fn black_scholes_rejects_american() {
    let mut p = atm(OptionType::Call);
    p.option_kind = OptionKind::American;
    assert!(matches!(
        black_scholes(&p),
        Err(OptionError::UnsupportedKind)
    ));
}
