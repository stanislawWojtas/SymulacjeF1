use crate::core::car::{Car, CarPars, CarStatus, StrategyEntry};
use crate::core::driver::{Driver, DriverPars};
use crate::core::track::{Track, TrackPars};
use crate::core::tireset::TireConfig;
use crate::post::race_result::{CarDriverPair, RaceEvent, RaceResult};
use serde::Deserialize;
use core::f64;
use std::collections::HashMap;
// use std::f32::INFINITY; // unused
use std::rc::Rc;
use helpers::general::{argmax, argsort, SortOrder};
use rand_distr::{Normal, Distribution}; 
use rand::Rng; // bring Rng trait into scope for thread_rng().gen::<T>()

/// * `season` - Sezon
/// * `tot_no_laps` - Całkowita liczba okrążeń
/// * `drs_allowed_lap` - (Nieużywane po uproszczeniu)
/// * `min_t_dist` - (Nieużywane po uproszczeniu)
/// * `t_duel` - (Nieużywane po uproszczeniu)
/// * `t_overtake_loser` - (Nieużywane po uproszczeniu)
/// * `drs_window` - (Nieużywane po uproszczeniu)
/// * `use_drs` - (Nieużywane po uproszczeniu)
/// * `participants` - Lista uczestników
fn default_initial_weather() -> String { "Dry".to_string() }
fn default_rain_probability() -> f64 { 0.0 }
fn default_min_weather_duration_s() -> f64 { 200.0 }
fn default_fuel_margin() -> f64 { 0.05 }
fn default_failure_rate_per_hour() -> f64 { 0.02 }
fn default_collision_factor() -> f64 { 20.0 }

#[derive(Debug, Deserialize, Clone)]
pub struct RacePars {
    pub season: u32,
    pub tot_no_laps: u32,
    #[serde(default)]
    pub track_name: Option<String>,
    #[serde(default = "default_initial_weather")]
    pub initial_weather: String,
    #[serde(default = "default_rain_probability")]
    pub rain_probability: f64,
    pub drs_allowed_lap: u32, 

    pub use_drs: bool,        
    pub participants: Vec<u32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SimConstants {
    #[serde(default = "default_fuel_margin")] 
    pub fuel_margin: f64,
    #[serde(default = "default_failure_rate_per_hour")] 
    pub failure_rate_per_hour: f64,
    #[serde(default = "default_collision_factor")] 
    pub collision_factor: f64,
    #[serde(default = "default_min_weather_duration_s")] 
    pub min_weather_duration_s: f64,
    pub min_t_dist: f64,
    pub t_duel: f64,
    pub t_overtake_loser: f64,
    pub drs_window: f64,
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

#[derive(Debug, Clone, PartialEq)]
pub enum WeatherState {
    Dry,
    Rain,
}

impl Default for FlagState {
    fn default() -> Self {
        FlagState::G
    }
}
impl SafetyCar {
    pub fn new() -> Self{
        SafetyCar { active: false, s_track: 0.0, speed: 50.0, lap: 0 }
    }
}

#[derive(Debug)]
pub struct Race {
    pub sc_timer: f64,
    pub timestep_size: f64,
    pub weather_state: WeatherState,
    pub print_events: bool,
    rain_probability: f64,
    min_weather_duration_s: f64,
    last_weather_change: f64,
    failure_rate_per_hour: f64,
    collision_factor: f64,
    weather_history_log: Vec<String>,
    events: Vec<RaceEvent>,
    pub cur_racetime: f64,
    pub safety_car: SafetyCar,
    sc_triggers: Vec<bool>, // auta które triggerowały safety car żeby w pętli tego nie robiły
    // Safety Car control
    sc_target_gap_m: f64,
    sc_lineup_tolerance_m: f64,
    sc_release_delay_s: f64,
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
    pub tire_config: TireConfig,
}

impl Race {
    pub fn new(
        race_pars: &RacePars,
        sim_consts: &SimConstants,
        tire_config: &TireConfig,
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

        // Build a robust lookup by actual driver initials (value), not only map key
        let mut drivers_by_initials: HashMap<String, Rc<Driver>> = HashMap::with_capacity(drivers_list.len());
        for driver in drivers_list.values() {
            drivers_by_initials.insert(driver.initials.clone(), Rc::clone(driver));
        }

        // Debug: list available driver initials
        // debug listing removed to avoid noisy output in multi-run scenarios

        // create cars
        let no_cars = race_pars.participants.len();
        let mut cars_list: Vec<Car> = Vec::with_capacity(no_cars);

        for car_no in race_pars.participants.iter() {
            let car_pars_tmp = car_pars_all
                .get(car_no)
                .expect("Missing car number in car parameters!");

            let init_req = car_pars_tmp.strategy[0].driver_initials.trim();
            let driver_rc = drivers_by_initials
                .get(init_req)
                .or_else(|| drivers_list.get(init_req))
                .unwrap_or_else(|| panic!("Could not find start driver initials '{}' in drivers list!", init_req));

            cars_list.push(Car::new(
                car_pars_tmp,
                Rc::clone(driver_rc),
            ));
        }

        // sort cars list by car number
        cars_list.sort_unstable_by(|a, b| a.car_no.partial_cmp(&b.car_no).unwrap());

        // Ensure starting fuel is sufficient for the race distance (no refueling era)
        for car in cars_list.iter_mut() {
            let required = car.fuel_needed_for_laps(race_pars.tot_no_laps);
            let target = required * (1.0 + sim_consts.fuel_margin);
            if car.get_fuel_mass() < target {
                car.set_fuel_mass(target);
            }
        }

        //set the weather
        let start_weather = match race_pars.initial_weather.as_str() {
            "Rain" => WeatherState::Rain,
            _ => WeatherState::Dry // domyślnie jest sucho            
        };

        // create race
        let mut race = Race {
            timestep_size,
            cur_racetime: 0.0,
            weather_state: start_weather,
            print_events: true,
            rain_probability: race_pars.rain_probability,
            min_weather_duration_s: sim_consts.min_weather_duration_s,
            last_weather_change: 0.0,
            failure_rate_per_hour: sim_consts.failure_rate_per_hour,
            collision_factor: sim_consts.collision_factor,
            weather_history_log: Vec::new(),
            events: Vec::new(),
            safety_car: SafetyCar::new(),
            sc_timer: 0.0,
            sc_triggers: vec![false; no_cars], //na start wszystkie false
            sc_target_gap_m: 15.0,
            sc_lineup_tolerance_m: 5.0,
            sc_release_delay_s: 5.0,
            season: race_pars.season,
            tot_no_laps: race_pars.tot_no_laps,
            drs_allowed_lap: race_pars.drs_allowed_lap,
            cur_lap_leader: 1,
            min_t_dist: sim_consts.min_t_dist,
            t_duel: sim_consts.t_duel,
            t_overtake_loser: sim_consts.t_overtake_loser,
            drs_window: sim_consts.drs_window,
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
            tire_config: tire_config.clone(),
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

        // Pogoda: skaluj prawdopodobieństwo zmian do kroku czasu i wymuś minimalny czas trwania
        // Interpretacja: `rain_probability` to prawdopodobieństwo zmiany na minutę (nie na krok).
        let mut rng = rand::thread_rng();
        let eligible_for_change = (self.cur_racetime - self.last_weather_change) >= self.min_weather_duration_s;
        if eligible_for_change {
            let p_step = self.rain_probability * (self.timestep_size / 60.0);
            if rng.gen::<f64>() < p_step {
                self.weather_state = match self.weather_state {
                    WeatherState::Dry => {
                        if self.print_events { println!("WEATHER CHANGE: Rain started at {:.2}s!", self.cur_racetime); }
                        self.last_weather_change = self.cur_racetime;
                        // event: rain start
                        self.events.push(RaceEvent {
                            kind: "WeatherRainStart".to_string(),
                            lap: self.cur_lap_leader,
                            time_s: self.cur_racetime,
                            cars: vec![],
                        });
                        // Zaplanuj pit na najbliższe okrążenie dla slicków → Intermediate
                        for (i, car) in self.cars_list.iter_mut().enumerate() {
                            if car.status == CarStatus::DNF { continue; }
                            let comp = car.get_current_compound();
                            match comp {
                                "SOFT" | "MEDIUM" | "HARD" => {
                                    car.last_slick_compound = Some(comp.to_owned());
                                    let target_lap = car.sh.get_compl_lap() + 1;
                                    car.schedule_weather_strategy(target_lap, "INTERMEDIATE");
                                },
                                _ => {},
                            }
                        }
                        WeatherState::Rain
                    },
                    WeatherState::Rain => {
                        if self.print_events { println!("WEATHER CHANGE: Rain stopped at {:.2}s!", self.cur_racetime); }
                        self.last_weather_change = self.cur_racetime;
                        // event: dry start
                        self.events.push(RaceEvent {
                            kind: "WeatherDryStart".to_string(),
                            lap: self.cur_lap_leader,
                            time_s: self.cur_racetime,
                            cars: vec![],
                        });
                        // Zaplanuj pit na najbliższe okrążenia dla Inter/Wet → powrót do slicków
                        for (i, car) in self.cars_list.iter_mut().enumerate() {
                            if car.status == CarStatus::DNF { continue; }
                            let comp = car.get_current_compound();
                            let target_slick = car.last_slick_compound.clone().unwrap_or_else(|| "MEDIUM".to_string());
                            match comp {
                                "INTERMEDIATE" => {
                                    let target_lap = car.sh.get_compl_lap() + 1;
                                    car.schedule_weather_strategy(target_lap, &target_slick);
                                },
                                "WET" => {
                                    let target_lap = car.sh.get_compl_lap() + 2;
                                    car.schedule_weather_strategy(target_lap, &target_slick);
                                },
                                _ => {},
                            }
                        }
                        WeatherState::Dry
                    },
                };
            }
        }

        // increment discretization variable
        self.cur_racetime += self.timestep_size;

        if matches!(self.flag_state, FlagState::Sc){
            if self.sc_timer.is_finite() {
                self.sc_timer -= self.timestep_size;
            }

            if !self.safety_car.active{
                self.safety_car.active = true;
                // safety car startuje z poziomu lidera
                let mut leader_idx = 0;
                let mut max_prog = -1.0;
                
                // szukamy lidera wyścigu i to przed nim będzie safety car
                for(i, car) in self.cars_list.iter().enumerate(){
                    let prog = car.sh.get_race_prog();
                    if prog > max_prog && car.status != CarStatus::DNF{
                        max_prog = prog;
                        leader_idx = i;
                    }
                }

                self.safety_car.s_track = self.cars_list[leader_idx].sh.get_s_tracks().1 + 500.0;
                // Obsługa przypadku, gdy dodanie 500m wyrzuca SC na następne okrążenie
                if self.safety_car.s_track > self.track.length {
                    self.safety_car.s_track -= self.track.length;
                    self.safety_car.lap = self.cur_lap_leader + 1;
                } else {
                    self.safety_car.lap = self.cur_lap_leader;
                }
                // event: SC deployed
                self.events.push(RaceEvent {
                    kind: "SC_DEPLOYED".to_string(),
                    lap: self.cur_lap_leader,
                    time_s: self.cur_racetime,
                    cars: vec![],
                });
            }

            // przecunięcie SC do przodu
            self.safety_car.s_track += self.safety_car.speed * self.timestep_size;

            if self.safety_car.s_track > self.track.length {
                self.safety_car.s_track -= self.track.length;
                self.safety_car.lap +=1;
            }

            if self.sc_timer.is_finite() && self.sc_timer <= 0.00 {
                if self.print_events { println!("SAFETY CAR IN THIS LAP - RACE RESUMING"); }
                self.flag_state = FlagState::G;
                self.safety_car.active = false;
                // event: SC in
                self.events.push(RaceEvent {
                    kind: "SC_IN".to_string(),
                    lap: self.cur_lap_leader,
                    time_s: self.cur_racetime,
                    cars: vec![],
                });
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
                    if self.print_events { println!("SAFETY CAR DEPLOYED (Caused by car #{})", car.car_no); }
                    self.flag_state = FlagState::Sc;
                    // Tryb dynamiczny: odjazd po ustawieniu kolejki kierowców
                    self.sc_timer = 300.0; // maksymalnie 300 sekund na sf
                    // Oznacz wszystkie aktualne DNFs jako już obsłużone (unikamy podwójnego SC)
                    for (j, c) in self.cars_list.iter().enumerate() {
                        if c.status == CarStatus::DNF { self.sc_triggers[j] = true; }
                    }
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

        //Pogoda
        let is_wet = self.weather_state == WeatherState::Rain;

        
        // Bazowy czas
        let lap_time_base = self.track.t_q
        + self.track.t_gap_racepace
        + self.cars_list[idx].calc_basic_timeloss(self.track.s_mass, is_wet, &self.tire_config);

        self.cur_th_laptimes[idx] = lap_time_base + random_factor;
    }

    /// Dostosowuje teoretyczne czasy okrążeń (uproszczone).
/// Dostosowuje teoretyczne czasy okrążeń (uproszczone + SC logic).
    fn calc_cur_laptimes(&mut self) {
        // Sprawdź czy Safety Car jest fizycznie na torze i aktywny
        let sc_active = matches!(self.flag_state, FlagState::Sc) && self.safety_car.active;
        
        let sc_speed = if sc_active { self.safety_car.speed } else { 0.0 };

        // Oblicz całkowity dystans Safety Cara od startu wyścigu.
        // Safety Car `lap` to numer aktualnego okrążenia (od 1).
        // Dla dystansu potrzebujemy liczby UKOŃCZONYCH okrążeń, więc (lap - 1).
        let sc_completed_laps = if self.safety_car.lap > 0 { self.safety_car.lap - 1 } else { 0 };
        let sc_total_dist = if sc_active {
            sc_completed_laps as f64 * self.track.length + self.safety_car.s_track
        } else {
            0.0
        };

        // --- CZĘŚĆ 1: PODSTAWOWE OBLICZENIA (FIZYKA + PIT STOPY) ---
        // (SC Logic wyrzucone stąd do osobnego bloku niżej, żeby obsłużyć kolejkowanie)
        for (i, car) in self.cars_list.iter().enumerate() {
            if car.status == CarStatus::DNF {
                self.cur_laptimes[i] = f64::INFINITY;
                continue;
            }

            // // Startujemy od teoretycznego czasu (fizyka)
            // self.cur_laptimes[i] = self.cur_th_laptimes[i];

            // NOWY KOD
            let s_track = car.sh.get_s_tracks().1;

            // KROK 2: Mapujemy metry na indeks tablicy multipliers
            // Dzielimy pozycję przez długość toru (ułamek 0.0-1.0) i mnożymy przez liczbę punktów pomiarowych
            let mult_count = self.track.multipliers.len();
            let mut idx_m = ((s_track / self.track.length) * mult_count as f64) as usize;

            // Zabezpieczenie: jeśli idx_m wyjdzie poza zakres (np. na samej mecie), bierzemy ostatni element
            if idx_m >= mult_count { 
                idx_m = mult_count.saturating_sub(1); 
            }

            // KROK 3: Pobieramy wartość mnożnika dla tego fragmentu toru
            // Jeśli wektor jest pusty (błąd pliku), ustawiamy bezpieczne 1.0
            let multiplier = if mult_count > 0 { 
                self.track.multipliers[idx_m] 
            } else { 
                1.0 
            };

            // KROK 4: Modyfikujemy czas okrążenia (odwrotność prędkości)
            // Dzielimy, ponieważ:
            // - Jeśli multiplier > 1 (prosta) -> mianownik duży -> czas mały -> AUTO PRZYSPIESZA
            // - Jeśli multiplier < 1 (zakręt) -> mianownik mały -> czas duży -> AUTO ZWALNIA
            self.cur_laptimes[i] = self.cur_th_laptimes[i] / multiplier;
            
            // NOWY KOD


            // Obsługa Flag (jeśli nie SC)
            if !sc_active && !car.sh.pit_act {
                if self.cur_laptimes[i] < self.get_min_laptime_flag_state() {
                    self.cur_laptimes[i] = self.get_min_laptime_flag_state();
                }
                // Dodatki wyścigowe (DRS, Duel) tylko gdy nie ma SC
                // DRS wyłączony podczas deszczu
                if car.sh.drs_act && self.weather_state == WeatherState::Dry {
                    self.cur_laptimes[i] += self.track.t_drseffect / self.track.overtaking_zones_lap_frac;
                }
                if car.sh.duel_act {
                    self.cur_laptimes[i] += self.t_duel / self.track.overtaking_zones_lap_frac;
                }
            }

            // Kary za zakręty
            if car.sh.corner_act {
                self.cur_laptimes[i] += 0.5;
            }

            // Obsługa Pit Stopów
            if car.sh.pit_act {
                if !car.sh.pit_standstill_act {
                    self.cur_laptimes[i] = self.track.length / self.track.pit_speedlimit * self.track.real_length_pit_zone / self.track.track_length_pit_zone;
                } else {
                    if let Some(t_driving) = car.sh.check_leaves_standstill(self.timestep_size) {
                        self.cur_laptimes[i] = self.track.length / self.track.pit_speedlimit * self.track.real_length_pit_zone / self.track.track_length_pit_zone * self.timestep_size / t_driving;
                    } else {
                        self.cur_laptimes[i] = f64::INFINITY;
                    }
                }
            }
        }

        if !sc_active {
            // 1. Ustal kolejność bolidów na torze
            let idxs_sorted = self.get_car_order_on_track(); // [Lider, P2, P3, ...]
            
            // 2. Iterujemy przez pary (samochód z przodu vs samochód z tyłu)
            // Używamy indeksów, żeby mieć dostęp do &mut self.laptimes i car.dirty_air_wear_factor
            for i in 0..idxs_sorted.len() {
                let idx_front = idxs_sorted[i];
                // Samochód za nim (obsługa pętli - ostatni ściga pierwszego przy dublowaniu, 
                // ale dla uproszczenia pomińmy dublowanie lidera przez marudera w logice blokowania)
                if i == idxs_sorted.len() - 1 { continue; } 
                let idx_rear = idxs_sorted[i + 1];

                // Pomijamy auta w boksach
                if self.cars_list[idx_front].sh.pit_act || self.cars_list[idx_rear].sh.pit_act {
                    continue;
                }

                // Obliczamy dystans/czas między autami
                // Funkcja pomocnicza, którą już masz w kodzie (ewentualnie upewnij się, że zwraca poprawny gap)
                let gap_time = self.calc_projected_delta_t(idx_front, idx_rear, 0.0);

                // PARAMETRY INTERAKCJI
                let dirty_air_threshold = 2.0; // Poniżej 2s zaczyna się brudne powietrze
                let blocking_threshold = 0.5;  // Poniżej 0.5s można próbować wyprzedzać (lub utknąć)
                let overtake_speed_delta = 0.15; // Wymagana różnica prędkości (w sekundach na kółko), żeby wyprzedzić

                // A. EFEKT BRUDNEGO POWIETRZA (Dirty Air)
                if gap_time < dirty_air_threshold {
                    // Im bliżej, tym gorzej. Skalujemy efekt od 0.0 do 1.0
                    let intensity = 1.0 - (gap_time / dirty_air_threshold);
                    
                    // 1. Kara aerodynamiczna (trudniej skręcać)
                    let aero_penalty = 0.3 * intensity; 
                    self.cur_laptimes[idx_rear] += aero_penalty;

                    // 2. Kara termiczna dla opon (przegrzewanie)
                    // Mnożnik od 1.0 do 2.0 (przy zderzaku)
                    self.cars_list[idx_rear].dirty_air_wear_factor = 1.0 + (1.0 * intensity);
                }

                // B. EFEKT BLOKOWANIA (Blocking / Overtaking)
                if gap_time < blocking_threshold {
                    // Sprawdzamy czy auto z tyłu jest w ogóle szybsze (potencjalnie)
                    let time_front = self.cur_laptimes[idx_front];
                    let time_rear_potential = self.cur_laptimes[idx_rear]; // To jest czas z uwzględnieniem już kary aero

                    if time_rear_potential < time_front {
                        // Tył jest szybszy. Czy może wyprzedzić?
                        
                        // Pobieramy pozycję auta z tyłu
                        let s_track_rear = self.cars_list[idx_rear].sh.get_s_tracks().1;
                        let in_overtaking_zone = self.track.is_in_overtaking_zone(s_track_rear);
                        
                        // Warunek wyprzedzania:
                        // 1. Jest w strefie wyprzedzania (prosta/DRS)
                        // 2. Jest znacząco szybszy (delta > threshold) LUB używa DRS (można dodać warunek)
                        let speed_advantage = time_front - time_rear_potential;
                        
                        let can_overtake = in_overtaking_zone && (speed_advantage > overtake_speed_delta);

                        if !can_overtake {
                            // BLOKADA! (Pociąg Trullego)
                            // Auto z tyłu musi zwolnić do tempa auta z przodu (plus minimalny dystans)
                            // Ustawiamy czas okrążenia na czas lidera (nie może pojechać szybciej)
                            self.cur_laptimes[idx_rear] = time_front; 
                            
                            // Opcjonalnie: Dodatkowa frustracja/zużycie opon za jazdę "na zderzaku"
                            self.cars_list[idx_rear].dirty_air_wear_factor += 0.5;
                        } else {
                            // WYPRZEDZANIE DOZWOLONE
                            // Nie robimy nic - fizyka sama przesunie auto z tyłu przed auto z przodu,
                            // ponieważ ma niższy czas okrążenia.
                            // Wizualnie zamienią się miejscami w kolejnych krokach.
                        }
                    }
                }
            }
        }

        // --- CZĘŚĆ 2: LOGIKA SAFETY CAR (KOLEJKOWANIE) ---
        if sc_active {
            // 1. Sortujemy auta według pozycji na torze (kto jest pierwszy)
            let mut car_indices: Vec<usize> = (0..self.cars_list.len()).collect();
            car_indices.sort_by(|&a, &b| {
                // Sortowanie malejące po postępie wyścigu
                self.cars_list[b].sh.get_race_prog().partial_cmp(&self.cars_list[a].sh.get_race_prog()).unwrap()
            });

            // 2. Ustalamy punkt odniesienia dla lidera (jest nim Safety Car)
            let mut front_obj_pos = sc_total_dist;
            // Prędkość obiektu z przodu (bazowa prędkość pociągu)
            let _front_obj_speed = sc_speed;

            // Parametry kolejkowania
            let target_gap = self.sc_target_gap_m; // Metrów odstępu między autami
            let catchup_factor = 0.5; // Jak agresywnie nadrabiać dystans

            for &i in &car_indices {
                // Pomijamy auta w boksach i DNF
                if self.cars_list[i].status == CarStatus::DNF || self.cars_list[i].sh.pit_act {
                    continue;
                }

                // Oblicz dystans tego auta
                let car_pos = self.cars_list[i].sh.get_race_prog() * self.track.length;
                
                // Dystans do obiektu przed nami (SC lub inne auto)
                let gap = front_obj_pos - car_pos;

                // Obliczamy docelową prędkość, żeby utrzymać 5m odstępu
                // Wzór: v_target = v_sc + (różnica_dystansu * współczynnik)
                // Jeśli gap > 5m -> jedź szybciej niż SC
                // Jeśli gap < 5m -> jedź wolniej niż SC
                let speed_correction = (gap - target_gap) * catchup_factor;
                let mut target_speed = sc_speed + speed_correction;

                // ZABEZPIECZENIA:
                // 1. Nie możemy jechać szybciej niż pozwala na to bolid (max speed)
                let max_phys_speed = self.track.length / self.cur_th_laptimes[i];
                if target_speed > max_phys_speed {
                    target_speed = max_phys_speed;
                }

                // 2. Nie możemy jechać do tyłu ani stać w miejscu (chyba że korek totalny)
                if target_speed < 10.0 {
                    target_speed = 10.0; 
                }

                // Aplikujemy prędkość (zamiana na czas okrążenia)
                self.cur_laptimes[i] = self.track.length / target_speed;

                // Aktualizujemy pozycję "obiektu z przodu" dla NASTĘPNEGO auta w kolejce.
                // Następne auto ma trzymać 5m odstępu od TEGO auta.
                front_obj_pos = car_pos;
            }

            // --- Sprawdzenie ustawienia kolejki za SC ---
            // Warunek lineup: wszystkie aktywne auta (nie DNF, nie pit) trzymają odstęp ~ target_gap z tolerancją
            let tol = self.sc_lineup_tolerance_m;
            let mut positions: Vec<(usize, f64)> = Vec::new();
            for &i in &car_indices {
                if self.cars_list[i].status == CarStatus::DNF || self.cars_list[i].sh.pit_act { continue; }
                let car_pos = self.cars_list[i].sh.get_race_prog() * self.track.length;
                positions.push((i, car_pos));
            }
            let mut lineup_ok = !positions.is_empty();
            if lineup_ok {
                // Sprawdź lidera względem SC
                let leader_pos = positions[0].1;
                let mut gap_leader = sc_total_dist - leader_pos;
                if gap_leader < 0.0 { gap_leader += self.track.length; }
                if (gap_leader - target_gap).abs() > tol { lineup_ok = false; }
            }
            if lineup_ok {
                // Sprawdź kolejne pary aut
                for w in 0..positions.len().saturating_sub(1) {
                    let front = positions[w].1;
                    let rear = positions[w + 1].1;
                    let mut gap = front - rear;
                    if gap < 0.0 { gap += self.track.length; }
                    if (gap - target_gap).abs() > tol { lineup_ok = false; break; }
                }
            }

            // Jeśli mamy lineup i nie odliczamy jeszcze, uruchom krótki licznik do zjazdu SC
            if lineup_ok {
                if self.sc_timer > 10.0 {
                    self.sc_timer = 10.0;
                    if self.print_events { println!("Pack formad - safety car coming in shortly")}
                }
            } 
        } 
        // --- CZĘŚĆ 3: INTERAKCJE (TYLKO BEZ SC) ---
        else {
            let idxs_sorted = self.get_idx_list_sorted_by_biggest_gap();
            let car_pair_idxs_list = self.get_car_pair_idxs_list(&idxs_sorted, true);
            let mut laptimes_updates: Vec<(usize, f64)> = Vec::new();

            for pair_idxs in car_pair_idxs_list.iter() {
                let idx_front = pair_idxs[0];
                let idx_rear = pair_idxs[1];
                let delta_t_proj = self.calc_projected_delta_t(idx_front, idx_rear, self.timestep_size);

                // --- PRESJA/BŁĘDY I DROBNE KONTAKTY (gdy auta są blisko) ---
                let gap_time_close = self.calc_projected_delta_t(idx_front, idx_rear, 0.0);
                if gap_time_close < 1.0
                    && !self.cars_list[idx_front].sh.pit_act
                    && !self.cars_list[idx_rear].sh.pit_act
                {
                    let mut rng = rand::thread_rng();

                    // 1) Presja i błędy kierowcy z przodu (lock-up lub wyjazd szeroko)
                    let pressure_intensity = (1.0 - gap_time_close).clamp(0.0, 1.0);
                    let defender_consistency = self.cars_list[idx_front].driver.consistency;
                    let mistake_prob = (1.0 - defender_consistency) * pressure_intensity * 0.05;

                    if rng.gen::<f64>() < mistake_prob {
                        if rng.gen::<bool>() {
                            // Lock-up: strata czasu + dodatkowe zużycie opon
                            if self.print_events { println!(
                                "MISTAKE: Car {} locked up under pressure!",
                                self.cars_list[idx_front].car_no
                            ); }
                            self.cur_laptimes[idx_front] += 1.2;
                            self.cars_list[idx_front].dirty_air_wear_factor += 2.0;
                        } else {
                            // Wyjazd szeroko: strata u broniącego, mały zysk atakującego
                            if self.print_events { println!(
                                "MISTAKE: Car {} went wide!",
                                self.cars_list[idx_front].car_no
                            ); }
                            self.cur_laptimes[idx_front] += 0.8;
                            self.cur_laptimes[idx_rear] -= 0.3;
                        }
                    }

                    // 2) Drobny kontakt i uszkodzenia (bez DNF)
                    if gap_time_close < 0.3 {
                        let agg_factor = self.cars_list[idx_front].driver.aggression
                            + self.cars_list[idx_rear].driver.aggression;
                        let contact_prob = 0.005 * agg_factor; // niewielka szansa

                        if rng.gen::<f64>() < contact_prob {
                            if self.print_events { println!(
                                "CONTACT: Minor contact between #{} and #{}",
                                self.cars_list[idx_front].car_no,
                                self.cars_list[idx_rear].car_no
                            ); }
                            // Częściej obrywa atakujący (z tyłu)
                            let victim_idx = if rng.gen::<f64>() > 0.3 { idx_rear } else { idx_front };
                            self.cars_list[victim_idx].accumulated_damage_penalty += 0.3;
                        }
                    }
                }

                if !self.cars_list[idx_front].sh.pit_act && delta_t_proj < self.min_t_dist {
                    // --- NEW: Collision logic for very close fights ---
                    let proximity_threshold = 0.3; // s (bardziej restrykcyjnie)
                    if delta_t_proj < proximity_threshold
                        && !self.cars_list[idx_front].sh.pit_act
                        && !self.cars_list[idx_rear].sh.pit_act
                        && self.cars_list[idx_front].status != CarStatus::DNF
                        && self.cars_list[idx_rear].status != CarStatus::DNF
                    {
                        // Kalibracja hazardu kolizji: bardzo mała szansa per sekunda
                        let base_lambda_per_s = 4e-6; // bazowy hazard
                        let dt = self.timestep_size.max(1e-6);

                        let in_corner = self.cars_list[idx_front].sh.corner_act
                            || self.cars_list[idx_rear].sh.corner_act;
                        let corner_mult = if in_corner { 15.0 } else { 1.0 };

                        let ag_a = self.cars_list[idx_front].driver.aggression;
                        let ag_b = self.cars_list[idx_rear].driver.aggression;
                        let ag_sum = (ag_a + ag_b).clamp(0.0, 2.0);
                        let ag_mult = 1.0 + 0.8 * (ag_sum - 1.0); // 0.2..1.8x

                        let lambda = base_lambda_per_s * corner_mult * ag_mult * self.collision_factor;
                        let p_step = 1.0 - (-lambda * dt).exp();

                        let mut rng = rand::thread_rng();
                        if rng.gen::<f64>() < p_step {
                            self.cars_list[idx_front].status = CarStatus::DNF;
                            self.cars_list[idx_rear].status = CarStatus::DNF;
                            // Reflect immediate removal from pace this step
                            self.cur_laptimes[idx_front] = f64::INFINITY;
                            self.cur_laptimes[idx_rear] = f64::INFINITY;
                            if self.print_events { println!(
                                "CRASH: Car {} and Car {} collided in Turn!",
                                self.cars_list[idx_front].car_no,
                                self.cars_list[idx_rear].car_no
                            ); }
                            // event: crash
                            self.events.push(RaceEvent {
                                kind: "Crash".to_string(),
                                lap: self.cur_lap_leader,
                                time_s: self.cur_racetime,
                                cars: vec![self.cars_list[idx_front].car_no, self.cars_list[idx_rear].car_no],
                            });
                            // Skip further interaction handling for this pair
                            continue;
                        }
                    }

                    let overtake_threshold = 0.2;
                    let potential_pace_diff = self.cur_th_laptimes[idx_front] - self.cur_th_laptimes[idx_rear];
                    // Aggression influence: more aggressive rear lowers required pace delta,
                    // aggressive front slightly raises it (defending).
                    let ag_front = self.cars_list[idx_front].driver.aggression;
                    let ag_rear = self.cars_list[idx_rear].driver.aggression;
                    let mut eff_overtake_threshold = overtake_threshold * (1.0 - 0.7 * ag_rear + 0.3 * ag_front);
                    if eff_overtake_threshold < 0.05 { eff_overtake_threshold = 0.05; }
                    let in_corner = self.cars_list[idx_front].sh.corner_act || self.cars_list[idx_rear].sh.corner_act;

                    if potential_pace_diff > eff_overtake_threshold && !in_corner {
                        laptimes_updates.push((idx_rear, 0.1));
                        laptimes_updates.push((idx_front, self.t_overtake_loser));
                    } else {
                        let delta_t_cur = self.calc_projected_delta_t(idx_front, idx_rear, 0.0);
                        let t_gap_add = (self.min_t_dist - delta_t_cur) / 5.0 * self.cur_laptimes[idx_rear];
                        let target_time = self.cur_laptimes[idx_front] + t_gap_add;
                        if self.cur_laptimes[idx_rear] < target_time {
                            let diff = target_time - self.cur_laptimes[idx_rear];
                            laptimes_updates.push((idx_rear, diff));
                        }
                    }
                }
            }
            for (idx, time_add) in laptimes_updates {
                self.cur_laptimes[idx] += time_add;
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

        //zapisanie pogody do logów
        if self.cur_lap_leader > self.weather_history_log.len() as u32 {
            let weather_str = match self.weather_state {
                WeatherState::Rain => "Rain".to_string(),
                WeatherState::Dry => "Dry".to_string(),
            };
            self.weather_history_log.push(weather_str);
        }

        if self.cur_lap_leader > self.tot_no_laps && !matches!(self.flag_state, FlagState::C) {
            self.flag_state = FlagState::C;
            // Oznacz wszystkie auta jako ukończone, aby pętla główna mogła się zakończyć
            for finished in self.race_finished.iter_mut() {
                *finished = true;
            }
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

                // Track potential engine failure event
                let prev_status = car.status.clone();
                car.drive_lap(self.cur_laptimes[i], self.failure_rate_per_hour, self.print_events);
                if prev_status != car.status && car.status == CarStatus::DNF {
                    // Log as an EngineFailure event (treated as crash on plots)
                    self.events.push(RaceEvent {
                        kind: "EngineFailure".to_string(),
                        lap: self.cur_lap_leader, // current leader's lap after crossing
                        time_s: self.cur_racetime,
                        cars: vec![car.car_no],
                    });
                }

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
        // Jeśli jest flaga szachownicy, kończymy natychmiast
        if matches!(self.flag_state, FlagState::C) {
            return true;
        }
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
            weather_history: self.weather_history_log.clone(),
            events: self.events.clone(),
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