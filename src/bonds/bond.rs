use crate::bonds::bond::BondError::DateOutOfRange;
use chrono::{Days, Months, NaiveDate};
use std::num::NonZeroU16;

#[derive(Debug, Clone, PartialEq, Copy, Eq)]
pub enum BondFrequency {
    Annual = 1,
    SemiAnnual = 2,
    Quarterly = 4,
    Monthly = 12,
}
#[derive(Debug, Clone, PartialEq)]
pub enum CouponRule {
    ZeroCoupon,
    Frequent {
        frequency: BondFrequency,
        rate: f64,
    },
    DateBased {
        date_interval: NonZeroU16,
        rate: f64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bond {
    pub issue_date: NaiveDate,
    pub face: f64,
    pub maturity: NaiveDate,
    pub nature: CouponRule,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShiftRow {
    pub shift_bp: f64,
    pub exact: f64,
    pub duration_est: f64,
    pub convexity_est: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BondAnalytics {
    pub price: f64,
    pub macaulay_duration: f64,
    pub modified_duration: f64,
    pub dv01: f64,
    pub convexity: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BondError {
    DateOutOfRange,
    MaturityBeforeIssue,
}

impl Bond {
    pub fn zero_coupon(issue_date: NaiveDate, face: f64, maturity: NaiveDate) -> Self {
        Self {
            issue_date,
            face,
            maturity,
            nature: CouponRule::ZeroCoupon,
        }
    }

    pub fn frequent(
        issue_date: NaiveDate,
        face: f64,
        maturity: NaiveDate,
        frequency: BondFrequency,
        rate: f64,
    ) -> Self {
        Self {
            issue_date,
            face,
            maturity,
            nature: CouponRule::Frequent { frequency, rate },
        }
    }

    pub fn date_based(
        issue_date: NaiveDate,
        face: f64,
        maturity: NaiveDate,
        date_interval: NonZeroU16,
        rate: f64,
    ) -> Self {
        Self {
            issue_date,
            face,
            maturity,
            nature: CouponRule::DateBased {
                date_interval,
                rate,
            },
        }
    }

    fn get_rate(&self) -> f64 {
        match self.nature {
            CouponRule::ZeroCoupon => 0.0,
            CouponRule::Frequent { rate, .. } => rate,
            CouponRule::DateBased { rate, .. } => rate,
        }
    }

    pub fn cashflows(&self) -> Result<Vec<(NaiveDate, f64)>, BondError> {
        if self.maturity <= self.issue_date {
            return Err(BondError::MaturityBeforeIssue);
        }
        match self.nature {
            CouponRule::Frequent { frequency, rate } => {
                let freq = frequency as u8 as f64;
                let period = 12 / frequency as u32;
                let coupon = rate * self.face / freq;
                let mut ans = vec![(self.maturity, self.face + coupon); 1];
                let mut i;
                let mut k = 1;
                loop {
                    i = self
                        .maturity
                        .checked_sub_months(Months::new(period * k))
                        .ok_or(DateOutOfRange)?;
                    if i <= self.issue_date {
                        break;
                    }
                    ans.push((i, coupon));
                    k += 1;
                }
                ans.reverse();
                Ok(ans)
            }
            CouponRule::DateBased {
                date_interval,
                rate,
            } => {
                let period = Days::new(date_interval.get() as u64);
                let coupon = rate * self.face * u16::from(date_interval) as f64 / 365.0;
                let mut ans = vec![(self.maturity, self.face + coupon); 1];
                let mut i = self
                    .maturity
                    .checked_sub_days(period)
                    .ok_or(DateOutOfRange)?;
                while i > self.issue_date {
                    ans.push((i, coupon));
                    i = i.checked_sub_days(period).ok_or(DateOutOfRange)?;
                }
                ans.reverse();
                Ok(ans)
            }
            CouponRule::ZeroCoupon => Ok(vec![(self.maturity, self.face); 1]),
        }
    }
    /// TODO: вернуть Result вместо expect.
    fn flows(&self) -> Vec<(f64, f64)> {
        self.cashflows()
            .expect("invalid bond dates")
            .into_iter()
            .map(|(d, cf)| ((d - self.issue_date).num_days() as f64 / 365.0, cf))
            .collect()
    }

    pub fn price(&self, rate: f64) -> f64 {
        let b = self.base(rate);
        self.flows().iter().map(|&(t, cf)| cf / b.powf(t)).sum()
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
        if market_price <= 0.0 || market_price > self.price(-0.99) || market_price < self.price(5.0)
        {
            return None;
        }
        let mut y = self.get_rate();
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
            if (y - prev_y).abs() < 1e-12 {
                return Some(y);
            }
        }
    }

    fn base(&self, rate: f64) -> f64 {
        1.0 + rate
    }

    /// Σ t · PV(t)
    fn pv_weighted_time(&self, rate: f64) -> f64 {
        let b = self.base(rate);
        self.flows().iter().map(|&(t, cf)| t * cf / b.powf(t)).sum()
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

    /// Σ t(t+1) · PV(t) / (1+y)² / цена
    pub fn convexity(&self, rate: f64) -> f64 {
        let b = self.base(rate);
        let s: f64 = self
            .flows()
            .iter()
            .map(|&(t, cf)| t * (t + 1.0) * cf / b.powf(t))
            .sum();
        s / (b * b) / self.price(rate)
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
        let an = self.analytics(rate);
        shifts_bp
            .iter()
            .map(|&bp| {
                let delta_rate = bp / 10_000.0;
                ShiftRow {
                    shift_bp: bp,
                    exact: self.price(rate + delta_rate),
                    duration_est: an.price * (1.0 - an.modified_duration * delta_rate),
                    convexity_est: an.price
                        * (1.0 - an.modified_duration * delta_rate
                            + 0.5 * an.convexity * delta_rate * delta_rate),
                }
            })
            .collect()
    }

    pub fn analytics(&self, rate: f64) -> BondAnalytics {
        let b = self.base(rate);
        let (mut pr, mut wt, mut cv) = (0.0, 0.0, 0.0);
        for (t, cf) in self.flows() {
            let pv = cf / b.powf(t);
            pr += pv;
            wt += t * pv;
            cv += t * (t + 1.0) * pv;
        }
        let mac_d = wt / pr;
        let mod_d = mac_d / b;
        BondAnalytics {
            price: pr,
            macaulay_duration: mac_d,
            modified_duration: mod_d,
            dv01: wt / b * 0.0001,
            convexity: cv / (b * b) / pr,
        }
    }
}
