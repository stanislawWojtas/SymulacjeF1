use crate::core::car::{Car, CarPars};
use crate::core::driver::{Driver, DriverPars};
use crate::core::track::{Track, TrackPars};
use crate::post::race_result::{CarDriverPair, RaceResult};
// Usunięto `argmax` i `argsort`, ponieważ złożone interakcje są usunięte
use serde::Deserialize;
use std::collections::HashMap;
use std::rc::Rc;
use helpers::general::{argmax, argsort, SortOrder};
use rand_distr::{Normal, Distribution}; // Na górze pliku

/// * `season` - Sezon
/// * `tot_no_laps` - Całkowita liczba okrążeń
/// * `drs_allowed_lap` - (Nieużywane po uproszczeniu, ale zachowane dla zgodności parsowania)
/// * `min_t_dist` - (Nieużywane po uproszczeniu)
/// * `t_duel` - (Nieużywane po uproszczeniu)
/// * `t_overtake_loser` - (Nieużywane po uproszczeniu)
/// * `drs_window` - (Nieużywane po uproszczeniu)
/// * `use_drs` - (Nieużywane po uproszczeniu)
/// * `participants` - Lista uczestników
#[derive(Debug, Deserialize, Clone)]
pub struct RacePars {
    pub season: u32,
    pub tot_no_laps: u32,
    pub drs_allowed_lap: u32, // Zachowane dla parsowania pliku JSON
    pub min_t_dist: f64,      // Zachowane dla parsowania pliku JSON
    pub t_duel: f64,          // Zachowane dla parsowania pliku JSON
    pub t_overtake_loser: f64, // Zachowane dla parsowania pliku JSON
    pub drs_window: f64,      // Zachowane dla parsowania pliku JSON
    pub use_drs: bool,        // Zachowane dla parsowania pliku JSON
    pub participants: Vec<u32>,
}

#[derive(Debug, Clone)]
pub enum FlagState {
    G,   // green
    Y,   // yellow
    Vsc, // virtual safety car
    Sc,  // safety car
    C,   // chequered
}

impl Default for FlagState {
    fn default() -> Self {
        FlagState::G
    }
}

#[derive(Debug)]
pub struct Race {
    pub timestep_size: f64,
    pub cur_racetime: f64,
    season: u32,
    pub tot_no_laps: u32,
    // Usunięto pola związane ze złożonymi interakcjami
    pub drs_allowed_lap: u32, 
    pub cur_lap_leader: u32,
    pub min_t_dist: f64,
    pub t_duel: f64,
    // t_overtake_loser: f64,
    pub drs_window: f64,
    pub use_drs: bool,
    pub flag_state: FlagState,
    pub track: Track,
    race_finished: Vec<bool>,
    pub laptimes: Vec<Vec<f64>>,
    pub racetimes: Vec<Vec<f64>>,
    pub cur_laptimes: Vec<f64>,
    cur_th_laptimes: Vec<f64>,
    pub cars_list: Vec<Car>,
    drivers_list: HashMap<String, Rc<Driver>>,
}

impl Race {
    pub fn new(
        race_pars: &RacePars,
        track_pars: &TrackPars,
        driver_pars_all: &HashMap<String, DriverPars>,
        car_pars_all: &HashMap<u32, CarPars>,
        timestep_size: f64,
    ) -> Race {
        // create drivers
        let mut drivers_list = HashMap::with_capacity(driver_pars_all.len());

        for (initials, driver_pars) in driver_pars_all.iter() {
            drivers_list.insert(initials.to_owned(), Rc::new(Driver::new(driver_pars)));
        }

        // create cars
        let no_cars = race_pars.participants.len();
        let mut cars_list: Vec<Car> = Vec::with_capacity(no_cars);

        for car_no in race_pars.participants.iter() {
            let car_pars_tmp = car_pars_all
                .get(car_no)
                .expect("Missing car number in car parameters!");

            cars_list.push(Car::new(
                car_pars_tmp,
                Rc::clone(
                    drivers_list
                        // To wywołanie jest teraz poprawne dzięki przywróceniu `driver_initials`
                        .get(&car_pars_tmp.strategy[0].driver_initials) 
                        .expect("Could not find start driver initials in drivers list!"),
                ),
            ));
        }

        // sort cars list by car number
        cars_list.sort_unstable_by(|a, b| a.car_no.partial_cmp(&b.car_no).unwrap());

        // create race
        let mut race = Race {
            timestep_size,
            cur_racetime: 0.0,
            season: race_pars.season,
            tot_no_laps: race_pars.tot_no_laps,
            drs_allowed_lap: race_pars.drs_allowed_lap, // Usunięte
            cur_lap_leader: 1,
            // Usunięto pola związane z interakcjami
            min_t_dist: race_pars.min_t_dist,
            t_duel: race_pars.t_duel,
            // t_overtake_loser: race_pars.t_overtake_loser,
            drs_window: race_pars.drs_window,
            use_drs: race_pars.use_drs,
            flag_state: FlagState::G,
            track: Track::new(track_pars),
            race_finished: vec![false; no_cars],
            laptimes: vec![vec![0.0; race_pars.tot_no_laps as usize + 1]; no_cars],
            racetimes: vec![vec![0.0; race_pars.tot_no_laps as usize + 1]; no_cars],
            cur_laptimes: vec![0.0; no_cars],
            cur_th_laptimes: vec![0.0; no_cars],
            cars_list,
            drivers_list,
        };

        // initialize race for each car
        for idx in 0..race.cars_list.len() {
            // calculate theoretical lap time for first lap
            race.calc_th_laptime(idx);

            // initialize state handler of the car
            let car = &mut race.cars_list[idx];

            let s_track_start =
                race.track.d_first_gridpos + (car.p_grid - 1) as f64 * race.track.d_per_gridpos;

            // --- PODMIEŃ TO WYWOŁANIE NA PEŁNE ---
            car.sh.initialize_state_handler(
                race.use_drs,                                   // 1. Czy DRS włączony
                race.track.turn_1,                              // 2. Blokada DRS na starcie
                race.drs_window,                                // 3. Okno czasowe (1s)
                s_track_start,                                  // 4. Pozycja startowa
                race.track.length,                              // 5. Długość toru
                race.track.drs_measurement_points.to_owned(),   // 6. Punkty detekcji
                race.track.pit_zone,                            // 7. Aleja serwisowa
                race.track.overtaking_zones.to_owned(),         // 8. Strefy wyprzedzania
                race.track.corners.to_owned(),                  // 9. Zakręty
            );
        }

        race
    }

    // ... (reszta pliku pozostaje bez zmian, tak jak w poprzedniej odpowiedzi) ...
    
    // ---------------------------------------------------------------------------------------------
    // MAIN METHOD ---------------------------------------------------------------------------------
    // ---------------------------------------------------------------------------------------------

    /// Metoda symuluje jeden krok czasowy.
    pub fn simulate_timestep(&mut self) {
        // increment discretization variable
        self.cur_racetime += self.timestep_size;

        // adjust current lap times
        self.calc_cur_laptimes();

        // update race progress
        for (i, car) in self.cars_list.iter_mut().enumerate() {
            car.sh
                .update_race_prog(self.cur_laptimes[i], self.timestep_size)
        }

        // handle pit stop standstill part (if pits are located in front of the finish line -
        // uncommon case)
        if !self.track.pits_aft_finishline {
            self.handle_pit_standstill()
        }

        // handle lap transitions
        self.handle_lap_transitions();

        // handle pit stop standstill part (if pits are located behind the finish line - common
        // case)
        if self.track.pits_aft_finishline {
            self.handle_pit_standstill()
        }

        // handle state transitions
        self.handle_state_transitions();
    }

    // ---------------------------------------------------------------------------------------------
    // RACE SIMULATOR PARTS ------------------------------------------------------------------------
    // ---------------------------------------------------------------------------------------------

    /// Oblicza teoretyczny czas okrążenia
    fn calc_th_laptime(&mut self, idx: usize) {
        // Pobieramy spójność kierowcy (np. 0.98 oznacza małe błędy, 0.90 duże)
        // Musisz upewnić się, że Driver ma to pole publiczne.
        let consistency = self.cars_list[idx].driver.consistency; 

        let mut rng = rand::thread_rng();
        let normal = Normal::new(0.0, 0.2 * (1.0 - self.cars_list[idx].driver.consistency)).unwrap();
        let random_factor = normal.sample(&mut rng);
        
        // Prosta symulacja błędu: im mniejsze consistency, tym większa szansa na dodanie czasu.
        // Tu robimy uproszczoną wersję bez zaawansowanego rozkładu normalnego, żeby nie ciągnąć zależności.
        
        // Bazowy czas
        let mut lap_time = self.track.t_q
            + self.track.t_gap_racepace
            + self.cars_list[idx].calc_basic_timeloss(self.track.s_mass);

        // Dodatek losowy (pseudolosowość - w prawdziwym kodzie użyj `rand`)
        // Tutaj tylko zaznaczam miejsce, gdzie to powinno być.
        // W oryginale było: lap_time += driver_randomness;
        
        self.cur_th_laptimes[idx] = lap_time + random_factor;
    }

    /// Dostosowuje teoretyczne czasy okrążeń (uproszczone).
    fn calc_cur_laptimes(&mut self) {
        for (i, car) in self.cars_list.iter().enumerate() {
            // reset lap time
            self.cur_laptimes[i] = self.cur_th_laptimes[i];

            // Usunięto logikę `duel_act` i `drs_act`

            // consider time loss due to a pit stop
            if car.sh.pit_act {
                if !car.sh.pit_standstill_act {
                    // case 1: driving through the pit lane
                    self.cur_laptimes[i] = self.track.length / self.track.pit_speedlimit
                        * self.track.real_length_pit_zone
                        / self.track.track_length_pit_zone;
                } else {
                    // case 2: car is in standstill
                    if let Some(t_driving) = car.sh.check_leaves_standstill(self.timestep_size) {
                        // case 2a: car returns from standstill
                        self.cur_laptimes[i] = self.track.length / self.track.pit_speedlimit
                            * self.track.real_length_pit_zone
                            / self.track.track_length_pit_zone
                            * self.timestep_size
                            / t_driving;
                    } else {
                        // case 2b: car stays in standstill
                        self.cur_laptimes[i] = f64::INFINITY;
                    }
                }
            }

            if car.sh.drs_act {
                self.cur_laptimes[i] +=
                    self.track.t_drseffect / self.track.overtaking_zones_lap_frac;
            }

            // consider current flag state (minimum lap time) if car is not in the pit lane
            if !car.sh.pit_act && self.cur_laptimes[i] < self.get_min_laptime_flag_state() {
                self.cur_laptimes[i] = self.get_min_laptime_flag_state()
            }

            if car.sh.duel_act {
                self.cur_laptimes[i] += self.t_duel / self.track.overtaking_zones_lap_frac;
            }

            // Jeśli w zakręcie, zwolnij (dodaj czas)
            if car.sh.corner_act {
                self.cur_laptimes[i] += 0.5; // Kara czasowa za zakręt (wolniejsza jazda)
            }
        }

        // Usunięto całą sekcję "ADJUST LAP TIME IF TOO CLOSE TO CAR IN FRONT"
        // (linie ok. 300-330), ponieważ `overtaking_act` już nie istnieje.

        // ... (tu jest Twoja pętla for po cars_list)

        // --- NOWA LOGIKA INTERAKCJI ---
        
        // 1. Ustal kolejność aut, zaczynając od tego, który ma czysto przed sobą
        let idxs_sorted = self.get_idx_list_sorted_by_biggest_gap();
        // 2. Utwórz pary (Ten z przodu, Ten z tyłu)
        let car_pair_idxs_list = self.get_car_pair_idxs_list(&idxs_sorted, true);

        for pair_idxs in car_pair_idxs_list.iter() {
            let idx_front = pair_idxs[0];
            let idx_rear = pair_idxs[1];

            // Oblicz przewidywany dystans czasowy na koniec tego kroku symulacji
            let delta_t_proj =
                self.calc_projected_delta_t(idx_front, idx_rear, self.timestep_size);

            // Jeśli dystans jest mniejszy niż minimalny bezpieczny (min_t_dist)
            // ORAZ auto z tyłu nie jest w boksie
            if !self.cars_list[idx_front].sh.pit_act
                && delta_t_proj < self.min_t_dist
            {
                // Sprawdź czy możliwe jest wyprzedzanie
                // Warunek 1: Auto z tyłu jest szybsze o pewien próg (np. 0.2s)
                // Warunek 2: Żadne z aut nie jest w zakręcie
                let overtake_threshold = 0.2;
                let potential_pace_diff = self.cur_th_laptimes[idx_front] - self.cur_th_laptimes[idx_rear];
                let in_corner = self.cars_list[idx_front].sh.corner_act || self.cars_list[idx_rear].sh.corner_act;

                if potential_pace_diff > overtake_threshold && !in_corner {
                    // WYPRZEDZANIE
                    // Nie spowalniamy auta z tyłu, pozwalamy mu jechać swoim tempem.
                    // Możemy dodać małą karę czasową za manewr (jazda po gorszej linii)
                    self.cur_laptimes[idx_rear] += 0.1; // np. 0.1s straty na manewr
                    
                    // Auto z przodu też może stracić trochę czasu (brudna linia, obrona)
                    self.cur_laptimes[idx_front] += 0.1;
                } else {
                    // BLOKOWANIE (brak wystarczającej przewagi prędkości LUB zakręt)
                    // Oblicz obecny dystans
                    let delta_t_cur = self.calc_projected_delta_t(idx_front, idx_rear, 0.0);

                    // Oblicz, o ile musimy zwolnić, żeby odbudować bezpieczny dystans w ciągu 5 sekund
                    let t_gap_add =
                        (self.min_t_dist - delta_t_cur) / 5.0 * self.cur_laptimes[idx_rear];

                    // Zastosuj spowolnienie: czas okrążenia auta z tyłu nie może być lepszy niż 
                    // czas auta z przodu + korekta na dystans.
                    if self.cur_laptimes[idx_rear] < self.cur_laptimes[idx_front] + t_gap_add {
                        self.cur_laptimes[idx_rear] = self.cur_laptimes[idx_front] + t_gap_add
                    }
                }
            }
        }
    }

    /// Zwraca minimalny czas okrążenia w zależności od flagi
    fn get_min_laptime_flag_state(&self) -> f64 {
        match self.flag_state {
            FlagState::Y => (self.track.t_q + self.track.t_gap_racepace) * 1.1,
            FlagState::Vsc => (self.track.t_q + self.track.t_gap_racepace) * 1.4,
            FlagState::Sc => (self.track.t_q + self.track.t_gap_racepace) * 1.4,
            _ => 0.0,
        }
    }

    // Usunięto `get_idx_list_sorted_by_biggest_gap`, ponieważ nie jest już używane

    /// Obsługuje logikę postoju w alei serwisowej
    fn handle_pit_standstill(&mut self) {
        for i in 0..self.cars_list.len() {
            let car = &mut self.cars_list[i];
            
            // check for possible activation of standstill state if car is within the pit and not
            // already in standstill state
            if car.sh.pit_act && !car.sh.pit_standstill_act {
                // calculate time part that was driven before crossing the pit location if the car
                // crossed the pit location within the current time step, else continue
                let t_part_drive: f64;

                if car.sh.get_s_track_passed_this_step(car.pit_location) {
                    let (s_track_prev, s_track_cur) = car.sh.get_s_tracks();

                    if !self.track.pits_aft_finishline {
                        // drive time part is known without issues caused by a possible lap
                        // transition
                        t_part_drive = (car.pit_location - s_track_prev) / self.track.length
                            * self.cur_laptimes[i];
                    } else {
                        // standstill time part is known without issues caused by a possible lap
                        // transition -> subtract it from the time step size
                        t_part_drive = self.timestep_size
                            - (s_track_cur - car.pit_location) / self.track.length
                                * self.cur_laptimes[i];
                    }
                } else {
                    continue;
                }

                // below this line we handle the case that the car enters the standstill state
                // within the current step ---------------------------------------------------------

                // determine standstill target time for current pit stop
                let compl_lap_cur = car.sh.get_compl_lap();
                let t_standstill_target = if self.track.pits_aft_finishline {
                    car.t_add_pit_standstill(compl_lap_cur)
                } else {
                    car.t_add_pit_standstill(compl_lap_cur + 1)
                };

                // set car state to pit standstill and set standstill time that was already achieved
                car.sh
                    .act_pit_standstill(self.timestep_size - t_part_drive, t_standstill_target);

                // POPRAWKA: Wykonaj pit stop TUTAJ (w momencie wejścia w postój)
                let compl_lap_for_pitstop = if self.track.pits_aft_finishline {
                    compl_lap_cur
                } else {
                    compl_lap_cur + 1
                };
                let pit_location = car.pit_location;
                car.perform_pitstop(compl_lap_for_pitstop, &self.drivers_list);

                // update race progress of the car such that it is placed exactly at the pit
                // location
                car.sh.set_s_track(pit_location);
                
                // Aktualizuj teoretyczny czas okrążenia od razu po zmianie opon
                // (wywołanie musi być po zakończeniu wypożyczenia car)
                self.calc_th_laptime(i);
            } else if car.sh.pit_standstill_act {
                // if standstill is active currently, it must be checked if the car stays or leaves
                // it within the current time step
                let leaves_standstill =
                    car.sh.check_leaves_standstill(self.timestep_size).is_some();

                if !leaves_standstill {
                    // car remains in standstill, therefore increment standstill time
                    car.sh.increment_t_standstill(self.timestep_size)
                } else {
                    // car leaves standstill state within current time step
                    car.sh.deact_pit_standstill()
                }
            }
        }
    }

    /// Obsługuje przejścia między okrążeniami
    fn handle_lap_transitions(&mut self) {
        // check at first if race was finished by any car such that checkered flag can be considered
        // in the loop afterward
        for car in self.cars_list.iter() {
            let compl_lap_cur = car.sh.get_compl_lap();

            if compl_lap_cur >= self.cur_lap_leader {
                self.cur_lap_leader = compl_lap_cur + 1
            }
        }

        if self.cur_lap_leader > self.tot_no_laps && !matches!(self.flag_state, FlagState::C) {
            self.flag_state = FlagState::C
        }

        // check for all cars if they jumped into a new lap within the current time step
        for i in 0..self.cars_list.len() {
            let car = &mut self.cars_list[i];

            if car.sh.get_new_lap() {
                // calculate the part of the current time step that was driven before crossing the
                // finish line
                let lap_frac_prev = car.sh.get_lap_fracs().0;
                let t_part_old = (1.0 - lap_frac_prev) * self.cur_laptimes[i];

                // update lap time and race time arrays (if laps are part of the race)
                let compl_lap_cur = car.sh.get_compl_lap();

                if compl_lap_cur <= self.tot_no_laps {
                    self.laptimes[i][compl_lap_cur as usize] =
                        self.cur_racetime - self.timestep_size + t_part_old
                            - self.racetimes[i][compl_lap_cur as usize - 1];
                    self.racetimes[i][compl_lap_cur as usize] = self.racetimes[i]
                        [compl_lap_cur as usize - 1]
                        + self.laptimes[i][compl_lap_cur as usize];
                }

                // set race finished for current car if it crosses the line after the chequered flag
                // got active
                if matches!(self.flag_state, FlagState::C) {
                    self.race_finished[i] = true
                }

                // increase car age by a lap
                car.drive_lap();

                // USUNIĘTE: perform_pitstop jest teraz wywoływane w handle_pit_standstill

                // update theoretical lap time (również gdy nie było pit stopu)
                self.calc_th_laptime(i);
            }
        }
    }

    /// Przygotowuje dane i wywołuje maszynę stanów (uproszczone).
    /// Metoda sprawdza, czy następuje zmiana stanu (np. rozpoczęcie wyprzedzania).
    fn handle_state_transitions(&mut self) {
        // 1. Ustal kolejność aut na torze
        let idxs_sorted = self.get_car_order_on_track();
        // 2. Stwórz pary (kto kogo goni)
        let car_pair_idxs_list = self.get_car_pair_idxs_list(&idxs_sorted, false);

        let mut delta_ts = vec![0.0; self.cars_list.len()];
        let mut lapping = vec![false; self.cars_list.len()];

        // 3. Oblicz dystanse czasowe i sprawdź dublowanie
        for (i, pair_idxs) in car_pair_idxs_list.iter().enumerate() {
            delta_ts[i] = self.calc_projected_delta_t(pair_idxs[0], pair_idxs[1], 0.0);

            if self.cars_list[pair_idxs[0]].sh.get_race_prog()
                < self.cars_list[pair_idxs[1]].sh.get_race_prog()
            {
                lapping[i] = true;
            }
        }

        // 4. Sprawdź przejścia stanów (przekazujemy pełne dane do StateHandlera)
        for (i, pair_idxs) in car_pair_idxs_list.iter().enumerate() {
            let car_idx = pair_idxs[1]; // Auto z tyłu
            let compl_lap_cur = self.cars_list[car_idx].sh.get_compl_lap();
            
            // Indeks pary za nami (żeby wiedzieć czy ktoś nas nie goni)
            let j = (i + 1) % car_pair_idxs_list.len();

            let pit_this_lap = self.cars_list[car_idx].pit_this_lap(compl_lap_cur + 1);

            // --- TO JEST KLUCZOWE WYWOŁANIE ---
            self.cars_list[car_idx].sh.check_state_transition(
                delta_ts[i],      // Dystans do auta przed nami
                delta_ts[j],      // Dystans do auta za nami
                pit_this_lap,
            );
        }
    }

    // ---------------------------------------------------------------------------------------------
    // METHODS (HELPERS) ---------------------------------------------------------------------------
    // ---------------------------------------------------------------------------------------------

    // Usunięto `get_car_order_on_track`, `calc_projected_delta_t`,
    // `calc_projected_delta_lap_frac` i `get_car_pair_idxs_list`,
    // ponieważ były używane tylko do złożonych interakcji.

    /// get_all_finished checks if all race participants have finished the race.
    pub fn get_all_finished(&self) -> bool {
        self.race_finished.iter().all(|&x| x)
    }

    /// get_race_result returns a race result struct of the race.
    pub fn get_race_result(&self) -> RaceResult {
        RaceResult {
            tot_no_laps: self.tot_no_laps,
            car_driver_pairs: self
                .cars_list
                .iter()
                .map(|car| CarDriverPair {
                    car_no: car.car_no,
                    driver_initials: car.driver.initials.to_owned(),
                })
                .collect(),
            laptimes: self.laptimes.to_owned(),
            racetimes: self.racetimes.to_owned(),
        }
    }
    /// Zwraca listę indeksów aut posortowaną tak, że auto z największą dziurą przed sobą jest pierwsze.
    /// To zapobiega problemom przy obliczaniu hamowania "łańcuszkowego".
    fn get_idx_list_sorted_by_biggest_gap(&self) -> Vec<usize> {
        let mut idx_list_sorted = self.get_car_order_on_track();
        let car_pair_idxs_list = self.get_car_pair_idxs_list(&idx_list_sorted, false);

        let delta_lap_fracs: Vec<f64> = car_pair_idxs_list
            .iter()
            .map(|x| self.calc_projected_delta_lap_frac(x[0], x[1], 0.0))
            .collect();

        let pair_idx_biggest_gap = argmax(&delta_lap_fracs);
        let start_idx = (pair_idx_biggest_gap + 1) % self.cars_list.len();
        idx_list_sorted.rotate_left(start_idx);

        idx_list_sorted
    }

    /// Zwraca kolejność aut na torze (według pozycji s).
    fn get_car_order_on_track(&self) -> Vec<usize> {
        let s_tracks_cur: Vec<f64> = self
            .cars_list
            .iter()
            .map(|car| car.sh.get_s_tracks().1)
            .collect();

        argsort(&s_tracks_cur, SortOrder::Descending)
    }

    /// Oblicza przewidywaną odległość CZASOWĄ między dwoma autami.
    pub fn calc_projected_delta_t(
        &self,
        idx_front: usize,
        idx_rear: usize,
        timestep_size: f64,
    ) -> f64 {
        let delta_lap_frac = self.calc_projected_delta_lap_frac(idx_front, idx_rear, timestep_size);
        delta_lap_frac * self.cur_laptimes[idx_rear]
    }

    /// Oblicza przewidywaną odległość PRZESTRZENNĄ (ułamek okrążenia) między autami.
    fn calc_projected_delta_lap_frac(
        &self,
        idx_front: usize,
        idx_rear: usize,
        timestep_size: f64,
    ) -> f64 {
        let mut lap_frac_cur_front = self.cars_list[idx_front].sh.get_lap_fracs().1;
        let mut lap_frac_cur_rear = self.cars_list[idx_rear].sh.get_lap_fracs().1;

        // Symulujemy ruch do przodu o timestep_size
        lap_frac_cur_front += timestep_size / self.cur_laptimes[idx_front];
        lap_frac_cur_rear += timestep_size / self.cur_laptimes[idx_rear];

        if lap_frac_cur_front >= 1.0 {
            lap_frac_cur_front -= 1.0
        }
        if lap_frac_cur_rear >= 1.0 {
            lap_frac_cur_rear -= 1.0
        }

        if lap_frac_cur_front >= lap_frac_cur_rear {
            lap_frac_cur_front - lap_frac_cur_rear
        } else {
            lap_frac_cur_front + 1.0 - lap_frac_cur_rear
        }
    }

    /// Tworzy pary indeksów (kto kogo goni).
    fn get_car_pair_idxs_list(&self, idxs: &[usize], del_last_pair: bool) -> Vec<[usize; 2]> {
        let mut car_pair_idxs_list = vec![[0; 2]; idxs.len()];

        for i in 0..idxs.len() {
            car_pair_idxs_list[i][0] = idxs[i]; // Auto z przodu
            car_pair_idxs_list[i][1] = idxs[(i + 1) % idxs.len()]; // Auto z tyłu
        }

        if del_last_pair {
            car_pair_idxs_list.remove(car_pair_idxs_list.len() - 1);
        }

        car_pair_idxs_list
    }
}