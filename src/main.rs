use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use log::error;
use nvml_wrapper::{
    enum_wrappers::device::{Clock, ClockId},
    Nvml,
};

use nvtop::{app::run, errors::NvTopError, gpu::GpuInfo, nvtop_args};
use termite::setup_logger;

fn main() -> Result<(), NvTopError> {
    let args = nvtop_args::Cli::parse();

    if args.logging {
        //TODO: add me.
        setup_logger()?;
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

pub mod termite {

    use std::env;

    /// Termite is a simple logging implementation, powered by the fern crate.
    #[allow(unused_imports)]
    use log::{debug, error, info, trace, warn};

    /// Initialise termite.log, after initilisation you'll find it works EXACTLY as most of the logging
    /// crates you're used to.
    //
    /// #Examples:
    /// // it works exactly like the std library's log crate.
    /// use fern;
    /// use log::{debug, error, info, trace, warn};
    ///
    /// info!("Info you'd like to log");
    /// warn!("Warning you'd like to log");
    /// error!("Error you'd like to log");
    /// trace!("Thing you'd like to trace");
    pub fn setup_logger() -> Result<(), fern::InitError> {
        match env::var("RUST_LOG") {
            Ok(_) => {
                std::env::set_var("RUST_LOG", "trace");
                warn!("$RUST_LOG was not set in $ENV");
                info!("$RUST_LOG set to 'trace'")
            }
            Err(e) => {
                error!("Unable to get, or set $RUST_LOG\n{e}");
            }
        };

        let termite_path = format!("termite_{}.log", chrono::Local::now().format("%Y-%m-%d"));
        fern::Dispatch::new()
            .format(|out, message, record| {
                out.finish(format_args!(
                    "{}[{}][{}][{}] {}",
                    chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                    record.level(),             // info!() or error!() etc.
                    record.target(),            // The file that spawned this entry into the log.
                    record.line().unwrap_or(0), // The line number in said file.
                    message
                ))
            })
            .chain(std::io::stdout())
            .chain(fern::log_file(termite_path)?)
            .apply()?;

        trace!("Logger setup complete.");
        Ok(())
    }
}
