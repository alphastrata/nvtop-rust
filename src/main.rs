use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use nvml_wrapper::Nvml;

use nvtop::{app::run, errors::NvTopError, nvtop_args, termite::LoggingHandle, daemon_processor};

fn main() -> Result<(), NvTopError> {
    let args = nvtop_args::Cli::parse();

    let mut lh = LoggingHandle::empty();
    if let Some(log_path) = &args.log {
        lh = LoggingHandle::init(log_path.clone());
    }

    // Init the GPU management-layer
    let nvml = Nvml::init()?;
    lh.debug("Nvml base layer initialized successfully");

    if args.daemon {
        lh.info("Daemon operational flag detected. Launching hayaku pipeline stream...");
        daemon_processor::execute_streaming_daemon(
            &nvml,
            Duration::from_millis(args.delay),
            &args.output,
            args.hook_pid,
            &lh,
        )?;
    } else {
        if let Err(e) = run(nvml, Duration::from_millis(args.delay), &lh, args.hook_pid) {
            lh.error(&format!("app::run() -> {e}"));
        }
    }

    Ok(())
}
