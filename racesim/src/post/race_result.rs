use std::fmt::Write;
use std::io::Write as IoWrite;

use serde::{Serialize, Deserialize};

/// CarDriverPair is used to store car number and driver initials for post-processing the results.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CarDriverPair {
    pub car_no: u32,
    pub driver_initials: String,
}

/// RaceResult contains all race information that is required for post-processing the results.
/// 
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RaceResult {
    pub tot_no_laps: u32,
    pub car_driver_pairs: Vec<CarDriverPair>,
    pub laptimes: Vec<Vec<f64>>,
    pub racetimes: Vec<Vec<f64>>,
    pub sc_active: bool, // czy SC jest na torze
    pub sc_position: f64, //gdzie jest SC
    pub weather_history: Vec<String>,
    pub events: Vec<RaceEvent>,
}

impl RaceResult {
    /// write_lap_and_race_times_to_file writes lap and race times to a text file in output/.
    /// Returns the path to the written file.
    pub fn write_lap_and_race_times_to_file(
        &self,
        path: Option<&std::path::Path>,
    ) -> anyhow::Result<String> {
        let mut tmp_string_laptime = String::new();
        let mut tmp_string_racetime = String::new();

        for lap in 1..self.tot_no_laps as usize + 1 {
            write!(&mut tmp_string_laptime, "{:3}, ", lap)?;
            write!(&mut tmp_string_racetime, "{:3}, ", lap)?;

            for i in 0..self.car_driver_pairs.len() {
                if i < self.car_driver_pairs.len() - 1 {
                    write!(&mut tmp_string_laptime, "{:8.3}s, ", self.laptimes[i][lap])?;
                    write!(
                        &mut tmp_string_racetime,
                        "{:8.3}s, ",
                        self.racetimes[i][lap]
                    )?;
                } else {
                    writeln!(&mut tmp_string_laptime, "{:8.3}s", self.laptimes[i][lap])?;
                    writeln!(&mut tmp_string_racetime, "{:8.3}s", self.racetimes[i][lap])?;
                }
            }
        }

        let mut tmp_string_car_driver_info = String::from("lap, ");
        for (i, car_driver_pair) in self.car_driver_pairs.iter().enumerate() {
            if i < self.car_driver_pairs.len() - 1 {
                write!(
                    &mut tmp_string_car_driver_info,
                    "{:3} ({}), ",
                    car_driver_pair.car_no, car_driver_pair.driver_initials
                )?;
            } else {
                write!(
                    &mut tmp_string_car_driver_info,
                    "{:3} ({})",
                    car_driver_pair.car_no, car_driver_pair.driver_initials
                )?;
            }
        }

        let mut content = String::new();
        writeln!(&mut content, "RESULT: Lap times")?;
        writeln!(&mut content, "{}", tmp_string_car_driver_info)?;
        writeln!(&mut content, "{}", tmp_string_laptime)?;
        writeln!(&mut content, "RESULT: Race times")?;
        writeln!(&mut content, "{}", tmp_string_car_driver_info)?;
        writeln!(&mut content, "{}", tmp_string_racetime)?;
        let out_dir = std::path::Path::new("output");
        std::fs::create_dir_all(out_dir)?;
        let out_path = if let Some(p) = path { p.to_path_buf() } else { out_dir.join("last_run.txt") };
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&out_path)?;
        file.write_all(content.as_bytes())?;
        file.flush()?;

        Ok(out_path.to_string_lossy().into_owned())
    }

    /// print_lap_and_race_times prints the resulting lap and race times to the console output.
    pub fn print_lap_and_race_times(&self) {
        let mut tmp_string_laptime = String::new();
        let mut tmp_string_racetime = String::new();

        for lap in 1..self.tot_no_laps as usize + 1 {
            write!(&mut tmp_string_laptime, "{:3}, ", lap).unwrap();
            write!(&mut tmp_string_racetime, "{:3}, ", lap).unwrap();

            for i in 0..self.car_driver_pairs.len() {
                if i < self.car_driver_pairs.len() - 1 {
                    write!(&mut tmp_string_laptime, "{:8.3}s, ", self.laptimes[i][lap]).unwrap();
                    write!(
                        &mut tmp_string_racetime,
                        "{:8.3}s, ",
                        self.racetimes[i][lap]
                    )
                    .unwrap();
                } else {
                    writeln!(&mut tmp_string_laptime, "{:8.3}s", self.laptimes[i][lap]).unwrap();
                    writeln!(&mut tmp_string_racetime, "{:8.3}s", self.racetimes[i][lap]).unwrap();
                }
            }
        }
        let mut tmp_string_car_driver_info = String::from("lap, ");

        for (i, car_driver_pair) in self.car_driver_pairs.iter().enumerate() {
            if i < self.car_driver_pairs.len() - 1 {
                write!(
                    &mut tmp_string_car_driver_info,
                    "{:3} ({}), ",
                    car_driver_pair.car_no, car_driver_pair.driver_initials
                )
                .unwrap()
            } else {
                write!(
                    &mut tmp_string_car_driver_info,
                    "{:3} ({})",
                    car_driver_pair.car_no, car_driver_pair.driver_initials
                )
                .unwrap()
            }
        }
        println!("RESULT: Lap times");
        println!("{}", tmp_string_car_driver_info);
        println!("{}", tmp_string_laptime);

        println!("RESULT: Race times");
        println!("{}", tmp_string_car_driver_info);
        println!("{}", tmp_string_racetime);
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RaceEvent {
    pub kind: String,        // "Crash", "WeatherRainStart", "WeatherDryStart", "SC_DEPLOYED", "SC_IN"
    pub lap: u32,            // numer okrążenia w momencie zdarzenia (1-based)
    pub time_s: f64,         // czas wyścigu w sekundach
    pub cars: Vec<u32>,      // dotknięte auta (np. przy kraksie)
}
