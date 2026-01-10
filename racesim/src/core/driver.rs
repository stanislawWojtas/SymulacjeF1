use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

/// * `initials` - Driver initials, e.g. BOT
/// * `name` - Driver name, e.g. Valtteri Bottas
/// * `t_driver` - (s) Time loss per lap due to driver abilities
/// * `vel_max` - (km/h) Maximum velocity during qualifying
#[derive(Debug, Deserialize, Clone)]
pub struct DriverPars {
    pub initials: String,
    pub name: String,
    pub t_driver: f64,
    #[serde(default = "default_consistency")]
    pub consistency: f64,
    #[serde(default = "default_aggression")]
    pub aggression: f64,
    pub vel_max: f64,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>, // allows legacy tire/degradation fields without using them
}

fn default_consistency() -> f64 {
    1.0
}

fn default_aggression() -> f64 {
    0.5
}

#[derive(Debug)]
pub struct Driver {
    pub initials: String,
    _name: String,
    pub t_driver: f64,
    pub consistency: f64,
    pub aggression: f64,
    _vel_max: f64,
}

impl Driver {
    pub fn new(driver_pars: &DriverPars) -> Driver {
        Driver {
            initials: driver_pars.initials.to_owned(),
            _name: driver_pars.name.to_owned(),
            t_driver: driver_pars.t_driver,
            consistency: driver_pars.consistency,
            aggression: driver_pars.aggression,
            _vel_max: driver_pars.vel_max,
        }
    }
}