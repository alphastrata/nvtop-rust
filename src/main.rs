use std::time::Duration;

use anyhow::Result;
use nvml_wrapper::Nvml;

use nvtop::{app::run, daemon_processor, errors::NvTopError, termite::LoggingHandle};

fn main() -> Result<(), NvTopError> {
    let cli_args: nvtop::nvtop_args::Cli = argh::from_env();

    let mut lh = LoggingHandle::empty();
    if let Some(log_path) = &cli_args.log {
        lh = LoggingHandle::init(log_path.clone());
    }

    let nvml = Nvml::init()?;
    lh.debug("Nvml base layer initialized successfully");

    if cli_args.daemon {
        lh.info("Daemon operational flag detected. Launching hayaku pipeline stream...");
        daemon_processor::execute_streaming_daemon(
            &nvml,
            Duration::from_millis(cli_args.delay),
            &cli_args.output,
            cli_args.hook_pid,
            &lh,
        )?;
    } else if let Err(e) = run(
        nvml,
        Duration::from_millis(cli_args.delay),
        &lh,
        cli_args.hook_pid,
    ) {
        lh.error(&format!("app::run() -> {e}"));
    }

    Ok(())
}
