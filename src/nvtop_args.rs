use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub struct Cli {
    /// Amount of time to wait in millis between updates.
    #[clap(short, long, value_name = "MILLISECONDS", default_value_t = 100)]
    pub delay: u64,

    /// Enable logging to DISK, requires a path.
    #[clap(long, value_name = "PATH")]
    pub log: Option<PathBuf>,

    /// Run as a background daemon streaming machine-readable JSONL tokens.
    #[clap(short = 'D', long)]
    pub daemon: bool,

    /// Pipe destination path for the hayaku messaging stream layer.
    #[clap(short, long, value_name = "PIPE_PATH", default_value_t = String::from("target/ptx/telemetry.stream"))]
    pub output: String,

    /// Specific application PID context target to hook and profile exclusively.
    #[clap(long, value_name = "TARGET_PID")]
    pub hook_pid: Option<u32>,
}
