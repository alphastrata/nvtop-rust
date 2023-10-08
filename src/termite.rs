use std::{env, path::PathBuf};

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
pub fn setup_logger(p: Option<PathBuf>) -> Result<(), fern::InitError> {
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

    let termite_path = match p {
        Some(p) => p,
        None => std::path::PathBuf::from("nvtop.log"),
    };

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
