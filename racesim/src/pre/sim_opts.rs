use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser, Clone)]
#[clap(
    version = "0.1.0",
    author = "Alexander Heilmeier <alexander.heilmeier@tum.de>",
    name = "RS-TD",
    about = "A time-discrete race simulator written in Rust"
)]
pub struct SimOpts {
    // FLAGS ---------------------------------------------------------------------------------------
    /// Activate debug printing (only for non-GUI mode)
    #[clap(short, long)]
    pub debug: bool,

    /// Activate GUI - race will be simulated in real-time with visualization
    #[clap(short, long)]
    pub gui: bool,

    // OPTIONS -------------------------------------------------------------------------------------
    /// Set number of simulation runs (only for non-GUI mode, ignored in GUI mode)
    #[clap(short, long, default_value = "1")]
    pub no_sim_runs: u32,

    /// Set path to the simulation parameter file (OPTIONAL: if not set, uses hardcoded 2-car race)
    #[clap(short, long)]
    pub parfile_path: Option<PathBuf>, 

    /// Set real-time factor (only relevant in GUI mode)
    #[clap(short, long, default_value = "1.0")]
    pub realtime_factor: f64,

    /// Set simulation timestep size in seconds, should be in the range [0.001, 1.0]
    #[clap(short, long, default_value = "0.1")]
    pub timestep_size: f64,
}