use serde::Deserialize;
use std::fs::OpenOptions;
use anyhow::{Context, Result};
use std::path::Path;

/// * `name` - Track name
/// * `t_q` - (s) Best qualifying lap time
/// * `t_gap_racepace` - (s) Estimated gap between t_q and best race lap time (due to engine mode
/// etc.)
/// * `s_mass` - (s/kg) Lap time mass sensitivity
/// * `t_drseffect` - (s) Lap time reduction when using DRS in all available DRS zones (negative)
/// * `pit_speedlimit` - (m/s) Speed limit when driving through the pit lane
/// * `t_loss_firstlap` - (s) Lap time loss due to the start from standstill
/// * `d_per_gridpos` - (m) Distance between two grid positions (negative)
/// * `d_first_gridpos` - (m) Distance between the first grid position and the finish line (can be
/// negative or positive)
/// * `length` - (m) Length of the track
/// * `real_length_pit_zone`- (m) Real length of pit zone (required to virtually adjust pit lane
/// speed such that a shorter or longer pit lane can be considered)
/// * `s12` - (m) Boundary between sectors 1 and 2
/// * `s23` - (m) Boundary between sectors 2 and 3
/// * `drs_measurement_points` - (m) DRS measurement points
/// * `turn_1` - (m) Distance between finish line and the first corner of the track
/// * `pit_zone` - (m) Start and end of the pit zone (in track coordinates)
/// * `pits_aft_finishline` - True if pits are located after the finish line, false if located
/// before
/// * `overtaking_zones` - (m) Start and end of the overtaking zones
#[derive(Debug, Deserialize, Clone)]
pub struct TrackPars {
    pub name: String,
    pub t_q: f64,
    pub t_gap_racepace: f64,
    pub s_mass: f64,
    pub t_drseffect: f64,
    pub pit_speedlimit: f64,
    pub t_loss_firstlap: f64,
    pub d_per_gridpos: f64,
    pub d_first_gridpos: f64,
    pub length: f64,
    pub real_length_pit_zone: f64,
    pub s12: f64,
    pub s23: f64,
    pub drs_measurement_points: Vec<f64>,
    pub turn_1: f64,
    pub pit_zone: [f64; 2],
    pub pits_aft_finishline: bool,
    pub overtaking_zones: Vec<[f64; 2]>,
    #[serde(default)]
    pub corners: Vec<[f64; 2]>,
}

#[derive(Debug)]
pub struct Track {
    pub name: String,
    pub t_q: f64,
    pub t_gap_racepace: f64,
    pub s_mass: f64,
    pub t_drseffect: f64,
    pub pit_speedlimit: f64,
    pub t_loss_firstlap: f64,
    pub d_per_gridpos: f64,
    pub d_first_gridpos: f64,
    pub length: f64,
    pub real_length_pit_zone: f64,
    pub track_length_pit_zone: f64,
    pub s12: f64,
    pub s23: f64,
    pub drs_measurement_points: Vec<f64>,
    pub turn_1: f64,
    pub turn_1_lap_frac: f64,
    pub pit_zone: [f64; 2],
    pub pits_aft_finishline: bool,
    pub overtaking_zones: Vec<[f64; 2]>,
    pub overtaking_zones_lap_frac: f64,
    pub corners: Vec<[f64; 2]>,
    pub multipliers: Vec<f64>,
}


#[derive(Debug, Deserialize, Clone)]
pub struct CsvTrackEl {
    pub x_m: f64,
    pub y_m: f64,
    pub w_tr_left_m: f64,
    pub w_tr_right_m: f64,
}

// CALCULATE TRACK MULTIPLIERS ON EACH POINT
// Fixed: Return Result<Vec<f64>> because Track needs the vector, not just min/max
pub fn calc_track_multipliers(track_name: &str) -> Result<Vec<f64>> {

    let mut trackfile_path = std::path::PathBuf::new();
    trackfile_path.push("input");
    trackfile_path.push("tracks");
    trackfile_path.push(&track_name);
    trackfile_path.set_extension("csv");

    let fh = OpenOptions::new()
        .read(true)
        .open(&trackfile_path)
        .context(format!(
            "Failed to open track file {}!",
            trackfile_path.to_str().unwrap_or("unknown")
        ))?;

    let mut csv_reader = csv::Reader::from_reader(&fh);
    let mut csv_track_cl: Vec<CsvTrackEl> = vec![];

    for result in csv_reader.deserialize() {
        let csv_track_el: CsvTrackEl = result?;
        csv_track_cl.push(csv_track_el);
    }

    let n = csv_track_cl.len();
    if n < 3 {
        // Return a default vector of 1.0s if track is too short
        return Ok(vec![1.0; n.max(1)]); 
    }

    // Compute distances
    let mut dist: Vec<f64> = vec![0.0; n - 1];
    for i in 0..n - 1 {
        let dx = csv_track_cl[i + 1].x_m - csv_track_cl[i].x_m;
        let dy = csv_track_cl[i + 1].y_m - csv_track_cl[i].y_m;
        dist[i] = (dx * dx + dy * dy).sqrt();
    }

    // Compute curvature approximations
    let mut kappa: Vec<f64> = vec![0.0; n];
    for i in 1..n - 1 {
        let prev_dx = csv_track_cl[i].x_m - csv_track_cl[i - 1].x_m;
        let prev_dy = csv_track_cl[i].y_m - csv_track_cl[i - 1].y_m;
        let next_dx = csv_track_cl[i + 1].x_m - csv_track_cl[i].x_m;
        let next_dy = csv_track_cl[i + 1].y_m - csv_track_cl[i].y_m;

        let norm_prev = (prev_dx * prev_dx + prev_dy * prev_dy).sqrt();
        let norm_next = (next_dx * next_dx + next_dy * next_dy).sqrt();

        if norm_prev == 0.0 || norm_next == 0.0 {
            continue;
        }

        let dot = prev_dx * next_dx + prev_dy * next_dy;
        let cos_theta = (dot / (norm_prev * norm_next)).clamp(-1.0, 1.0);
        let theta = cos_theta.acos();

        let ds = (dist[i - 1] + dist[i]) / 2.0;
        if ds == 0.0 {
            continue;
        }

        kappa[i] = theta / ds;
    }

    // Set end curvatures
    kappa[0] = kappa[1];
    kappa[n - 1] = kappa[n - 2];

    // Compute raw multipliers
    let mut raw_multi: Vec<f64> = vec![0.0; n];
    for i in 0..n {
        raw_multi[i] = 1.0 / (1.0 + kappa[i]);
        // make the raw_multi more sensite to curvature (power of 5)
        raw_multi[i] = raw_multi[i].powf(5.0);
        // minimum 0.1 multiplier
        raw_multi[i] = raw_multi[i].max(0.5);
    }

    // Normalize multipliers
    let avg_raw: f64 = raw_multi.iter().sum::<f64>() / n as f64;
    let mut multi: Vec<f64> = vec![0.0; n];
    for i in 0..n {
        multi[i] = if avg_raw != 0.0 {
            raw_multi[i] / avg_raw
        } else {
            1.0
        };
    }

    Ok(multi) // Return the vector
}


impl Track {
    pub fn new(track_pars: &TrackPars) -> Track {
        // determine track distance that is covered by the pit lane when driving through it
        let track_length_pit_zone = if track_pars.pit_zone[0] < track_pars.pit_zone[1] {
            track_pars.pit_zone[1] - track_pars.pit_zone[0]
        } else {
            track_pars.length - track_pars.pit_zone[0] + track_pars.pit_zone[1]
        };

        // calculate overtaking zones lap fraction
        let mut len_overtaking_zones = 0.0;

        for overtaking_zone in track_pars.overtaking_zones.iter() {
            len_overtaking_zones += if overtaking_zone[0] < overtaking_zone[1] {
                overtaking_zone[1] - overtaking_zone[0]
            } else {
                track_pars.length - overtaking_zone[0] + overtaking_zone[1]
            };
        }

        let overtaking_zones_lap_frac = len_overtaking_zones / track_pars.length;

        // calculate turn 1 lap fraction
        let turn_1_lap_frac = (track_pars.turn_1 - track_pars.d_first_gridpos) / track_pars.length;

        // Calculate multipliers
        // We handle the error gracefully by defaulting to an empty vector or 1.0s if file fails
        let multipliers = calc_track_multipliers(track_pars.name.as_str()).unwrap_or_else(|e| {
            eprintln!("Warning: Could not calc multipliers: {}. Defaulting to 1.0", e);
            vec![1.0] 
        });

        // create track
        Track {
            name: track_pars.name.to_owned(),
            t_q: track_pars.t_q,
            t_gap_racepace: track_pars.t_gap_racepace,
            s_mass: track_pars.s_mass,
            t_drseffect: track_pars.t_drseffect,
            pit_speedlimit: track_pars.pit_speedlimit,
            t_loss_firstlap: track_pars.t_loss_firstlap,
            d_per_gridpos: track_pars.d_per_gridpos,
            d_first_gridpos: track_pars.d_first_gridpos,
            length: track_pars.length,
            real_length_pit_zone: track_pars.real_length_pit_zone,
            track_length_pit_zone,
            s12: track_pars.s12,
            s23: track_pars.s23,
            drs_measurement_points: track_pars.drs_measurement_points.to_owned(),
            turn_1: track_pars.turn_1,
            turn_1_lap_frac,
            overtaking_zones_lap_frac,
            pits_aft_finishline: track_pars.pits_aft_finishline,
            pit_zone: track_pars.pit_zone,
            overtaking_zones: track_pars.overtaking_zones.to_owned(),
            corners: track_pars.corners.to_owned(),
            multipliers,
        }
    }

    pub fn is_in_overtaking_zone(&self, s_track: f64) -> bool {
        for zone in &self.overtaking_zones {
            if zone[0] < zone[1] {
                // Normal case: overtaking zone does not wrap around the finish line
                if s_track >= zone[0] && s_track <= zone[1] {
                    return true;
                }
            } else {
                // Wrap-around case: zone crosses the finish line
                if s_track >= zone[0] || s_track <= zone[1] {
                    return true;
                }
            }
        }
        false
    }

    /// The method returns the approximate time loss when driving through the pit lane.
    pub fn get_pit_drive_timeloss(&self) -> f64 {
        let pit_zone_lap_frac = self.track_length_pit_zone / self.length;
        self.real_length_pit_zone / self.pit_speedlimit
            - (self.t_q + self.t_gap_racepace) * 1.04 * pit_zone_lap_frac
    }
}
