use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use log::{error, trace};
use nvml_wrapper::{
    enum_wrappers::device::{Clock, ClockId},
    Nvml,
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

    // Get the first `Device` (GPU) in the system
    let device = nvml.device_by_index(0)?;
    trace!("Compatible GPU found at [0]");

    // Do some setup for things that will _not_ change, i.e driver version etc.
    let card_type = format!("{:?}", device.brand()?);
    let driver_version = device.nvml().sys_driver_version()?;
    let cuda_version = device.nvml().sys_cuda_driver_version()? as f32;

    let misc = format!(
        "Card: {:?}    Driver Version: {}    CUDA Version: {}",
        card_type,
        driver_version,
        cuda_version / 1000.0
    );
    trace!("Setting misc = {misc}");

    let max_memory_clock: Vec<u32> = device.supported_memory_clocks()?;
    let max_clock = match device.clock(Clock::Graphics, ClockId::CustomerMaxBoost) {
        Ok(max_clock) => {
            trace!("CustomerMaxBoost is available!");
            max_clock
        }
        Err(e) => {
            error!("{e} CustomerMaxBoost is UNAVAILABLE");
            1
        }
    };

    let gpu = GpuInfo {
        inner: &device,
        max_memory_clock: max_memory_clock.into_iter().max().unwrap_or_default(), // FIXME:
        max_core_clock: max_clock,
        card_type,
        driver_version,
        cuda_version,
        misc,
    };

    trace!("Starting TUI with GPU = {gpu}\n");
    if let Err(e) = run(&gpu, Duration::from_millis(args.delay)) {
        error!("{e}");
    }

    Ok(())
}
