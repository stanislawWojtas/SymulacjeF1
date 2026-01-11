use crate::core::race::{Race, WeatherState, SimConstants, FlagState};
use crate::core::tireset::TireConfig;
use crate::interfaces::gui_interface::{CarState, RaceState, RgbColor, MAX_GUI_UPDATE_FREQUENCY};
use crate::post::race_result::RaceResult;
use crate::pre::read_sim_pars::SimPars;
use anyhow::Context;
use css_color_parser;
use flume::Sender;
use std::thread::sleep;
use std::time::{Duration, Instant};

/// handle_race creates and simulates a race on the basis of the inserted parameters, and returns
/// the results for post-processing.
pub fn handle_race(
    sim_pars: &SimPars,
    sim_consts: &SimConstants,
    tire_config: &TireConfig,
    timestep_size: f64,
    print_debug: bool,
    tx: Option<&Sender<RaceState>>,
    realtime_factor: f64,
    print_events: bool,
) -> anyhow::Result<RaceResult> {
    let mut race = Race::new(
        &sim_pars.race_pars,
        sim_consts,
        tire_config,
        &sim_pars.track_pars,
        &sim_pars.driver_pars_all,
        &sim_pars.car_pars_all,
        timestep_size,
    );
    race.print_events = print_events;

    // check if sender was inserted -> in that case use real-time simulation for GUI
    let sim_realtime = tx.is_some();
    if !sim_realtime {
        let mut t_race_update_print = 0.0;
        let mut last_printed_lap = 0u32;
        while !race.get_all_finished() {
            race.simulate_timestep();
            if print_debug && race.cur_racetime > t_race_update_print + 0.9999 {
                println!(
                    "INFO: Simulating... Current race time is {:.3}s, current lap is {}",
                    race.cur_racetime, race.cur_lap_leader
                );
                t_race_update_print = race.cur_racetime;
            }
            if print_debug && race.cur_lap_leader > last_printed_lap {
                println!("INFO: Leader started lap {}", race.cur_lap_leader);
                last_printed_lap = race.cur_lap_leader;
            }
        }
    } else {
        let mut t_race_update_print = 0.0;
        let mut t_race_update_gui = 0.0;

        while !race.get_all_finished() {
            let t_start = Instant::now();
            race.simulate_timestep();
            if race.cur_racetime > t_race_update_print + 0.9999 {
                println!(
                    "INFO: Simulating... Current race time is {:.3}s, current lap is {}",
                    race.cur_racetime, race.cur_lap_leader
                );
                t_race_update_print = race.cur_racetime;
            }
            if race.cur_racetime > t_race_update_gui + 1.0 / MAX_GUI_UPDATE_FREQUENCY - 0.001 {

                let sc_prog = if race.safety_car.active{
                    race.safety_car.lap as f64 + race.safety_car.s_track / race.track.length
                } else {
                    0.0
                };
                let mut race_state = RaceState {
                    car_states: Vec::with_capacity(race.cars_list.len()),
                    flag_state: race.flag_state.to_owned(),
                    sc_active: race.safety_car.active,
                    sc_race_prog: sc_prog,
                    weather_is_rain: matches!(race.weather_state, WeatherState::Rain),
                    final_result: None,
                };

                for (i, car) in race.cars_list.iter().enumerate() {
                    let tmp_color = car
                        .color
                        .parse::<css_color_parser::Color>()
                        .context("Could not parse hex color!")?;

                    let velocity = if car.sh.pit_standstill_act {
                        0.0
                    } else if car.sh.pit_act {
                        race.track.pit_speedlimit
                    } else if matches!(race.flag_state, FlagState::Sc) {
                        // SC Mode: Display average speed (no visual scaling to match movement logic)
                        race.track.length / race.cur_laptimes[i]
                    } else {
                        let cur_laptime = race.cur_laptimes[i];
                        if cur_laptime > 0.0 && cur_laptime.is_finite() && race.track.multipliers.len() > 0 {
                            let v_avg = race.track.length / cur_laptime;
                            let s_track = car.sh.get_s_tracks().1;
                            let mult_count = race.track.multipliers.len();
                            let mut idx_m = ((s_track / race.track.length) * mult_count as f64) as usize;
                            if idx_m >= mult_count {
                                idx_m = mult_count - 1;
                            }
                            let multiplier = race.track.multipliers[idx_m].max(0.1);
                            
                            // Smoother formula: 35% base speed + scaled boost (matches race.rs)
                            let visual_speed_factor = 0.35 + (1.15 * multiplier.powf(2.0));
                            
                            v_avg * visual_speed_factor
                        } else {
                            race.track.length / cur_laptime
                        }
                    };

                    race_state.car_states.push(CarState {
                        car_no: car.car_no,
                        driver_initials: car.driver.initials.to_owned(),
                        color: RgbColor {
                            r: tmp_color.r,
                            g: tmp_color.g,
                            b: tmp_color.b,
                        },
                        race_prog: car.sh.get_race_prog(),
                        velocity,
                    });
                }

                // send current race state
                tx.unwrap()
                    .send(race_state)
                    .context("Failed to send race state to GUI!")?;
                t_race_update_gui = race.cur_racetime;
            }

            // sleep until time step is finished in real-time as well (calculation in ms)
            let t_sleep = (race.timestep_size * 1000.0 / realtime_factor) as i64
                - t_start.elapsed().as_millis() as i64;

            if t_sleep > 0 {
                sleep(Duration::from_millis(t_sleep as u64));
            } else {
                println!("WARNING: Could not keep up with real-time!")
            }
        }

        // after real-time loop finishes, send final result once
        if let Some(tx) = tx {
            let result = race.get_race_result();
            let final_msg = RaceState {
                car_states: Vec::new(),
                flag_state: race.flag_state.to_owned(),
                sc_active: result.sc_active,
                sc_race_prog: if result.sc_active { result.sc_position / race.track.length } else { 0.0 },
                weather_is_rain: matches!(race.weather_state, WeatherState::Rain),
                final_result: Some(result),
            };
            tx.send(final_msg).context("Failed to send final race result to GUI!")?;
        }
    }
    if print_debug {
        println!(
            "DEBUG: Estimated time loss for driving through the pit lane (w/o standstill): {:.2}s",
            race.track.get_pit_drive_timeloss()
        )
    }

    // return race result
    Ok(race.get_race_result())
}
