use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use log::{error, trace};
use nvml_wrapper::{
    enum_wrappers::device::{Clock, ClockId},
    Nvml, error::NvmlError,
};

use nvtop::{app::run, errors::NvTopError, gpu::GpuInfo, nvtop_args, termite::setup_logger};

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

    // Get the available devices
    let devices = nvtop::gpu::list_available_gpus(&nvml)?;

    // Get the first `Device` (GPU) in the system
    let gpu = &devices[0];
    // panic!("Starting TUI with GPU = {gpu}\n");

    if let Err(e) = run(&gpu, Duration::from_millis(args.delay)) {
        error!("app::run() -> {e}");
    }

    Ok(())
}
