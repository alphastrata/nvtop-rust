use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use log::error;
use nvml_wrapper::{
    enum_wrappers::device::{Clock, ClockId},
    Nvml,
};

use nvtop::{app::run, errors::NvTopError, gpu::GpuInfo, nvtop_args, termite::setup_logger};

fn main() -> Result<(), NvTopError> {
    let args = nvtop_args::Cli::parse();

    if args.log.is_some() {
        setup_logger(args.log)?;
    } else {
        pretty_env_logger::init();
    }
    let nvml = Nvml::init()?;

    // Get the first `Device` (GPU) in the system
    let device = nvml.device_by_index(0)?;

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

    let max_memory_clock: Vec<u32> = device.supported_memory_clocks()?;
    let max_clock = device.clock(Clock::Graphics, ClockId::CustomerMaxBoost)?;

    let gpu = GpuInfo {
        inner: &device,
        max_memory_clock: max_memory_clock.into_iter().max().unwrap_or_default(), // FIXME:
        max_core_clock: max_clock,
        card_type,
        driver_version,
        cuda_version,
        misc,
    };

    if let Err(e) = run(gpu, Duration::from_millis(args.delay)) {
        error!("{e}");
    }

    Ok(())
}
