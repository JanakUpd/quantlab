#[derive(Debug, Clone, PartialEq)]
pub struct Bond {
    pub face: f64,
    pub coupon_rate: f64,
    pub maturity: f64,
    pub freq: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShiftRow {
    pub shift_bp: f64,
    pub exact: f64,
    pub duration_est: f64,
    pub convexity_est: f64,
}

impl Bond {
    pub fn new(face: f64, coupon_rate: f64, maturity: f64, freq: u32) -> Self {
        Self {
            face,
            coupon_rate,
            maturity,
            freq,
        }
    }

    fn cashflow_iter(&self) -> impl DoubleEndedIterator<Item = (f64, f64)> + '_ {
        let n = (self.maturity * self.freq as f64).round() as usize;
        let coupon = self.coupon_rate * self.face / self.freq as f64;
        (1..n)
            .map(move |x| (x as f64 / self.freq as f64, coupon))
            .chain([(n as f64 / self.freq as f64, coupon + self.face)])
    }

    pub fn cashflows(&self) -> Vec<(f64, f64)> {
        self.cashflow_iter().collect()
    }

    pub fn price(&self, rate: f64) -> f64 {
        let v = 1.0 / self.base(rate);
        let mut res = 0.0;
        for (_, cf) in self.cashflow_iter().rev() {
            res = res * v + cf;
        }
        res * v
    }

    pub fn ytm_bisection(&self, market_price: f64) -> Option<f64> {
        let (mut l, mut r) = (-0.999, 5.0);
        if market_price <= 0.0 || market_price > self.price(l) || market_price < self.price(r) {
            return None;
        }
        let mut iter = 0;
        while r - l > 1e-12 {
            let mid = (l + r) / 2.0;
            if iter > 60 {
                return None;
            }
            if self.price(mid) < market_price {
                r = mid;
            } else {
                l = mid;
            }
            iter += 1;
        }
        Some((l + r) / 2.0)
    }

    pub fn ytm(&self, market_price: f64) -> Option<f64> {
        if market_price <= 0.0 || market_price > self.price(-0.99) || market_price < self.price(5.0) {
            return None;
        }
        let mut y = self.coupon_rate;
        let mut prev_y;
        let mut iter = 0;
        loop {
            if iter > 20 {
                return self.ytm_bisection(market_price);
            }
            let price = self.price(y);
            prev_y = y;
            y += (price - market_price) * self.base(y) / self.pv_weighted_time(y);
            iter += 1;
            if (y - prev_y).abs() < 1e-12 { return Some(y); }
        }
    }

    fn base(&self, rate: f64) -> f64 {
        1.0 + rate / self.freq as f64
    }

    fn pv_weighted_time(&self, rate: f64) -> f64 {
        let mut res = 0.0;
        let v = 1.0 / self.base(rate);
        for (t, cf) in self.cashflow_iter().rev() {
            res = res * v + cf * t
        }
        res * v
    }

    pub fn macaulay_duration(&self, rate: f64) -> f64 {
        self.pv_weighted_time(rate) / self.price(rate)
    }

    pub fn modified_duration(&self, rate: f64) -> f64 {
        self.macaulay_duration(rate) / self.base(rate)
    }

    pub fn dv01(&self, rate: f64) -> f64 {
        self.pv_weighted_time(rate) / self.base(rate) * 0.0001
    }

    pub fn price_change_pct(&self, rate: f64, delta_rate: f64) -> f64 {
        -self.modified_duration(rate) * delta_rate
    }

    pub fn convexity(&self, rate: f64) -> f64 {
        let mut res = 0.0;
        let v = 1.0 / self.base(rate);
        let m = self.freq as f64;
        for (t, cf) in self.cashflow_iter().rev() {
            res = res * v + t * (t + 1.0 / m) * cf;
        }
        res * v * v * v / self.price(rate)
    }

    pub fn price_estimate_duration(&self, rate: f64, delta_rate: f64) -> f64 {
        self.price(rate) * (1.0 + self.price_change_pct(rate, delta_rate))
    }

    pub fn price_estimate_convexity(&self, rate: f64, delta_rate: f64) -> f64 {
        self.price(rate)
            * (1.0 - self.modified_duration(rate) * delta_rate
                + 0.5 * self.convexity(rate) * delta_rate * delta_rate)
    }

    pub fn shift_table(&self, rate: f64, shifts_bp: &[f64]) -> Vec<ShiftRow> {
        shifts_bp
            .iter()
            .map(|x| {
                let delta_rate = *x / 10_000.0;
                ShiftRow {
                    shift_bp: *x,
                    exact: self.price(rate + delta_rate),
                    duration_est: self.price_estimate_duration(rate, delta_rate),
                    convexity_est: self.price_estimate_convexity(rate, delta_rate),
                }
            })
            .collect()
    }
}
