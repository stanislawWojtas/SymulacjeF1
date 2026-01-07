use serde::{Deserialize, de};

const MAX_TIRE_PENALTY: f64 = 25.0; // Maksymalna strata: 25 sekund na okrążenie

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum DegrModel {
    Lin,
    NonlinWithCliff, //nielioniowy model degradacji opon
}

/// * `degr_model` - Uproszczony model degradacji -> tylko lin (liniowy)
/// * `k_0` - (s) Parametr degradacji -> offset dla świeżych opon
/// * `k_1_lin` - (s/lap) Parametr degradacji (model liniowy)
#[derive(Debug, Deserialize, Clone)]
pub struct DegrPars {
    pub degr_model: DegrModel,
    pub k_0: f64,
    pub k_1_lin: f64,
    pub cliff_age: Option<f64>,
    pub k_2_cliff: Option<f64>,
}

#[derive(Debug)]
pub struct Tireset {
    pub compound: String,
    pub age_tot: f64,
    pub age_cur_stint: f64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TireCompoundConfig {
    pub k0: f64,
    pub k1_lin: f64,
    pub k1_scale: f64,
    pub default_cliff_age: f64,
    pub default_k2: f64,
    pub base_offset: f64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TireConfig {
    pub soft: TireCompoundConfig,
    pub medium: TireCompoundConfig,
    pub hard: TireCompoundConfig,
    pub intermediate: TireCompoundConfig,
    pub wet: TireCompoundConfig,
}

impl TireConfig {
    pub fn for_compound<'a>(&'a self, comp: &str) -> &'a TireCompoundConfig {
        match comp.to_uppercase().as_str() {
            "SOFT" => &self.soft,
            "MEDIUM" => &self.medium,
            "HARD" => &self.hard,
            "INTERMEDIATE" => &self.intermediate,
            "WET" => &self.wet,
            _ => &self.medium, // neutral fallback
        }
    }

    /// Returns base degradation parameters for a compound shared by all drivers/teams.
    pub fn degr_pars_for_compound(&self, comp: &str) -> DegrPars {
        let cfg = self.for_compound(comp);
        DegrPars {
            degr_model: DegrModel::Lin,
            k_0: cfg.k0,
            k_1_lin: cfg.k1_lin,
            cliff_age: Some(cfg.default_cliff_age),
            k_2_cliff: Some(cfg.default_k2),
        }
    }
}

impl Tireset {
    pub fn new(compound: String, age_tot: u32) -> Tireset {
        Tireset {
            compound,
            age_tot: age_tot as f64,
            age_cur_stint: 0.0,
        }
    }

    /// drive_lap zwiększa wiek opon o jedno okrążenie.
    pub fn drive_lap(&mut self, wear_factor: f64) {
        self.age_cur_stint += 1.0 * wear_factor;
        self.age_tot += 1.0 * wear_factor;
    }

    /// t_add_tireset zwraca obecną utratę czasu z powodu degradacji opon.
    /// Usunięto logikę 'zimnych opon'.
    pub fn t_add_tireset(&self, degr_pars: &DegrPars, tire_cfg: &TireConfig) -> f64 {
        self.calc_tire_degr(degr_pars, tire_cfg)
    }

    /// calc_tire_degr zwraca deltę czasu degradacji opon.
    ///
    /// * `model liniowy`: t_tire_degr = k_0 + k_1_lin * age
    ///
    /// `age` to całkowity wiek opon w okrążeniach na starcie bieżącego okrążenia.
    fn calc_tire_degr(&self, degr_pars: &DegrPars, tire_cfg: &TireConfig) -> f64 {
        // Używaj wieku STINTU (age_cur_stint), aby kara za degradację
        // rosła głównie w ramach jednego przejazdu. To sprawia, że brak pit stopów
        // powoduje wyraźnie większą stratę tempa.
        let age = self.age_cur_stint;

        // Globalne skalowanie k_1 dla różnych mieszanek + domyślny 'cliff' i bazowy offset tempa
        // Uwaga: base_offset jest ujemny dla szybszych mieszanek (zysk czasu na świeżym komplecie)
        // Rekomendacje dla Monzy: SOFT ~15 okr., MEDIUM ~28 okr., HARD ~45 okr.
        // Degradacja: SOFT x1.8, MEDIUM x1.0, HARD x0.5
        // Cliff ostrość (k2): SOFT 0.050, MEDIUM 0.020, HARD 0.010
        // Bazowe offsety: SOFT -1.0s, MEDIUM -0.5s, HARD 0.0s
        let cfg = tire_cfg.for_compound(&self.compound);
        let k1_scale = cfg.k1_scale;
        let default_cliff_age = cfg.default_cliff_age;
        let default_k2 = cfg.default_k2;
        let base_offset = cfg.base_offset;

        // Pozostał tylko model liniowy
        match degr_pars.degr_model {
            DegrModel::Lin => {
                // Wersja liniowa z dodatkowym domyślnym cliffem po długim stincie
                // Wynik: bazowy offset mieszanki + (k_0 + k_1 * age) + ewentualny cliff
                let linear_degr = degr_pars.k_0 + (degr_pars.k_1_lin * k1_scale) * age;
                let cliff_penalty = if age > default_cliff_age {
                    let over = age - default_cliff_age;
                    (default_k2 * over.powf(2.0)).min(MAX_TIRE_PENALTY)
                } else {
                    0.0
                };
                base_offset + linear_degr + cliff_penalty
            },

            DegrModel::NonlinWithCliff => {
                let mut degradation = degr_pars.k_0 + degr_pars.k_1_lin * age;

                let cliff_age = degr_pars.cliff_age.unwrap_or(default_cliff_age);
                let k_2 = degr_pars.k_2_cliff.unwrap_or(default_k2);

                if age > cliff_age {
                    let over_cliff = age - cliff_age;
                    let cliff_penalty = k_2* over_cliff.powf(2.0);
                    degradation += cliff_penalty.min(MAX_TIRE_PENALTY);
                }
                // Dodaj bazowy offset mieszanki również dla modelu nieliniowego
                base_offset + degradation
            }
        }
    }
}