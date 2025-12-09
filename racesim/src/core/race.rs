use crate::core::car::{Car, CarPars, CarStatus};
use crate::core::driver::{Driver, DriverPars};
use crate::core::track::{Track, TrackPars};
use crate::post::race_result::{CarDriverPair, RaceResult};
use serde::Deserialize;
use core::f64;
use std::collections::HashMap;
use std::f32::INFINITY;
use std::rc::Rc;
use helpers::general::{argmax, argsort, SortOrder};
use rand_distr::{Normal, Distribution}; 
use rand; // Dodano brakujący import do obsługi thread_rng

/// * `season` - Sezon
/// * `tot_no_laps` - Całkowita liczba okrążeń
/// * `drs_allowed_lap` - (Nieużywane po uproszczeniu)
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
    pub drs_allowed_lap: u32, 
    pub min_t_dist: f64,      
    pub t_duel: f64,          
    pub t_overtake_loser: f64, 
    pub drs_window: f64,      
    pub use_drs: bool,        
    pub participants: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct SafetyCar{
    pub active: bool,
    pub s_track: f64,
    pub speed: f64,
    pub lap: u32,
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
impl SafetyCar {
    pub fn new() -> Self{
        SafetyCar { active: false, s_track: 0.0, speed: 70.0, lap: 0 }
    }
}

#[derive(Debug)]
pub struct Race {
    pub sc_timer: f64,
    pub timestep_size: f64,
    pub cur_racetime: f64,
    pub safety_car: SafetyCar,
    sc_triggers: Vec<bool>, // auta które triggerowały safety car żeby w pętli tego nie robiły
    season: u32,
    pub tot_no_laps: u32,
    pub drs_allowed_lap: u32, 
    pub cur_lap_leader: u32,
    pub min_t_dist: f64,
    pub t_duel: f64,
    pub t_overtake_loser: f64,
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
            safety_car: SafetyCar::new(),
            sc_timer: 0.0,
            sc_triggers: vec![false; no_cars], //na start wszystkie false
            season: race_pars.season,
            tot_no_laps: race_pars.tot_no_laps,
            drs_allowed_lap: race_pars.drs_allowed_lap,
            cur_lap_leader: 1,
            min_t_dist: race_pars.min_t_dist,
            t_duel: race_pars.t_duel,
            t_overtake_loser: race_pars.t_overtake_loser,
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

    // ---------------------------------------------------------------------------------------------
    // MAIN METHOD ---------------------------------------------------------------------------------
    // ---------------------------------------------------------------------------------------------

    /// Metoda symuluje jeden krok czasowy.
    pub fn simulate_timestep(&mut self) {
        // increment discretization variable
        self.cur_racetime += self.timestep_size;

        if matches!(self.flag_state, FlagState::Sc){
            self.sc_timer -= self.timestep_size;

            if !self.safety_car.active{
                self.safety_car.active = true;
                // safety car startuje z poziomu lidera
                let leader_idx = self.cars_list.iter().position(|c| c.sh.get_compl_lap() == self.cur_lap_leader - 1).unwrap_or(0);
                self.safety_car.s_track = self.cars_list[leader_idx].sh.get_s_tracks().1 + 500.0; // wjeżdża 500m przed lidera
                self.safety_car.lap = self.cur_lap_leader;
            }

            // przecunięcie SC do przodu
            self.safety_car.s_track += self.safety_car.speed * self.timestep_size;

            if(self.safety_car.s_track > self.track.length) {
                self.safety_car.s_track -= self.track.length;
                self.safety_car.lap +=1;
            }

            if self.sc_timer <= 0.00 {
                println!("SAFETY CAR IN THIS LAP - RACE RESUMING");
                self.flag_state = FlagState::G;
                self.safety_car.active = false;
            }
        } else{
            self.safety_car.active = false;
        }

        let active_sc = matches!(self.flag_state, FlagState::Sc);
        if !active_sc {
            for (i, car) in self.cars_list.iter().enumerate() {
                // Sprawdzamy czy auto ma DNF i czy nie skończyło wyścigu (zabezpieczenie przed ciągłym wywoływaniem SC)
                if car.status == CarStatus::DNF && !self.race_finished[i] && !self.sc_triggers[i] {
                     // Tutaj prosta logika: jak ktoś ma DNF i nie dojechał do mety (czyli rozbił się), wywołaj SC.
                     // W pełnej wersji trzeba by sprawdzać czy ten DNF nastąpił *teraz*.
                    println!("SAFETY CAR DEPLOYED (Caused by car #{}", car.car_no);
                    self.flag_state = FlagState::Sc;
                    self.sc_timer = 180.0; // czas trwania safery Car

                    self.sc_triggers[i] = true; //odchaczamy ten samochód
                    break;
                }
            }
        }

        // adjust current lap times
        self.calc_cur_laptimes();

        // handle state transitions
        self.handle_state_transitions();

        // update race progress
        for (i, car) in self.cars_list.iter_mut().enumerate() {
            car.sh
                .update_race_prog(self.cur_laptimes[i], self.timestep_size)
        }

        // handle pit stop standstill part (uncommon case)
        if !self.track.pits_aft_finishline {
            self.handle_pit_standstill()
        }

        // handle lap transitions
        self.handle_lap_transitions();

        // handle pit stop standstill part (common case)
        if self.track.pits_aft_finishline {
            self.handle_pit_standstill()
        }
    }

    // ---------------------------------------------------------------------------------------------
    // RACE SIMULATOR PARTS ------------------------------------------------------------------------
    // ---------------------------------------------------------------------------------------------

    /// Oblicza teoretyczny czas okrążenia
    fn calc_th_laptime(&mut self, idx: usize) {
        if self.cars_list[idx].status == CarStatus::DNF {
            self.cur_th_laptimes[idx] = f64::INFINITY;
            return;
        }
        let consistency = self.cars_list[idx].driver.consistency; 

        let std_dev = (1.0 - consistency) * 2.0;

        let random_factor = if std_dev > 0.0 {
            let normal = Normal::new(0.0, std_dev).unwrap();
            normal.sample(&mut rand::thread_rng())
        } else {
            0.0
        };
        
        // Bazowy czas
        let lap_time_base = self.track.t_q
        + self.track.t_gap_racepace
        + self.cars_list[idx].calc_basic_timeloss(self.track.s_mass);

        self.cur_th_laptimes[idx] = lap_time_base + random_factor;
    }

    /// Dostosowuje teoretyczne czasy okrążeń (uproszczone).
    fn calc_cur_laptimes(&mut self) {
        // --- CZĘŚĆ 1: PODSTAWOWE OBLICZENIA DLA KAŻDEGO AUTA ---
        for (i, car) in self.cars_list.iter().enumerate() {

            // jezeli jest awaria - POPRAWIONA SKŁADNIA == na =
            if car.status == CarStatus::DNF {
                self.cur_laptimes[i] = f64::INFINITY;
                continue;
            }

            self.cur_laptimes[i] = self.cur_th_laptimes[i];

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

            // consider current flag state
            if !car.sh.pit_act && self.cur_laptimes[i] < self.get_min_laptime_flag_state() {
                self.cur_laptimes[i] = self.get_min_laptime_flag_state()
            }

            if car.sh.duel_act {
                self.cur_laptimes[i] += self.t_duel / self.track.overtaking_zones_lap_frac;
            }

            if car.sh.corner_act {
                self.cur_laptimes[i] += 0.5; // Kara czasowa za zakręt
            }
        }

        // --- CZĘŚĆ 2: NOWA LOGIKA INTERAKCJI (Wyprzedzanie / Blokowanie) ---
        // Uwaga: Musimy rozdzielić odczyt (self) od zapisu (self.cur_laptimes), 
        // aby zadowolić Borrow Checkera w Rust.
        
        let idxs_sorted = self.get_idx_list_sorted_by_biggest_gap();
        let car_pair_idxs_list = self.get_car_pair_idxs_list(&idxs_sorted, true);

        // Bufor na zmiany czasów, aby nie modyfikować `self` w pętli czytającej `self`
        let mut laptimes_updates: Vec<(usize, f64)> = Vec::new();

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
                let overtake_threshold = 0.2;
                let potential_pace_diff = self.cur_th_laptimes[idx_front] - self.cur_th_laptimes[idx_rear];
                let in_corner = self.cars_list[idx_front].sh.corner_act || self.cars_list[idx_rear].sh.corner_act;

                if potential_pace_diff > overtake_threshold && !in_corner {
                    // WYPRZEDZANIE
                    // Zapisujemy zmiany do bufora
                    laptimes_updates.push((idx_rear, 0.1)); // Auto z tyłu traci 0.1s
                    laptimes_updates.push((idx_front, self.t_overtake_loser)); // Auto z przodu traci
                } else {
                    // BLOKOWANIE
                    let delta_t_cur = self.calc_projected_delta_t(idx_front, idx_rear, 0.0);

                    // Oblicz, o ile musimy zwolnić
                    let t_gap_add = (self.min_t_dist - delta_t_cur) / 5.0 * self.cur_laptimes[idx_rear];

                    // Sprawdź czy trzeba zwolnić
                    let target_time = self.cur_laptimes[idx_front] + t_gap_add;
                    if self.cur_laptimes[idx_rear] < target_time {
                         // Oblicz różnicę do dodania
                         let diff = target_time - self.cur_laptimes[idx_rear];
                         laptimes_updates.push((idx_rear, diff));
                    }
                }
            }
        }

        // Aplikujemy zmiany z bufora
        for (idx, time_add) in laptimes_updates {
            self.cur_laptimes[idx] += time_add;
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

    /// Obsługuje logikę postoju w alei serwisowej
    fn handle_pit_standstill(&mut self) {
        for i in 0..self.cars_list.len() {
            let car = &mut self.cars_list[i];
            
            if car.sh.pit_act && !car.sh.pit_standstill_act {
                let t_part_drive: f64;

                if car.sh.get_s_track_passed_this_step(car.pit_location) {
                    let (s_track_prev, s_track_cur) = car.sh.get_s_tracks();

                    if !self.track.pits_aft_finishline {
                        t_part_drive = (car.pit_location - s_track_prev) / self.track.length
                            * self.cur_laptimes[i];
                    } else {
                        t_part_drive = self.timestep_size
                            - (s_track_cur - car.pit_location) / self.track.length
                                * self.cur_laptimes[i];
                    }
                } else {
                    continue;
                }

                let compl_lap_cur = car.sh.get_compl_lap();
                let t_standstill_target = if self.track.pits_aft_finishline {
                    car.t_add_pit_standstill(compl_lap_cur)
                } else {
                    car.t_add_pit_standstill(compl_lap_cur + 1)
                };

                car.sh
                    .act_pit_standstill(self.timestep_size - t_part_drive, t_standstill_target);

                // Pit stop execution
                let compl_lap_for_pitstop = if self.track.pits_aft_finishline {
                    compl_lap_cur
                } else {
                    compl_lap_cur + 1
                };
                let pit_location = car.pit_location;
                car.perform_pitstop(compl_lap_for_pitstop, &self.drivers_list);

                car.sh.set_s_track(pit_location);
                
                // Recalculate theoretical lap time immediately after tire change
                self.calc_th_laptime(i);

            } else if car.sh.pit_standstill_act {
                let leaves_standstill =
                    car.sh.check_leaves_standstill(self.timestep_size).is_some();

                if !leaves_standstill {
                    car.sh.increment_t_standstill(self.timestep_size)
                } else {
                    car.sh.deact_pit_standstill()
                }
            }
        }
    }

    /// Obsługuje przejścia między okrążeniami
    fn handle_lap_transitions(&mut self) {
        for car in self.cars_list.iter() {
            let compl_lap_cur = car.sh.get_compl_lap();

            if compl_lap_cur >= self.cur_lap_leader {
                self.cur_lap_leader = compl_lap_cur + 1
            }
        }

        if self.cur_lap_leader > self.tot_no_laps && !matches!(self.flag_state, FlagState::C) {
            self.flag_state = FlagState::C
        }

        for i in 0..self.cars_list.len() {
            let car = &mut self.cars_list[i];

            if car.sh.get_new_lap() {
                let lap_frac_prev = car.sh.get_lap_fracs().0;
                let t_part_old = (1.0 - lap_frac_prev) * self.cur_laptimes[i];

                let compl_lap_cur = car.sh.get_compl_lap();

                if compl_lap_cur <= self.tot_no_laps {
                    self.laptimes[i][compl_lap_cur as usize] =
                        self.cur_racetime - self.timestep_size + t_part_old
                            - self.racetimes[i][compl_lap_cur as usize - 1];
                    self.racetimes[i][compl_lap_cur as usize] = self.racetimes[i]
                        [compl_lap_cur as usize - 1]
                        + self.laptimes[i][compl_lap_cur as usize];
                }

                if matches!(self.flag_state, FlagState::C) {
                    self.race_finished[i] = true
                }

                car.drive_lap();

                // update theoretical lap time
                self.calc_th_laptime(i);
            }
        }
    }

    /// Przygotowuje dane i wywołuje maszynę stanów (uproszczone).
    fn handle_state_transitions(&mut self) {
        let idxs_sorted = self.get_car_order_on_track();
        let car_pair_idxs_list = self.get_car_pair_idxs_list(&idxs_sorted, false);

        let mut delta_ts = vec![0.0; self.cars_list.len()];
        let mut lapping = vec![false; self.cars_list.len()];

        for (i, pair_idxs) in car_pair_idxs_list.iter().enumerate() {
            delta_ts[i] = self.calc_projected_delta_t(pair_idxs[0], pair_idxs[1], 0.0);

            if self.cars_list[pair_idxs[0]].sh.get_race_prog()
                < self.cars_list[pair_idxs[1]].sh.get_race_prog()
            {
                lapping[i] = true;
            }
        }

        for (i, pair_idxs) in car_pair_idxs_list.iter().enumerate() {
            let car_idx = pair_idxs[1]; 
            let compl_lap_cur = self.cars_list[car_idx].sh.get_compl_lap();
            
            let j = (i + 1) % car_pair_idxs_list.len();

            let pit_this_lap = self.cars_list[car_idx].pit_this_lap(compl_lap_cur + 1);

            self.cars_list[car_idx].sh.check_state_transition(
                delta_ts[i],      
                delta_ts[j],      
                pit_this_lap,
            );
        }
    }

    // ---------------------------------------------------------------------------------------------
    // METHODS (HELPERS) ---------------------------------------------------------------------------
    // ---------------------------------------------------------------------------------------------

    pub fn get_all_finished(&self) -> bool {
        self.race_finished.iter().all(|&x| x)
    }

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
            sc_active: self.safety_car.active,
            sc_position: self.safety_car.s_track,
        }
    }
    
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

    fn get_car_order_on_track(&self) -> Vec<usize> {
        let s_tracks_cur: Vec<f64> = self
            .cars_list
            .iter()
            .map(|car| car.sh.get_s_tracks().1)
            .collect();

        argsort(&s_tracks_cur, SortOrder::Descending)
    }

    pub fn calc_projected_delta_t(
        &self,
        idx_front: usize,
        idx_rear: usize,
        timestep_size: f64,
    ) -> f64 {
        let delta_lap_frac = self.calc_projected_delta_lap_frac(idx_front, idx_rear, timestep_size);
        delta_lap_frac * self.cur_laptimes[idx_rear]
    }

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