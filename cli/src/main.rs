use clap::Parser;
use flume;
use gui::core::gui::RacePlot;
use racesim::pre::read_sim_pars::read_sim_pars;
use racesim::pre::sim_opts::SimOpts;
use std::thread;
use std::time::Instant;
use plotters::prelude::*;

fn export_results_plot(
    result: &racesim::post::race_result::RaceResult,
    track_length_m: f64,
    show_speed: bool,
) -> anyhow::Result<String> {
    let out_dir = std::path::Path::new("output");
    std::fs::create_dir_all(out_dir)?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let filename = format!("race_plot_{}.png", ts);
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
    let mut chart = ChartBuilder::on(&root)
        .caption(
            if show_speed { "Średnia prędkość na okrążeniach" } else { "Czas okrążenia" },
            ("sans-serif", 24).into_font(),
        )
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
            "Crash" => (RED, 2),
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

fn main() -> anyhow::Result<()> {
    // PRE-PROCESSING ------------------------------------------------------------------------------
    // get simulation options from the command line arguments
    let sim_opts: SimOpts = SimOpts::parse();

    // get simulation parameters
    let sim_pars = if let Some(parfile_path) = &sim_opts.parfile_path {
        println!("INFO: Reading simulation parameters from {:?}", parfile_path);
        read_sim_pars(parfile_path)?
    } else {
        anyhow::bail!("No parameter file provided! Use -p <path_to_json> to run the simulation.");
    };

    // print race details
    println!(
        "INFO: Simulating {} {} with a time step size of {:.3}s",
        sim_pars.track_pars.name, sim_pars.race_pars.season, sim_opts.timestep_size
    );

    // EXECUTION -----------------------------------------------------------------------------------
    if !sim_opts.gui {
        // NON-GUI CASE - prosta symulacja bez wizualizacji
        println!("INFO: Running simulation without GUI...");
        let t_start = Instant::now();

        let race_result = racesim::core::handle_race::handle_race(
            &sim_pars,
            sim_opts.timestep_size,
            sim_opts.debug,
            None,
            1.0,
        )?;

        println!(
            "INFO: Execution time: {}ms",
            t_start.elapsed().as_millis()
        );

        // Wyświetl wyniki
        race_result.print_lap_and_race_times();

        // Zapisz wykres wyników do PNG
        match export_results_plot(&race_result, sim_pars.track_pars.length, false) {
            Ok(path) => println!("INFO: Wykres zapisany: {}", path),
            Err(e) => eprintln!("WARNING: Nie udało się zapisać wykresu: {}", e),
        }
    } else {
        // GUI CASE - symulacja w czasie rzeczywistym z wizualizacją
        println!("INFO: Starting GUI simulation...");
        
        // Utwórz kanał komunikacji między GUI a symulatorem
        let (tx, rx) = flume::unbounded();

        // Uruchom symulator w osobnym wątku
        let sim_opts_thread = sim_opts.clone();
        let sim_pars_thread = sim_pars.clone();

        let _ = thread::spawn(move || {
            racesim::core::handle_race::handle_race(
                &sim_pars_thread,
                sim_opts_thread.timestep_size,
                false, // debug wyłączony w GUI
                Some(&tx),
                sim_opts_thread.realtime_factor,
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