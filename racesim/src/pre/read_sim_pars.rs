use crate::core::car::CarPars;
use crate::core::driver::DriverPars;
use crate::core::race::{RacePars, SimConstants};
use crate::core::track::TrackPars;
use anyhow::Context;
use serde::Deserialize;
use crate::core::tireset::TireConfig;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::path::Path;

/// SimPars is used to store all other parameter structs.
#[derive(Debug, Deserialize, Clone)]
pub struct SimPars {
    pub race_pars: RacePars,
    pub track_pars: TrackPars,
    pub driver_pars_all: HashMap<String, DriverPars>,
    pub car_pars_all: HashMap<u32, CarPars>,
}

/// read_sim_pars reads the JSON file and decodes the JSON string into the simulation parameters
/// struct.
pub fn read_sim_pars(filepath: &Path) -> anyhow::Result<SimPars> {
    let fh = OpenOptions::new()
        .read(true)
        .open(filepath)
        .context(format!(
            "Failed to open parameter file {}!",
            filepath.to_str().unwrap()
        ))?;
    let pars = serde_json::from_reader(&fh).context(format!(
        "Failed to parse parameter file {}!",
        filepath.to_str().unwrap()
    ))?;
    Ok(pars)
}

/// Read simulation constants (physics/engine parameters) from a JSON file.
pub fn read_sim_constants(filepath: &Path) -> anyhow::Result<SimConstants> {
    let fh = OpenOptions::new()
        .read(true)
        .open(filepath)
        .context(format!(
            "Failed to open simulation constants file {}!",
            filepath.to_str().unwrap()
        ))?;

    let pars = serde_json::from_reader(&fh).context(format!(
        "Failed to parse simulation constants file {}!",
        filepath.to_str().unwrap()
    ))?;
    Ok(pars)
}

#[derive(Debug, Deserialize, Clone)]
pub struct RaceScenarioFile {
    pub race_pars: RacePars,
    pub driver_pars_all: HashMap<String, DriverPars>,
    pub car_pars_all: HashMap<u32, CarPars>,
}

pub fn read_race_scenario(filepath: &Path) -> anyhow::Result<RaceScenarioFile> {
    let fh = OpenOptions::new()
        .read(true)
        .open(filepath)
        .context(format!(
            "Failed to open race scenario file {}!",
            filepath.to_str().unwrap()
        ))?;
    let pars = serde_json::from_reader(&fh).context(format!(
        "Failed to parse race scenario file {}!",
        filepath.to_str().unwrap()
    ))?;
    Ok(pars)
}

pub fn read_track_pars(filepath: &Path) -> anyhow::Result<TrackPars> {
    let fh = OpenOptions::new()
        .read(true)
        .open(filepath)
        .context(format!(
            "Failed to open track config file {}!",
            filepath.to_str().unwrap()
        ))?;
    let pars = serde_json::from_reader(&fh).context(format!(
        "Failed to parse track config file {}!",
        filepath.to_str().unwrap()
    ))?;
    Ok(pars)
}

pub fn read_tire_config(filepath: &Path) -> anyhow::Result<TireConfig> {
    let fh = OpenOptions::new()
        .read(true)
        .open(filepath)
        .context(format!(
            "Failed to open tire config file {}!",
            filepath.to_str().unwrap()
        ))?;
    let pars = serde_json::from_reader(&fh).context(format!(
        "Failed to parse tire config file {}!",
        filepath.to_str().unwrap()
    ))?;
    Ok(pars)
}

/// Flexible reader: tries full SimPars first; if it fails, reads a scenario-only file
/// (without `track_pars`) and loads track from `input/parameters/tracks/{track_name}.json`.
pub fn read_sim_pars_flexible(filepath: &Path) -> anyhow::Result<SimPars> {
    match read_sim_pars(filepath) {
        Ok(p) => Ok(p),
        Err(_) => {
            let scen = read_race_scenario(filepath)?;
            let track_name = scen
                .race_pars
                .track_name
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Scenario missing track_name; required when track_pars is not present"))?;
            let track_path: std::path::PathBuf = [
                "input",
                "parameters",
                "tracks",
                &format!("{}.json", track_name),
            ]
            .iter()
            .collect();
            let track_pars = read_track_pars(&track_path)?;
            Ok(SimPars {
                race_pars: scen.race_pars,
                track_pars,
                driver_pars_all: scen.driver_pars_all,
                car_pars_all: scen.car_pars_all,
            })
        }
    }
}
