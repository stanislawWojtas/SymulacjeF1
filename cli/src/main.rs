use clap::Parser;
use flume;
use gui::core::gui::RacePlot;
use racesim::post::race_result::RaceResult;
use racesim::pre::read_sim_pars::{read_sim_pars_flexible, read_sim_constants, read_tire_config};
use racesim::pre::sim_opts::SimOpts;
use std::path::PathBuf;
use std::thread;
use std::time::Instant;
use plotters::prelude::*;

fn export_results_plot(
    result: &racesim::post::race_result::RaceResult,
    track_length_m: f64,
    show_speed: bool,
    averaged_n: Option<u32>,
) -> anyhow::Result<String> {
    let out_dir = std::path::Path::new("output");
    std::fs::create_dir_all(out_dir)?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let filename = if let Some(n) = averaged_n {
        format!("race_plot_avg_{}_{}.png", n, ts)
    } else {
        format!("race_plot_{}.png", ts)
    };
    let out_path = out_dir.join(filename);

    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    let tot_laps = result.tot_no_laps as usize;
    for (i, _) in result.car_driver_pairs.iter().enumerate() {
        for lap in 1..=tot_laps {
            let lt = result.laptimes[i][lap];
            if lt.is_finite() && lt > 0.0 {
                let y = if show_speed { (track_length_m / lt) * 3.6 } else { lt };
                if y < y_min { y_min = y; }
                if y > y_max { y_max = y; }
            }
        }
    }
    if !y_min.is_finite() || !y_max.is_finite() { y_min = 0.0; y_max = 1.0; }
    let margin = (y_max - y_min) * 0.05;
    y_min -= margin; y_max += margin;

    let root = BitMapBackend::new(out_path.to_str().unwrap(), (1280, 720)).into_drawing_area();
    root.fill(&WHITE)?;
    let title_base = if show_speed { "Średnia prędkość na okrążeniach" } else { "Czas okrążenia" };
    let title = if let Some(n) = averaged_n {
        format!("{} (uśrednione z {} prób)", title_base, n)
    } else {
        title_base.to_string()
    };

    let mut chart = ChartBuilder::on(&root)
        .caption(title, ("sans-serif", 24).into_font())
        .margin(20)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .build_cartesian_2d(1u32..result.tot_no_laps, y_min..y_max)?;

    // Light-grey background bands for rainy laps
    if !result.weather_history.is_empty() {
        for lap in 1..=result.tot_no_laps as usize {
            if result.weather_history.get(lap - 1).map(|s| s == "Rain").unwrap_or(false) {
                let x0 = lap as u32;
                let x1 = (lap as u32).saturating_add(1);
                chart.draw_series(std::iter::once(Rectangle::new(
                    [(x0, y_min), (x1, y_max)],
                    RGBAColor(200, 200, 200, 0.20).filled(),
                )))?;
            }
        }
    }

    chart.configure_mesh()
        .x_desc("Okrążenie")
        .y_desc(if show_speed { "km/h" } else { "s" })
        .label_style(("sans-serif", 16))
        .axis_desc_style(("sans-serif", 16))
        .draw()?;

    let palette = Palette99::pick;
    for (i, pair) in result.car_driver_pairs.iter().enumerate() {
        let mut series: Vec<(u32, f64)> = Vec::new();
        for lap in 1..=tot_laps {
            let lt = result.laptimes[i][lap];
            if lt.is_finite() && lt > 0.0 {
                let y = if show_speed { (track_length_m / lt) * 3.6 } else { lt };
                series.push((lap as u32, y));
            }
        }
        chart.draw_series(LineSeries::new(series.into_iter(), palette(i)))?
            .label(format!("{} ({})", pair.car_no, pair.driver_initials))
            .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], palette(i)));
    }

    for ev in &result.events {
        let x = ev.lap as u32;
        let (color, width) = match ev.kind.as_str() {
            "WeatherRainStart" | "WeatherDryStart" => (RGBColor(150, 150, 150), 1),
            "SC_DEPLOYED" | "SC_IN" => (RGBColor(255, 165, 0), 1),
            "Crash" | "EngineFailure" => (RED, 2),
            _ => (BLACK, 1),
        };
        chart.draw_series(std::iter::once(PathElement::new(
            vec![(x, y_min), (x, y_max)], color.stroke_width(width),
        )))?;
    }

    chart.configure_series_labels()
        .border_style(&BLACK)
        .background_style(&WHITE.mix(0.8))
        .label_font(("sans-serif", 16))
        .position(plotters::chart::SeriesLabelPosition::UpperRight)
        .draw()?;

    root.present()?;
    Ok(out_path.to_string_lossy().into_owned())
}

fn average_results(results: &[RaceResult]) -> RaceResult {
    assert!(!results.is_empty(), "No results to average");

    let base = &results[0];
    let tot_no_laps = base.tot_no_laps as usize;
    let n_cars = base.car_driver_pairs.len();

    // Prepare accumulators
    let mut avg_laptimes = vec![vec![0.0f64; tot_no_laps + 1]; n_cars];

    // Average lap times per car, per lap, skipping invalids
    for car_idx in 0..n_cars {
        for lap in 1..=tot_no_laps {
            let mut sum = 0.0f64;
            let mut cnt = 0usize;
            for run in results {
                // Defensive: shape consistency
                if car_idx >= run.laptimes.len() || lap >= run.laptimes[car_idx].len() {
                    continue;
                }
                let t = run.laptimes[car_idx][lap];
                if t.is_finite() && t > 0.0 {
                    sum += t;
                    cnt += 1;
                }
            }
            avg_laptimes[car_idx][lap] = if cnt > 0 { sum / cnt as f64 } else { 0.0 };
        }
    }

    // Build cumulative race times from averaged lap times
    let mut avg_racetimes = vec![vec![0.0f64; tot_no_laps + 1]; n_cars];
    for car_idx in 0..n_cars {
        for lap in 1..=tot_no_laps {
            let lt = avg_laptimes[car_idx][lap];
            avg_racetimes[car_idx][lap] = avg_racetimes[car_idx][lap - 1] + lt.max(0.0);
        }
    }

    RaceResult {
        tot_no_laps: base.tot_no_laps,
        car_driver_pairs: base.car_driver_pairs.clone(),
        laptimes: avg_laptimes,
        racetimes: avg_racetimes,
        sc_active: false,
        sc_position: 0.0,
        weather_history: Vec::new(),
        events: Vec::new(),
    }
}

fn main() -> anyhow::Result<()> {
    // PRE-PROCESSING ------------------------------------------------------------------------------
    // get simulation options from the command line arguments
    let sim_opts: SimOpts = SimOpts::parse();

    // get simulation parameters (scenario + data)
    let sim_pars = if let Some(parfile_path) = &sim_opts.parfile_path {
        println!("INFO: Reading simulation parameters from {:?}", parfile_path);
        read_sim_pars_flexible(parfile_path)?
    } else {
        anyhow::bail!("No parameter file provided! Use -p <path_to_json> to run the simulation.");
    };

    // get simulation constants (physics engine), from default path
    let sim_consts_path: PathBuf = ["input", "parameters", "sim_constants.json"].iter().collect();
    let sim_consts = read_sim_constants(&sim_consts_path)?;

    // get tire configuration from default path
    let tire_cfg_path: PathBuf = ["input", "parameters", "tires.json"].iter().collect();
    let tire_cfg = read_tire_config(&tire_cfg_path)?;

    // print race details
    println!(
        "INFO: Simulating {} {} with a time step size of {:.3}s",
        sim_pars.track_pars.name, sim_pars.race_pars.season, sim_opts.timestep_size
    );

    // EXECUTION -----------------------------------------------------------------------------------
    if !sim_opts.gui {
        // NON-GUI CASE - Monte Carlo (multi-run) or single-run if no_sim_runs == 1
        let runs = sim_opts.no_sim_runs.max(1);
        if runs == 1 {
            println!("INFO: Running single simulation without GUI...");
            let t_start = Instant::now();

            let race_result = racesim::core::handle_race::handle_race(
                &sim_pars,
                &sim_consts,
                &tire_cfg,
                sim_opts.timestep_size,
                sim_opts.debug,
                None,
                1.0,
                true,
            )?;

            println!("INFO: Execution time: {}ms", t_start.elapsed().as_millis());

            match race_result.write_lap_and_race_times_to_file(None) {
                Ok(path) => println!("INFO: Wyniki zapisane: {}", path),
                Err(e) => eprintln!("WARNING: Nie udało się zapisać wyników: {}", e),
            }

            match export_results_plot(&race_result, sim_pars.track_pars.length, false, None) {
                Ok(path) => println!("INFO: Wykres zapisany: {}", path),
                Err(e) => eprintln!("WARNING: Nie udało się zapisać wykresu: {}", e),
            }
        } else {
            println!("INFO: Running {} simulations for averaging...", runs);
            let t_start_total = Instant::now();
            let mut results: Vec<RaceResult> = Vec::with_capacity(runs as usize);

            for i in 0..runs {
                println!("INFO: Simulating run {}/{}", i + 1, runs);
                let res = racesim::core::handle_race::handle_race(
                    &sim_pars,
                    &sim_consts,
                    &tire_cfg,
                    sim_opts.timestep_size,
                    false, // suppress per-run debug for speed; use --debug with single run
                    None,
                    1.0,
                    false, // suppress event prints in multi-run
                )?;
                results.push(res);
            }

            let averaged = average_results(&results);
            println!("INFO: All runs done in {}ms", t_start_total.elapsed().as_millis());

            // Save averaged results to a dedicated file
            let mut out_path = PathBuf::new();
            out_path.push("output");
            std::fs::create_dir_all(&out_path)?;
            out_path.push("last_run_averaged.txt");

            match averaged.write_lap_and_race_times_to_file(Some(&out_path)) {
                Ok(path) => println!("INFO: Averaged results saved: {}", path),
                Err(e) => eprintln!("WARNING: Could not save averaged results: {}", e),
            }

            match export_results_plot(&averaged, sim_pars.track_pars.length, false, Some(runs)) {
                Ok(path) => println!("INFO: Averaged plot saved: {}", path),
                Err(e) => eprintln!("WARNING: Could not save averaged plot: {}", e),
            }
        }
    } else {
        // GUI CASE - symulacja w czasie rzeczywistym z wizualizacją
        println!("INFO: Starting GUI simulation...");
        
        // Utwórz kanał komunikacji między GUI a symulatorem
        let (tx, rx) = flume::unbounded();

        // Uruchom symulator w osobnym wątku
        let sim_opts_thread = sim_opts.clone();
        let sim_pars_thread = sim_pars.clone();
        let sim_consts_thread = sim_consts.clone();
        let tire_cfg_thread = tire_cfg.clone();

        let _ = thread::spawn(move || {
            racesim::core::handle_race::handle_race(
                &sim_pars_thread,
                &sim_consts_thread,
                &tire_cfg_thread,
                sim_opts_thread.timestep_size,
                false, // debug wyłączony w GUI
                Some(&tx),
                sim_opts_thread.realtime_factor,
                false, // suppress event prints in GUI
            )
        });

        // Ustaw ścieżkę do pliku toru (zawsze z input/tracks)
        let mut trackfile_path = std::path::PathBuf::new();
        trackfile_path.push("input");
        trackfile_path.push("tracks");
        trackfile_path.push(&sim_pars.track_pars.name);
        trackfile_path.set_extension("csv");

        println!("INFO: Loading track from: {:?}", trackfile_path);

        // Uruchom GUI (musi być w głównym wątku)
        let gui = RacePlot::new(
            rx,
            &sim_pars.race_pars,
            &sim_pars.track_pars,
            trackfile_path.as_path(),
        )?;
        let native_options = eframe::NativeOptions {
            initial_window_size: Some(eframe::egui::Vec2::new(1280.0, 720.0)),
            ..eframe::NativeOptions::default()
        };
        eframe::run_native(Box::new(gui), native_options);
    }

    Ok(())
}