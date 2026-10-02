use clap::Parser;

fn positive(s: &str) -> Result<f64, String> {
    let v: f64 = s.parse().map_err(|_| format!("`{s}` is not a number"))?;
    if v > 0.0 {
        Ok(v)
    } else {
        Err(format!("must be > 0, got {v}"))
    }
}
#[derive(Debug, Clone, Copy)]
pub struct OptionParams {
    pub spot: f64,
    pub strike: f64,
    pub time: f64,
    pub rate: f64,
    pub vol: f64,
}

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(long, value_parser = positive)]
    pub(crate) spot: f64,
    #[arg(long, value_parser = positive)]
    pub(crate) strike: f64,
    #[arg(long, value_parser = positive)]
    pub(crate) time: f64,
    #[arg(long)]
    pub(crate) rate: f64,
    #[arg(long, value_parser = positive)]
    pub(crate) vol: f64,
    #[arg(long, default_value_t = 1_000_000)]
    pub(crate) paths: usize,
}
