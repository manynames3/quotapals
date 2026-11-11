// One provider's latest session reading for the pet-only usage glance.
use super::logos::Provider;

pub struct ServiceRow {
    pub provider: Provider,
    pub pct: Option<f64>,
}
