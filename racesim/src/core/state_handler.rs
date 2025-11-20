// Usunięto FlagState, ponieważ interakcje są usunięte
// use crate::core::race::FlagState; 

#[derive(Debug)]
pub enum State {
    OnTrack, // Uproszczono: Racestart, NormalZone, OvertakingZone połączone
    Pitlane,
    PitStandstill,
}

/// StateHandler został drastycznie uproszczony.
/// Śledzi teraz tylko postęp na torze i podstawowe stany (tor, aleja, postój).
/// Usunięto całą logikę DRS, wyprzedzania, pojedynków i stref.
#[derive(Debug)]
pub struct StateHandler {
    // parametry
    pit_zone: [f64; 2], // [start, end]
    track_length: f64,
    use_drs: bool,
    drs_blocked_s: f64, // turn_1
    drs_window: f64,
    drs_measurement_points: Vec<f64>,
    overtaking_zones: Vec<[f64; 2]>,
    corners: Vec<[f64; 2]>,

    // zmienne związane z postępem na torze
    s_track_prev: f64,
    s_track_cur: f64,
    // zmienne związane z maszyną stanów
    state: State,
    t_standstill: f64, // czas postoju
    t_standstill_target: f64, // docelowy czas postoju
    pub pit_act: bool,
    pub pit_standstill_act: bool,
    pub drs_act: bool,
    pub duel_act: bool,
    pub corner_act: bool,
    // zmienne związane z postępem wyścigu
    compl_lap_prev: u32,
    compl_lap_cur: u32,
}

impl StateHandler {
    #[allow(clippy::too_many_arguments)]
    pub fn initialize_state_handler(
        &mut self,
        use_drs: bool,
        drs_blocked_s: f64,
        drs_window: f64,
        s_track_start: f64,
        track_length: f64,
        drs_measurement_points: Vec<f64>,
        pit_zone: [f64; 2],
        overtaking_zones: Vec<[f64; 2]>,
        corners: Vec<[f64; 2]>,
    ) {
        // inicjalizacja parametrów
        self.pit_zone = pit_zone;
        self.track_length = track_length;
        self.use_drs = use_drs;
        self.drs_blocked_s = drs_blocked_s;
        self.drs_window = drs_window;
        self.drs_measurement_points = drs_measurement_points;
        self.overtaking_zones = overtaking_zones;
        self.corners = corners;

        // inicjalizacja zmiennych pozycji s
        self.s_track_prev = s_track_start;
        self.s_track_cur = s_track_start;
        
        // Usunięto logikę 'first_zone_info'
    }

    pub fn get_s_track_passed_this_step(&self, s_track: f64) -> bool {
        // Sprawdza, czy bolid przekroczył linię mety w tym kroku czasowym
        let new_lap = self.get_new_lap();

        // Sprawdza, czy bolid minął dany koordynat s w tym kroku
        if (self.s_track_prev < s_track || new_lap) && s_track <= self.s_track_cur
            || self.s_track_prev < s_track && (s_track <= self.s_track_cur || new_lap)
        {
            return true;
        }
        false
    }

    /// check_state_transition sprawdza, czy bolid zmienia stan (tylko wejście/wyjście z alei).
    /// Drastycznie uproszczone: usunięto logikę DRS, wyprzedzania, flag, okrążeń.
    #[allow(clippy::too_many_arguments)]
    pub fn check_state_transition(
        &mut self,
        delta_t_front: f64,
        delta_t_rear: f64,
        pit_this_lap: bool,
    ) {
        // Prosta logika pojedynków: jeśli jesteśmy blisko kogoś (z przodu lub z tyłu), to walczymy
        if delta_t_front < 1.0 || delta_t_rear < 1.0 {
            self.duel_act = true;
        } else {
            self.duel_act = false;
        }

        // Sprawdź czy jesteśmy w zakręcie
        self.corner_act = false;
        for corner in &self.corners {
            if self.get_s_track_passed_this_step(corner[0]) || 
               (self.s_track_cur >= corner[0] && self.s_track_cur <= corner[1]) {
                self.corner_act = true;
                break;
            }
        }

        match self.state {
            // Bolid jest na torze (łączy Racestart, NormalZone, OvertakingZone)
            State::OnTrack => {
                if pit_this_lap && self.get_s_track_passed_this_step(self.pit_zone[0]) {
                    self.state = State::Pitlane;
                    self.pit_act = true;
                }
            }

            // Bolid jest w alei serwisowej
            State::Pitlane => {
                if self.get_s_track_passed_this_step(self.pit_zone[1]) {
                    // Wyjazd z alei, powrót na tor
                    self.state = State::OnTrack; 
                    self.pit_act = false;
                }
            }

            // Postój w alei (obsługiwany przez metody zewnętrzne)
            State::PitStandstill => {}
        }
    }

    /// act_pit_standstill aktywuje stan postoju
    pub fn act_pit_standstill(&mut self, t_standstill: f64, t_standstill_target: f64) {
        if !matches!(self.state, State::Pitlane) {
            panic!("Tried to enter pit standstill state without being in pit state!")
        }

        self.state = State::PitStandstill;
        self.pit_standstill_act = true;
        self.t_standstill = t_standstill;
        self.t_standstill_target = t_standstill_target;
    }

    /// deact_pit_standstill deaktywuje stan postoju
    pub fn deact_pit_standstill(&mut self) {
        if !matches!(self.state, State::PitStandstill) {
            panic!("Tried to revert to pit state without being in pit standstill state!")
        }

        self.state = State::Pitlane;
        self.pit_standstill_act = false;
        self.t_standstill = 0.0;
        self.t_standstill_target = 0.0;
    }

    /// increment_t_standstill inkrementuje czas postoju
    pub fn increment_t_standstill(&mut self, timestep_size: f64) {
        if !matches!(self.state, State::PitStandstill) {
            panic!("Tried to increment standstill time without being in standstill state!")
        }

        self.t_standstill += timestep_size;
    }

    /// check_leaves_standstill sprawdza, czy bolid opuszcza postój
    pub fn check_leaves_standstill(&self, timestep_size: f64) -> Option<f64> {
        if !matches!(self.state, State::PitStandstill) {
            panic!("Tried to check if car leaves standstill without being in standstill state!")
        }

        if self.t_standstill + timestep_size <= self.t_standstill_target {
            None
        } else {
            Some(self.t_standstill + timestep_size - self.t_standstill_target)
        }
    }

    // Usunięto get_act_state_and_zone (już niepotrzebne)

    /// get_lap_fracs zwraca ułamki okrążenia (poprzedni i obecny)
    pub fn get_lap_fracs(&self) -> (f64, f64) {
        let lap_frac_prev = if self.s_track_prev < 0.0 {
            (self.s_track_prev + self.track_length) / self.track_length
        } else {
            self.s_track_prev / self.track_length
        };

        let lap_frac_cur = if self.s_track_cur < 0.0 {
            (self.s_track_cur + self.s_track_cur) / self.track_length
        } else {
            self.s_track_cur / self.track_length
        };

        (lap_frac_prev, lap_frac_cur)
    }

    /// get_s_tracks zwraca koordynaty s (poprzedni i obecny)
    pub fn get_s_tracks(&self) -> (f64, f64) {
        let s_track_prev = if self.s_track_prev < 0.0 {
            self.s_track_prev + self.track_length
        } else {
            self.s_track_prev
        };

        let s_track_cur = if self.s_track_cur < 0.0 {
            self.s_track_cur + self.s_track_cur
        } else {
            self.s_track_cur
        };

        (s_track_prev, s_track_cur)
    }

    /// get_compl_lap zwraca liczbę ukończonych okrążeń
    pub fn get_compl_lap(&self) -> u32 {
        self.compl_lap_cur
    }

    /// get_race_prog zwraca obecny postęp wyścigu
    pub fn get_race_prog(&self) -> f64 {
        self.compl_lap_cur as f64 + self.s_track_cur / self.track_length
    }

    /// get_new_lap zwraca bool, czy rozpoczęto nowe okrążenie
    pub fn get_new_lap(&self) -> bool {
        self.compl_lap_cur > self.compl_lap_prev
    }

    /// set_s_track ustawia koordynat s
    pub fn set_s_track(&mut self, s_track_cur: f64) {
        if !(0.0 <= s_track_cur && s_track_cur < self.track_length) {
            panic!(
                "Distance s_track_cur must be in [0.0, track_length[, but is {:.3}m!",
                s_track_cur
            )
        }
        self.s_track_cur = s_track_cur;
    }

    /// update_race_prog aktualizuje postęp wyścigu
    pub fn update_race_prog(&mut self, cur_laptime: f64, timestep_size: f64) {
        // update poprzedniego stanu
        self.compl_lap_prev = self.compl_lap_cur;
        self.s_track_prev = self.s_track_cur;

        // update obecnego stanu
        self.s_track_cur += timestep_size / cur_laptime * self.track_length;

        // sprawdza, czy rozpoczęto nowe okrążenie
        if self.s_track_cur >= self.track_length {
            self.compl_lap_cur += 1;
            self.s_track_cur -= self.track_length;
        }
    }
}

impl Default for StateHandler {
    fn default() -> Self {
        StateHandler {
            pit_zone: [0.0, 0.0],
            track_length: 0.0,
            s_track_prev: 0.0,
            s_track_cur: 0.0,
            state: State::OnTrack, // Domyślny stan to OnTrack
            t_standstill: 0.0,
            t_standstill_target: 0.0,
            pit_act: false,
            pit_standstill_act: false,
            drs_act: false,
            duel_act: false,
            corner_act: false,
            compl_lap_prev: 0,
            compl_lap_cur: 0,
            use_drs: false,
            drs_blocked_s: 0.0,
            drs_window: 0.0,
            drs_measurement_points: Vec::new(),
            overtaking_zones: Vec::new(),
            corners: Vec::new(),
        }
    }
}