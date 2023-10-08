use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub struct Cli {
    /// Amount of time to wait in millis.
    /// nvtop --delay 1000  # to run with a delay of 1s.
    /// nvtop -d 200        # short flags are supported.
    #[clap(short, long, value_name = "MILLISECONDS", default_value_t = 100)]
    pub delay: u64,

    /// Enable logging to DISK, disabled by default, requires a path that you want to log to, i.e:
    /// `nvtop --log ~/Documents/nvtop.log`
    #[clap(long, value_name = "Enable Logging")]
    pub log: Option<PathBuf>,
}
