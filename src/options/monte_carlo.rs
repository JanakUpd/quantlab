use crate::options::option::{OptionContract, OptionError, OptionKind};
use rand::{RngExt, SeedableRng, rngs::StdRng};
use rand_distr::StandardNormal;
use rayon::prelude::*;

impl OptionContract {
    fn simulate_chunk(&self, paths: usize, seed: u64) -> (f64, f64) {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut sum = 0.0;
        let mut sum_sq = 0.0;
        let diffusion = self.vol * self.time.sqrt();
        let shift = (self.rate - self.dividend_yield - self.vol.powi(2) / 2.0) * self.time;
        for _ in 0..paths {
            let z: f64 = rng.sample(StandardNormal);
            let s_t = self.spot * (shift + diffusion * z).exp();
            let payoff = self.option_type.payoff(s_t, self.strike);
            sum += payoff;
            sum_sq += payoff * payoff;
        }

        (sum, sum_sq)
    }

    fn finish(sum: f64, sum_sq: f64, paths: usize, discount: f64) -> (f64, f64) {
        let n = paths as f64;
        let mean = sum / n;
        let std = (sum_sq / n - mean * mean).max(0.0).sqrt();
        (discount * mean, discount * std / n.sqrt())
    }

    fn validate_mc(&self, paths: usize) -> Result<(), OptionError> {
        if self.option_kind != OptionKind::European {
            return Err(OptionError::UnsupportedKind);
        }
        if paths == 0 {
            return Err(OptionError::InvalidPaths);
        }
        Ok(())
    }

    pub fn monte_carlo(&self, paths: usize, seed: u64) -> Result<(f64, f64), OptionError> {
        Self::validate_mc(self, paths)?;

        let discount = (-self.rate * self.time).exp();
        let (sum, sum_sq) = self.simulate_chunk(paths, seed);

        Ok(Self::finish(sum, sum_sq, paths, discount))
    }

    pub fn monte_carlo_multithread(
        &self,
        paths: usize,
        seed: u64,
    ) -> Result<(f64, f64), OptionError> {
        Self::validate_mc(self, paths)?;

        const CHUNK: usize = 65_536;
        let chunks = paths.div_ceil(CHUNK);
        let parts: Vec<(f64, f64)> = (0..chunks)
            .into_par_iter()
            .map(|i| {
                let len = CHUNK.min(paths - i * CHUNK);
                self.simulate_chunk(len, seed.wrapping_add(i as u64))
            })
            .collect();

        let (sum, sum_sq) = parts.iter().fold((0.0, 0.0), |a, b| (a.0 + b.0, a.1 + b.1));
        let discount = (-self.rate * self.time).exp();

        Ok(Self::finish(sum, sum_sq, paths, discount))
    }
}
