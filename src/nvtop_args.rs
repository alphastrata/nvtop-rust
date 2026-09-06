use std::path::PathBuf;

use argh::FromArgs;

#[derive(FromArgs)]
/// Command line arguments for nvtop.
pub struct Cli {
    /// amount of time to wait in millis between updates.
    #[argh(option, short = 'd', default = "100")]
    pub delay: u64,

    /// enable logging to DISK, requires a path.
    #[argh(option, short = 'l')]
    pub log: Option<PathBuf>,

    /// run as a background daemon streaming machine-readable JSONL tokens.
    #[argh(switch)]
    pub daemon: bool,

    /// pipe destination path for the hayagaku messaging stream layer.
    #[argh(option, default = "\"target/ptx/telemetry.stream\".to_string()")]
    pub output: String,

    /// specific application PID context target to hook and profile exclusively.
    #[argh(option)]
    pub hook_pid: Option<u32>,

    /// maximum file stream size in MiB before older lines are dropped.
    #[argh(option, default = "1")]
    pub stream_cap_mb: u64,
}
