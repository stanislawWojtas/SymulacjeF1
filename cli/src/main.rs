use clap::Parser;
use flume;
use gui::core::gui::RacePlot;
use racesim::pre::read_sim_pars::read_sim_pars;
use racesim::pre::sim_opts::SimOpts;
use std::thread;
use std::time::Instant;

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