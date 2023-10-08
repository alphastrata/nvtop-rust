use clap::Parser;

#[derive(Parser)]
pub struct Cli {
    /// Number of time to wait in millis.
    #[clap(short, long, value_name = "MILLISECONDS", default_value_t = 100)]
    pub delay: u64,

    /// Enable logging to DISK, disabled by default.
    #[clap(long, value_name = "Enable Logging", default_value_t = false)]
    pub logging: bool,
}
