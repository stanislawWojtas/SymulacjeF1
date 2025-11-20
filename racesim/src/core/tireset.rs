use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum DegrModel {
    Lin,
}

/// * `degr_model` - Uproszczony model degradacji -> tylko lin (liniowy)
/// * `k_0` - (s) Parametr degradacji -> offset dla świeżych opon
/// * `k_1_lin` - (s/lap) Parametr degradacji (model liniowy)
#[derive(Debug, Deserialize, Clone)]
pub struct DegrPars {
    pub degr_model: DegrModel,
    pub k_0: f64,
    pub k_1_lin: f64, // Usunięto Option<>, teraz jest wymagane dla modelu liniowego
}

#[derive(Debug)]
pub struct Tireset {
    pub compound: String,
    pub age_tot: u32,
    pub age_cur_stint: u32,
}

impl Tireset {
    pub fn new(compound: String, age_tot: u32) -> Tireset {
        Tireset {
            compound,
            age_tot,
            age_cur_stint: 0,
        }
    }

    /// drive_lap zwiększa wiek opon o jedno okrążenie.
    pub fn drive_lap(&mut self) {
        self.age_cur_stint += 1;
        self.age_tot += 1;
    }

    /// t_add_tireset zwraca obecną utratę czasu z powodu degradacji opon.
    /// Usunięto logikę 'zimnych opon'.
    pub fn t_add_tireset(&self, degr_pars: &DegrPars) -> f64 {
        self.calc_tire_degr(degr_pars)
    }

    /// calc_tire_degr zwraca deltę czasu degradacji opon.
    ///
    /// * `model liniowy`: t_tire_degr = k_0 + k_1_lin * age
    ///
    /// `age` to całkowity wiek opon w okrążeniach na starcie bieżącego okrążenia.
    fn calc_tire_degr(&self, degr_pars: &DegrPars) -> f64 {
        let age_tot = self.age_tot as f64;

        // Pozostał tylko model liniowy
        match degr_pars.degr_model {
            DegrModel::Lin => degr_pars.k_0 + degr_pars.k_1_lin * age_tot,
        }
    }
}