use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use log::error;
use nvml_wrapper::Nvml;
use nvtop::{app::run, gpu, nvtop_args};

fn main() -> Result<()> {
    pretty_env_logger::init();

    let args = nvtop_args::Cli::parse();

    let nvml = Nvml::init()?;
    // Get the first `Device` (GPU) in the system
    let device = nvml.device_by_index(0)?;

    let gpu = gpu::GpuInfo { inner: &device };

    let delay = Duration::from_millis(args.delay);

    // loop {
    //     println!("GpuInfo:\n{}", gpu);
    //     std::thread::sleep(Duration::from_millis(delay));

    //     if t1.elapsed().as_secs() > 10 {
    //         break;
    //     }
    // }

    if let Err(e) = run(gpu, delay) {
        error!("{e}");
    }

    Ok(())
}
