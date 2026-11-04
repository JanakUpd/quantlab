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
    MaturityBeforeSettlement,
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
    fn flows(&self, settlement: NaiveDate) -> Result<Vec<(f64, f64)>, BondError> {
        if self.maturity <= settlement {
            return Err(BondError::MaturityBeforeSettlement);
        }
        Ok(self
            .cashflows()?
            .into_iter()
            .map(|(d, cf)| ((d - settlement).num_days() as f64 / 365.0, cf))
            .filter(|&(t, _)| t > 0.0)
            .collect())
    }

    pub fn price(&self, rate: f64, settlement: NaiveDate) -> f64 {
        let b = self.base(rate);
        self.flows(settlement)
            .expect("settlement must be before maturity")
            .iter()
            .map(|&(t, cf)| cf / b.powf(t))
            .sum()
    }

    pub fn ytm_bisection(&self, market_price: f64, settlement: NaiveDate) -> Option<f64> {
        let (mut l, mut r) = (-0.999, 5.0);
        if market_price <= 0.0
            || market_price > self.price(l, settlement)
            || market_price < self.price(r, settlement)
        {
            return None;
        }
        let mut iter = 0;
        while r - l > 1e-12 {
            let mid = (l + r) / 2.0;
            if iter > 60 {
                return None;
            }
            if self.price(mid, settlement) < market_price {
                r = mid;
            } else {
                l = mid;
            }
            iter += 1;
        }
        Some((l + r) / 2.0)
    }

    pub fn ytm(&self, market_price: f64, settlement: NaiveDate) -> Option<f64> {
        if market_price <= 0.0
            || market_price > self.price(-0.99, settlement)
            || market_price < self.price(5.0, settlement)
        {
            return None;
        }
        let mut y = self.get_rate();
        let mut prev_y;
        let mut iter = 0;
        loop {
            if iter > 20 {
                return self.ytm_bisection(market_price, settlement);
            }
            let price = self.price(y, settlement);
            prev_y = y;
            y += (price - market_price) * self.base(y) / self.pv_weighted_time(y, settlement);
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
    fn pv_weighted_time(&self, rate: f64, settlement: NaiveDate) -> f64 {
        let b = self.base(rate);
        self.flows(settlement)
            .expect("settlement must be before maturity")
            .iter()
            .map(|&(t, cf)| t * cf / b.powf(t))
            .sum()
    }

    pub fn macaulay_duration(&self, rate: f64, settlement: NaiveDate) -> f64 {
        self.pv_weighted_time(rate, settlement) / self.price(rate, settlement)
    }

    pub fn modified_duration(&self, rate: f64, settlement: NaiveDate) -> f64 {
        self.macaulay_duration(rate, settlement) / self.base(rate)
    }

    pub fn dv01(&self, rate: f64, settlement: NaiveDate) -> f64 {
        self.pv_weighted_time(rate, settlement) / self.base(rate) * 0.0001
    }

    pub fn price_change_pct(&self, rate: f64, delta_rate: f64, settlement: NaiveDate) -> f64 {
        -self.modified_duration(rate, settlement) * delta_rate
    }

    /// Σ t(t+1) · PV(t) / (1+y)² / цена
    pub fn convexity(&self, rate: f64, settlement: NaiveDate) -> f64 {
        let b = self.base(rate);
        let s: f64 = self
            .flows(settlement)
            .expect("settlement must be before maturity")
            .iter()
            .map(|&(t, cf)| t * (t + 1.0) * cf / b.powf(t))
            .sum();
        s / (b * b) / self.price(rate, settlement)
    }

    pub fn price_estimate_duration(
        &self,
        rate: f64,
        delta_rate: f64,
        settlement: NaiveDate,
    ) -> f64 {
        self.price(rate, settlement) * (1.0 + self.price_change_pct(rate, delta_rate, settlement))
    }

    pub fn price_estimate_convexity(
        &self,
        rate: f64,
        delta_rate: f64,
        settlement: NaiveDate,
    ) -> f64 {
        self.price(rate, settlement)
            * (1.0 - self.modified_duration(rate, settlement) * delta_rate
                + 0.5 * self.convexity(rate, settlement) * delta_rate * delta_rate)
    }

    pub fn shift_table(
        &self,
        rate: f64,
        shifts_bp: &[f64],
        settlement: NaiveDate,
    ) -> Vec<ShiftRow> {
        let an = self.analytics(rate, settlement);
        shifts_bp
            .iter()
            .map(|&bp| {
                let delta_rate = bp / 10_000.0;
                ShiftRow {
                    shift_bp: bp,
                    exact: self.price(rate + delta_rate, settlement),
                    duration_est: an.price * (1.0 - an.modified_duration * delta_rate),
                    convexity_est: an.price
                        * (1.0 - an.modified_duration * delta_rate
                            + 0.5 * an.convexity * delta_rate * delta_rate),
                }
            })
            .collect()
    }

    /// one-pass analytics for better performance; returns batch of metrics
    pub fn analytics(&self, rate: f64, settlement: NaiveDate) -> BondAnalytics {
        let b = self.base(rate);
        let (mut pr, mut wt, mut cv) = (0.0, 0.0, 0.0);
        for (t, cf) in self
            .flows(settlement)
            .expect("settlement must be before maturity")
            .iter()
        {
            let pv = cf / b.powf(*t);
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

    /// accrued interest (НКД) at settlement; 0 on coupon dates and for zero-coupon bonds
    pub fn accrued(&self, settlement: NaiveDate) -> f64 {
        if settlement >= self.maturity || self.nature == CouponRule::ZeroCoupon {
            return 0.0;
        }
        let cfs = self
            .cashflows()
            .expect("cashflow error at accrued calculation");
        let prev = cfs
            .iter()
            .map(|&(d, _)| d)
            .filter(|&d| d <= settlement)
            .next_back()
            .unwrap_or(self.issue_date);
        let next = cfs
            .iter()
            .map(|&(d, _)| d)
            .find(|&d| d > settlement)
            .unwrap();
        match self.nature {
            CouponRule::ZeroCoupon => 0.0,
            CouponRule::Frequent { frequency, rate } => {
                self.face * rate / frequency as u8 as f64 * (settlement - prev).num_days() as f64
                    / (next - prev).num_days() as f64
            }
            CouponRule::DateBased {
                date_interval: _,
                rate,
            } => self.face * rate * (settlement - prev).num_days() as f64 / 365.0,
        }
    }

    /// clean = dirty_price - accrued
    pub fn clean_price(&self, rate: f64, settlement: NaiveDate) -> f64 {
        self.price(rate, settlement) - self.accrued(settlement)
    }

    /// ytm_from_clean(price = clean) = ytm(price = clean + accrued)
    pub fn ytm_from_clean(&self, clean: f64, settlement: NaiveDate) -> Option<f64> {
        self.ytm(clean + self.accrued(settlement), settlement)
    }
}
