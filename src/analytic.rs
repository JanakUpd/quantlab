use crate::params::OptionParams;
use statrs::distribution::{ContinuousCDF, Normal};

pub fn black_scholes(p: &OptionParams) -> (f64, f64) {
    let d1 = ((p.spot / p.strike).ln() + (p.rate + p.vol.powi(2) / 2.0) * p.time)
        / (p.vol * p.time.sqrt());
    let d2 = d1 - p.vol * p.time.sqrt();
    let n = Normal::new(0.0, 1.0).unwrap();
    let nd1 = n.cdf(d1);
    let nd2 = n.cdf(d2);
    let c = p.spot * nd1 - p.strike * (-p.rate * p.time).exp() * nd2;
    let put = -p.spot * n.cdf(-d1) + p.strike * (-p.rate * p.time).exp() * n.cdf(-d2);

    (c, put)
}
