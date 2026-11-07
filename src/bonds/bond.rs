use chrono::{Days, Months, NaiveDate};
use moka::sync::Cache;
use std::num::NonZeroU16;
use std::sync::Arc;

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

type Flows = Arc<Vec<(f64, f64)>>;
type FlowsCache = Cache<NaiveDate, Flows>;

#[derive(Debug, Clone)]
pub struct Bond {
    issue_date: NaiveDate,
    face: f64,
    maturity: NaiveDate,
    nature: CouponRule,
    flows_cache: Option<FlowsCache>,
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
    InvalidMarketPrice,
    NoConvergence,
    IndefinitePriceCalculated,
    InvalidRate,
}

impl std::fmt::Display for BondError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            BondError::DateOutOfRange => "date is out of the supported range",
            BondError::MaturityBeforeIssue => "maturity must be after the issue date",
            BondError::MaturityBeforeSettlement => "settlement must be before maturity",
            BondError::InvalidMarketPrice => {
                "market price is outside the range matched by any yield in [-99.9%, 500%]"
            }
            BondError::NoConvergence => "yield solver did not converge",
            BondError::IndefinitePriceCalculated => "price function calculated indefinite result",
            BondError::InvalidRate => "rate must be finite and greater than -100%",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for BondError {}

impl From<Arc<BondError>> for BondError {
    fn from(e: Arc<BondError>) -> Self {
        (*e).clone()
    }
}

impl Bond {
    fn validate_rate(rate: f64) -> Result<(), BondError> {
        if !rate.is_finite() || rate <= -1.0 {
            return Err(BondError::InvalidRate);
        }
        Ok(())
    }

    fn validate_market_price(price: f64) -> Result<(), BondError> {
        if !price.is_finite() {
            return Err(BondError::InvalidMarketPrice);
        }
        Ok(())
    }
    pub fn get_issue_date(&self) -> NaiveDate {
        self.issue_date
    }

    pub fn get_face(&self) -> f64 {
        self.face
    }

    pub fn get_maturity(&self) -> NaiveDate {
        self.maturity
    }

    pub fn get_nature(&self) -> CouponRule {
        self.nature.clone()
    }

    pub fn with_cache(mut self, capacity: u64) -> Self {
        self.flows_cache = Some(Cache::new(capacity));
        self
    }

    pub fn zero_coupon(issue_date: NaiveDate, face: f64, maturity: NaiveDate) -> Self {
        Self {
            issue_date,
            face,
            maturity,
            nature: CouponRule::ZeroCoupon,
            flows_cache: None,
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
            flows_cache: None,
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
            flows_cache: None,
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
                        .ok_or(BondError::DateOutOfRange)?;
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
                    .ok_or(BondError::DateOutOfRange)?;
                while i > self.issue_date {
                    ans.push((i, coupon));
                    i = i
                        .checked_sub_days(period)
                        .ok_or(BondError::DateOutOfRange)?;
                }
                ans.reverse();
                Ok(ans)
            }
            CouponRule::ZeroCoupon => Ok(vec![(self.maturity, self.face); 1]),
        }
    }

    fn flows(&self, settlement: NaiveDate) -> Result<Flows, Arc<BondError>> {
        match &self.flows_cache {
            Some(cache) => cache.try_get_with(settlement, || self.compute_flows(settlement)),
            None => Ok(self.compute_flows(settlement)?),
        }
    }

    fn compute_flows(&self, settlement: NaiveDate) -> Result<Flows, BondError> {
        if self.maturity <= settlement {
            return Err(BondError::MaturityBeforeSettlement);
        }
        Ok(Arc::new(
            self.cashflows()?
                .into_iter()
                .map(|(d, cf)| ((d - settlement).num_days() as f64 / 365.0, cf))
                .filter(|&(t, _)| t > 0.0)
                .collect(),
        ))
    }

    fn compute_flows_from_cfs(
        &self,
        settlement: NaiveDate,
        cfs: &[(NaiveDate, f64)],
    ) -> Result<Flows, BondError> {
        if self.maturity <= settlement {
            return Err(BondError::MaturityBeforeSettlement);
        }
        Ok(Arc::new(
            cfs.iter()
                .map(|(d, cf)| ((*d - settlement).num_days() as f64 / 365.0, *cf))
                .filter(|&(t, _)| t > 0.0)
                .collect(),
        ))
    }

    pub fn price(&self, rate: f64, settlement: NaiveDate) -> Result<f64, BondError> {
        Self::validate_rate(rate)?;
        let res = self.price_from_flow(rate, self.flows(settlement)?.as_slice());
        if res.is_finite() {
            Ok(res)
        } else {
            Err(BondError::IndefinitePriceCalculated)
        }
    }

    pub fn price_from_flow(&self, rate: f64, flow: &[(f64, f64)]) -> f64 {
        let b = self.base(rate);
        flow.iter().map(|&(t, cf)| cf / b.powf(t)).sum()
    }

    pub fn ytm_bisection(
        &self,
        market_price: f64,
        settlement: NaiveDate,
    ) -> Result<f64, BondError> {
        Self::validate_market_price(market_price)?;
        let flows = self.flows(settlement)?;
        self.ytm_bisection_from_flow(market_price, flows.as_slice())
    }

    pub fn ytm_bisection_from_flow(
        &self,
        market_price: f64,
        flow: &[(f64, f64)],
    ) -> Result<f64, BondError> {
        let (mut l, mut r) = (-0.999, 5.0);
        if market_price <= 0.0
            || market_price > self.price_from_flow(l, flow)
            || market_price < self.price_from_flow(r, flow)
        {
            return Err(BondError::InvalidMarketPrice);
        }
        let mut iter = 0;
        while r - l > 1e-12 {
            let mid = (l + r) / 2.0;
            if iter > 60 {
                return Err(BondError::NoConvergence);
            }
            if self.price_from_flow(mid, flow) < market_price {
                r = mid;
            } else {
                l = mid;
            }
            iter += 1;
        }
        Ok((l + r) / 2.0)
    }

    pub fn ytm(&self, market_price: f64, settlement: NaiveDate) -> Result<f64, BondError> {
        Self::validate_market_price(market_price)?;
        let flows = self.flows(settlement)?;
        self.ytm_from_flow(market_price, flows.as_slice())
    }

    pub fn ytm_from_flow(&self, market_price: f64, flow: &[(f64, f64)]) -> Result<f64, BondError> {
        if market_price <= 0.0
            || market_price > self.price_from_flow(-0.99, flow)
            || market_price < self.price_from_flow(5.0, flow)
        {
            return Err(BondError::InvalidMarketPrice);
        }
        let mut y = self.get_rate();
        let mut prev_y;
        let mut iter = 0;
        loop {
            if iter > 20 {
                return self.ytm_bisection_from_flow(market_price, flow);
            }
            let price = self.price_from_flow(y, flow);
            prev_y = y;
            y += (price - market_price) * self.base(y) / self.pv_weighted_time_from_flow(y, flow);
            iter += 1;
            if (y - prev_y).abs() < 1e-12 {
                return Ok(y);
            }
        }
    }

    fn base(&self, rate: f64) -> f64 {
        1.0 + rate
    }

    /// Σ t · PV(t)
    fn pv_weighted_time_from_flow(&self, rate: f64, flow: &[(f64, f64)]) -> f64 {
        let b = self.base(rate);
        flow.iter().map(|&(t, cf)| t * cf / b.powf(t)).sum()
    }

    pub fn macaulay_duration(&self, rate: f64, settlement: NaiveDate) -> Result<f64, BondError> {
        Self::validate_rate(rate)?;
        let flows = self.flows(settlement)?;
        Ok(self.macaulay_duration_from_flow(rate, flows.as_slice()))
    }

    pub fn macaulay_duration_from_flow(&self, rate: f64, flow: &[(f64, f64)]) -> f64 {
        self.pv_weighted_time_from_flow(rate, flow) / self.price_from_flow(rate, flow)
    }

    pub fn modified_duration(&self, rate: f64, settlement: NaiveDate) -> Result<f64, BondError> {
        Self::validate_rate(rate)?;
        let flows = self.flows(settlement)?;
        Ok(self.modified_duration_from_flow(rate, flows.as_slice()))
    }

    pub fn modified_duration_from_flow(&self, rate: f64, flow: &[(f64, f64)]) -> f64 {
        self.macaulay_duration_from_flow(rate, flow) / self.base(rate)
    }

    pub fn dv01(&self, rate: f64, settlement: NaiveDate) -> Result<f64, BondError> {
        Self::validate_rate(rate)?;
        let flows = self.flows(settlement)?;
        Ok(self.dv01_from_flow(rate, flows.as_slice()))
    }

    pub fn dv01_from_flow(&self, rate: f64, flow: &[(f64, f64)]) -> f64 {
        self.pv_weighted_time_from_flow(rate, flow) / self.base(rate) * 0.0001
    }

    pub fn price_change_pct(
        &self,
        rate: f64,
        delta_rate: f64,
        settlement: NaiveDate,
    ) -> Result<f64, BondError> {
        Self::validate_rate(rate)?;
        let flows = self.flows(settlement)?;
        Ok(self.price_change_pct_from_flow(rate, delta_rate, flows.as_slice()))
    }

    pub fn price_change_pct_from_flow(
        &self,
        rate: f64,
        delta_rate: f64,
        flow: &[(f64, f64)],
    ) -> f64 {
        -self.modified_duration_from_flow(rate, flow) * delta_rate
    }

    /// Σ t(t+1) · PV(t) / (1+y)² / цена
    pub fn convexity(&self, rate: f64, settlement: NaiveDate) -> Result<f64, BondError> {
        Self::validate_rate(rate)?;
        let flows = self.flows(settlement)?;
        Ok(self.convexity_from_flow(rate, flows.as_slice()))
    }

    pub fn convexity_from_flow(&self, rate: f64, flow: &[(f64, f64)]) -> f64 {
        let b = self.base(rate);
        let s: f64 = flow
            .iter()
            .map(|&(t, cf)| t * (t + 1.0) * cf / b.powf(t))
            .sum();
        s / (b * b) / self.price_from_flow(rate, flow)
    }

    pub fn price_estimate_duration(
        &self,
        rate: f64,
        delta_rate: f64,
        settlement: NaiveDate,
    ) -> Result<f64, BondError> {
        Self::validate_rate(rate)?;
        let flows = self.flows(settlement)?;
        Ok(self.price_from_flow(rate, flows.as_slice())
            * (1.0 + self.price_change_pct_from_flow(rate, delta_rate, flows.as_slice())))
    }

    pub fn price_estimate_convexity(
        &self,
        rate: f64,
        delta_rate: f64,
        settlement: NaiveDate,
    ) -> Result<f64, BondError> {
        Self::validate_rate(rate)?;
        let flows = self.flows(settlement)?;
        Ok(self.price_from_flow(rate, flows.as_slice())
            * (1.0 - self.modified_duration_from_flow(rate, flows.as_slice()) * delta_rate
                + 0.5 * self.convexity_from_flow(rate, flows.as_slice()) * delta_rate * delta_rate))
    }

    pub fn shift_table(
        &self,
        rate: f64,
        shifts_bp: &[f64],
        settlement: NaiveDate,
    ) -> Result<Vec<ShiftRow>, BondError> {
        Self::validate_rate(rate)?;
        let flows = self.flows(settlement)?;
        let an = self.analytics_from_flow(rate, flows.as_slice());
        shifts_bp
            .iter()
            .map(|&bp| {
                let delta_rate = bp / 10_000.0;
                Self::validate_rate(rate + delta_rate)?;
                Ok(ShiftRow {
                    shift_bp: bp,
                    exact: self.price_from_flow(rate + delta_rate, flows.as_slice()),
                    duration_est: an.price * (1.0 - an.modified_duration * delta_rate),
                    convexity_est: an.price
                        * (1.0 - an.modified_duration * delta_rate
                            + 0.5 * an.convexity * delta_rate * delta_rate),
                })
            })
            .collect()
    }

    /// one-pass analytics for better performance; returns batch of metrics
    pub fn analytics(&self, rate: f64, settlement: NaiveDate) -> Result<BondAnalytics, BondError> {
        Self::validate_rate(rate)?;
        let flows = self.flows(settlement)?;
        Ok(self.analytics_from_flow(rate, flows.as_slice()))
    }

    pub fn analytics_from_flow(&self, rate: f64, flows: &[(f64, f64)]) -> BondAnalytics {
        let b = self.base(rate);
        let (mut pr, mut wt, mut cv) = (0.0, 0.0, 0.0);
        for (t, cf) in flows.iter() {
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
    pub fn accrued(&self, settlement: NaiveDate) -> Result<f64, BondError> {
        let cfs = self.cashflows()?;
        Ok(self.accrued_from_flow(settlement, cfs.as_slice()))
    }

    pub fn accrued_from_flow(&self, settlement: NaiveDate, cfs: &[(NaiveDate, f64)]) -> f64 {
        if settlement <= self.issue_date
            || settlement >= self.maturity
            || self.nature == CouponRule::ZeroCoupon
        {
            return 0.0;
        }
        let prev = cfs
            .iter()
            .map(|&(d, _)| d)
            .rfind(|&d| d <= settlement)
            .unwrap_or(self.issue_date);
        let next = cfs
            .iter()
            .map(|&(d, _)| d)
            .find(|&d| d > settlement)
            .expect("maturity is always in cashflows");
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
    pub fn clean_price(&self, rate: f64, settlement: NaiveDate) -> Result<f64, BondError> {
        Self::validate_rate(rate)?;
        let cfs = self.cashflows()?;
        let flows = self.compute_flows_from_cfs(settlement, cfs.as_slice())?;
        Ok(self.price_from_flow(rate, flows.as_slice())
            - self.accrued_from_flow(settlement, cfs.as_slice()))
    }

    /// ytm_from_clean(price = clean) = ytm(price = clean + accrued)
    pub fn ytm_from_clean(&self, clean: f64, settlement: NaiveDate) -> Result<f64, BondError> {
        let cfs = self.cashflows()?;
        let flows = self.compute_flows_from_cfs(settlement, cfs.as_slice())?;
        let market_price = clean + self.accrued_from_flow(settlement, cfs.as_slice());
        Self::validate_market_price(market_price)?;
        self.ytm_from_flow(market_price, flows.as_slice())
    }
}
