use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use log::{error, trace};
use nvml_wrapper::Nvml;

use nvtop::{app::run, errors::NvTopError, nvtop_args, termite::setup_logger};

fn main() -> Result<(), NvTopError> {
    let args = nvtop_args::Cli::parse();

    // If they've used the --log arg we write all logs to disk.
    if args.log.is_some() {
        setup_logger(args.log)?;
    } else {
        pretty_env_logger::init(); // If they've got RUST_LOG=trace on the TUI is ruined.
    }

    // Init the GPU management-layer
    let nvml = Nvml::init()?;
    trace!("Nvml init success");

    if let Err(e) = run(nvml, Duration::from_millis(args.delay)) {
        error!("app::run() -> {e}");
    }

    Ok(())
}
