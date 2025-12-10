use crate::core::car;
use crate::core::driver::Driver;
use crate::core::state_handler::StateHandler;
use crate::core::tireset::Tireset;
use serde::Deserialize;
use std::collections::HashMap;
use std::rc::Rc;
use rand::Rng;

/// Uproszczona strategia: dodano z powrotem `driver_initials` tylko dla startu.
/// * `inlap` - Okrążenie zjazdowe pit stopu (0 dla info o oponach na starcie)
/// * `tire_start_age` - Wiek opon przy montażu
/// * `compound` - Mieszanka montowana podczas pit stopu
/// * `driver_initials` - Inicjały kierowcy (używane tylko w wpisie 0 do ustawienia startowego kierowcy)
#[derive(Debug, Deserialize, Clone)]
pub struct StrategyEntry {
    pub inlap: u32,
    pub tire_start_age: u32,
    pub compound: String,
    pub driver_initials: String, // Przywrócone na potrzeby inicjalizacji
    pub refuel_mass: f64,
}

#[derive(Debug, PartialEq, Clone)]
pub enum CarStatus{
    Running,
    DNF,
}

/// Uproszczone parametry bolidu.
/// * `t_car` - (s) Strata czasu na okrążenie z powodu parametrów bolidu
/// * `t_pit_tirechange` - (s) Czas postoju na zmianę opon
/// * `pit_location` - (m) Lokalizacja pit stopu
/// ... reszta parametrów
#[derive(Debug, Deserialize, Clone)]
pub struct CarPars {
    pub car_no: u32,
    //pub team: String,
    //pub manufacturer: String,
    pub color: String,
    pub t_car: f64, // referencyjny czas okrążenia bolidu (bazowy performance)
    pub b_fuel_per_lap: f64, // zużycie paliwa na okrążenie (fuel/lap)
    pub m_fuel: f64, // aktualna masa/ilość paliwa (kg)
    pub t_pit_refuel_per_kg: Option<f64>, // (Opcjonalny) - współczynnik czasu tankowania na jednostke paliwa
    pub t_pit_tirechange: f64, // czas samej wymiany opon w boksie
    //pub t_pit_driverchange: Option<f64>, // (Opcjonalny) - czas samej zmiany kierowcy w boksie, jeśli bez zmiany to none
    pub pit_location: f64, // Pozycja pit stopu na torze (metry)
    pub strategy: Vec<StrategyEntry>, // strategia wyścigu
    pub p_grid: u32, // pozycja startowa na polach startowych
}

#[derive(Debug)]
pub struct Car {
    pub car_no: u32,
    pub color: String,
    pub status: CarStatus,
    pub reliability: f64,
    t_car: f64,
    m_fuel: f64,              
    b_fuel_per_lap: f64,  
    t_pit_refuel_per_kg: Option<f64>,
    t_pit_tirechange: f64,
    pub pit_location: f64,
    strategy: Vec<StrategyEntry>,
    pub p_grid: u32,
    pub driver: Rc<Driver>,
    pub sh: StateHandler,
    tireset: Tireset,
}

impl Car {
    pub fn new(car_pars: &CarPars, driver: Rc<Driver>) -> Car {
        Car {
            car_no: car_pars.car_no,
            color: car_pars.color.to_owned(),
            status: CarStatus::Running,
            reliability: 0.99, // 0.1% na awarie silnika
            t_car: car_pars.t_car,
            m_fuel: car_pars.m_fuel,
            b_fuel_per_lap: car_pars.b_fuel_per_lap, 
            t_pit_refuel_per_kg: car_pars.t_pit_refuel_per_kg, 
            t_pit_tirechange: car_pars.t_pit_tirechange,
            pit_location: car_pars.pit_location,
            strategy: car_pars.strategy.to_owned(),
            p_grid: car_pars.p_grid,
            driver,
            sh: StateHandler::default(),
            tireset: Tireset::new(
                car_pars.strategy[0].compound.to_owned(),
                car_pars.strategy[0].tire_start_age,
            ),
        }
    }


    pub fn calc_basic_timeloss(&self, s_mass: f64) -> f64 { // _s_mass jest ignorowane
        let degr_pars = self.driver.get_degr_pars(&self.tireset.compound);
        let tire_loss = self.tireset.t_add_tireset(&degr_pars);
        
        if self.car_no == 44 {
             // println!("DEBUG: Car 44: Compound: {}, Age: {}, TireLoss: {}, Fuel: {}", 
             //    self.tireset.compound, self.tireset.age_tot, tire_loss, self.m_fuel);
        }

        self.t_car
            + self.driver.t_driver
            + tire_loss
            + self.m_fuel * s_mass
    }

    /// Metoda zwiększa wiek opon.
    /// Usunięto spalanie paliwa.
    pub fn drive_lap(&mut self) {

        //obsługa awarii
        if (self.status == CarStatus::DNF){
            return;
        }
        let mut rng = rand::thread_rng();
        if(rng.gen::<f64>() > self.reliability){
            self.status = CarStatus::DNF;
            println!("CRASH: Car {} has retired from the race due to engine failure", self.car_no)
        }


        // Usunięto logikę m_fuel
        self.m_fuel -= self.b_fuel_per_lap; // <--- SPALAMY

        // Zabezpieczenie przed ujemnym paliwem (chociaż w F1 to dyskwalifikacja, tu symulujemy dalej)
        if self.m_fuel < 0.0 {
            println!(
                "WARNING: Remaining fuel mass of car {} is negative!",
                self.car_no
            );
            self.m_fuel = 0.0;
        }

        self.tireset.drive_lap();
    }

    /// Metoda sprawdza, czy bolid zjeżdża do alei w tym okrążeniu.
    pub fn pit_this_lap(&self, cur_lap: u32) -> bool {
        self.strategy
            .iter()
            .any(|strat_entry| strat_entry.inlap == cur_lap)
    }

    /// Metoda pobiera wpis strategii dla bieżącego okrążenia zjazdowego.
    /// Zwraca `None`, jeśli brak wpisu dla danego `inlap`.
    fn get_strategy_entry(&self, inlap: u32) -> Option<StrategyEntry> {
        self.strategy
            .iter()
            .find(|&x| x.inlap == inlap)
            .cloned()
    }

    /// Metoda wykonuje pit stop: tylko zmiana opon.
    /// Usunięto tankowanie i zmiany kierowców.
    pub fn perform_pitstop(&mut self, inlap: u32, _drivers_list: &HashMap<String, Rc<Driver>>) {
        // get strategy entry (opcjonalnie)
        if let Some(strategy_entry) = self.get_strategy_entry(inlap) {
            // handle tire change
            if !strategy_entry.compound.is_empty() {
                self.tireset = Tireset::new(
                    strategy_entry.compound.to_owned(),
                    strategy_entry.tire_start_age,
                );
            }
        } else {
            // Brak wpisu strategii dla tego okrążenia – pomijamy pit stop.
            // Pozostawiamy bieżący zestaw opon bez zmian.
        }
        
        // Refueling logic removed
        // if strategy_entry.refuel_mass > 0.0 {
        //     self.m_fuel += strategy_entry.refuel_mass;
        // }

        
    }

    /// Metoda zwraca czas postoju w alei.
    /// Tylko czas zmiany opon.
    pub fn t_add_pit_standstill(&self, inlap: u32) -> f64 {
        let strategy_entry_opt = self.get_strategy_entry(inlap);

        // Czas zmiany opon (tylko jeśli strategia przewiduje zmianę)
        let t_standstill = if let Some(strategy_entry) = strategy_entry_opt {
            if !strategy_entry.compound.is_empty() {
                self.t_pit_tirechange
            } else {
                0.0
            }
        } else {
            // Brak wpisu strategii – brak postoju
            0.0
        };

        // Refueling time calculation removed
        // if strategy_entry.refuel_mass > 0.0 {
        //      let t_refuel = strategy_entry.refuel_mass * self.t_pit_refuel_per_kg.unwrap_or(0.0);
        //      t_standstill = t_standstill.max(t_refuel);
        // }

        t_standstill
    }
}