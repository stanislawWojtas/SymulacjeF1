use crate::core::tireset::DegrPars;
use serde::Deserialize;
use std::collections::HashMap;

/// * `initials` - Driver initials, e.g. BOT
/// * `name` - Driver name, e.g. Valtteri Bottas
/// * `t_driver` - (s) Time loss per lap due to driver abilities
/// * `vel_max` - (km/h) Maximum velocity during qualifying
/// * `degr_pars_all` - Map containing the degradation parameters for all relevant tire compounds
#[derive(Debug, Deserialize, Clone)]
pub struct DriverPars {
    pub initials: String,
    pub name: String,
    pub t_driver: f64,
    #[serde(default = "default_consistency")]
    pub consistency: f64,
    #[serde(default = "default_aggression")]
    pub aggression: f64,
    // Usunięto t_teamorder
    pub vel_max: f64,
    pub degr_pars_all: HashMap<String, DegrPars>,
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
    name: String,
    pub t_driver: f64,
    pub consistency: f64,
    pub aggression: f64,
    // Usunięto t_teamorder
    vel_max: f64,
    degr_pars_all: HashMap<String, DegrPars>,
}

impl Driver {
    pub fn new(driver_pars: &DriverPars) -> Driver {
        Driver {
            initials: driver_pars.initials.to_owned(),
            name: driver_pars.name.to_owned(),
            t_driver: driver_pars.t_driver,
            consistency: driver_pars.consistency,
            aggression: driver_pars.aggression,
            // Usunięto t_teamorder
            vel_max: driver_pars.vel_max,
            degr_pars_all: driver_pars.degr_pars_all.to_owned(),
        }
    }

    /// The method returns the degradation parameters of the current driver for the given compound.
    pub fn get_degr_pars(&self, compound: &str) -> DegrPars {
        self.degr_pars_all
            .get(compound)
            .expect("Degradation parameters are not available for the given compound!")
            .to_owned()
    }
}