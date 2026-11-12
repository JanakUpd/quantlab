#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OptionKind {
    American,
    European,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OptionType {
    Call,
    Put,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OptionContract {
    pub spot: f64,
    pub strike: f64,
    pub time: f64,
    pub rate: f64,
    pub vol: f64,
    pub dividend_yield: f64,
    pub option_kind: OptionKind,
    pub option_type: OptionType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OptionError {
    UnsupportedKind,
    InvalidPaths,
}

impl OptionType {
    pub fn payoff(self, spot: f64, strike: f64) -> f64 {
        match self {
            OptionType::Call => (spot - strike).max(0.0),
            OptionType::Put => (strike - spot).max(0.0),
        }
    }
}

impl std::fmt::Display for OptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            OptionError::UnsupportedKind => "unsupported option type supplied",
            OptionError::InvalidPaths => "invalid option type supplied",
        };
        f.write_str(msg)
    }
}
