use quantlab::analytic::black_scholes;
use quantlab::monte_carlo::{monte_carlo_call, monte_carlo_call_multithread};
use quantlab::params::OptionParams;

fn params(spot: f64, strike: f64, time: f64, rate: f64, vol: f64) -> OptionParams {
    OptionParams {
        spot,
        strike,
        time,
        rate,
        vol,
    }
}

fn atm() -> OptionParams {
    params(100.0, 100.0, 1.0, 0.05, 0.2)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn reference_one_year() {
    let (c, p) = black_scholes(&atm());
    assert!(close(c, 10.4506), "call = {c}");
    assert!(close(p, 5.5735), "put = {p}");
}

#[test]
fn reference_half_year() {
    let (c, _) = black_scholes(&params(100.0, 100.0, 0.5, 0.05, 0.2));
    assert!(close(c, 6.8887), "call = {c}");
}

#[test]
fn put_call_parity() {
    for p in [
        params(100.0, 90.0, 0.5, 0.03, 0.25),
        params(50.0, 70.0, 2.0, 0.01, 0.4),
    ] {
        let (c, put) = black_scholes(&p);
        let rhs = p.spot - p.strike * (-p.rate * p.time).exp();
        assert!(close(c - put, rhs), "C - P = {}, expected {rhs}", c - put);
    }
}

#[test]
fn monte_carlo_matches_black_scholes() {
    let (bs, _) = black_scholes(&atm());
    let (mc, se) = monte_carlo_call(&atm(), 200_000, 7);
    assert!(
        (mc - bs).abs() < 3.0 * se,
        "mc = {mc}, bs = {bs}, se = {se}"
    );
}

#[test]
fn multithread_matches_black_scholes() {
    let (bs, _) = black_scholes(&atm());
    let (mc, se) = monte_carlo_call_multithread(&atm(), 2_500_000, 7);
    assert!(
        (mc - bs).abs() < 3.0 * se,
        "mc = {mc}, bs = {bs}, se = {se}"
    );
}

#[test]
fn multithread_is_reproducible() {
    let a = monte_carlo_call_multithread(&atm(), 2_500_000, 42);
    let b = monte_carlo_call_multithread(&atm(), 2_500_000, 42);
    assert_eq!(a, b);
}
