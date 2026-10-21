#[derive(Debug, Clone, Copy)]
pub struct EuropeanOption {
    pub spot: f64,
    pub strike: f64,
    pub time: f64,
    pub rate: f64,
    pub vol: f64,
}
