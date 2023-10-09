use std::{path::PathBuf, time::Duration};

use anyhow::Result;
use clap::Parser;
use nvml_wrapper::Nvml;

use nvtop::{app::run, errors::NvTopError, nvtop_args, termite::LoggingHandle};

fn main() -> Result<(), NvTopError> {
    let args = nvtop_args::Cli::parse();

    let mut lh = LoggingHandle::empty();
    if args.log.is_some() {
        let log_path = match args.log {
            Some(lp) => lp,
            None => PathBuf::from("nvtop.log"),
        };
        lh = LoggingHandle::init(log_path);
    }

    // Init the GPU management-layer
    let nvml = Nvml::init()?;
    lh.debug("Nvml init success");

    if let Err(e) = run(nvml, Duration::from_millis(args.delay), &lh) {
        lh.error(&format!("app::run() -> {e}"));
    }

    Ok(())
}
