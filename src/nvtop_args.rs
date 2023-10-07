use clap::Parser;

#[derive(Parser)]
pub struct Cli {
    /// Number of time to wait in millis.
    #[clap(short, long, value_name = "MILLISECONDS", default_value_t = 100)]
    pub delay: u64,
}
