use crate::options::option::{OptionContract, OptionError, OptionKind};
use statrs::distribution::{ContinuousCDF, Normal};

pub fn black_scholes(p: &OptionContract) -> Result<(f64, f64), OptionError> {
    if p.option_kind != OptionKind::European {
        return Err(OptionError::UnsupportedKind);
    }
    let q = p.dividend_yield;
    let d1 = ((p.spot / p.strike).ln() + (p.rate - q + p.vol.powi(2) / 2.0) * p.time)
        / (p.vol * p.time.sqrt());
    let d2 = d1 - p.vol * p.time.sqrt();
    let n = Normal::new(0.0, 1.0).unwrap();
    let nd1 = n.cdf(d1);
    let nd2 = n.cdf(d2);
    let c = p.spot * (-q * p.time).exp() * nd1 - p.strike * (-p.rate * p.time).exp() * nd2;
    let put = -p.spot * (-q * p.time).exp() * n.cdf(-d1)
        + p.strike * (-p.rate * p.time).exp() * n.cdf(-d2);

    Ok((c, put))
}
