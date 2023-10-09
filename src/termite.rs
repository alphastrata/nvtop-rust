use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    sync::mpsc::{self, Sender},
};

pub enum LogType {
    /// What it says on the can
    Error(String),
    /// What it says on the can
    Info(String),
    /// What it says on the can
    Debug(String),
    /// What it says on the can
    Warn(String),

    /// Send this to hangup the logger (clean it up)
    HangUp,
}

/// An abstraction to allow logging to a file, as opposed to stdout, which is hard when developing a TUI app.
pub struct LoggingHandle {
    /// The channel you send LogType(Some string) across
    pub sender: Sender<LogType>,
}

impl LoggingHandle {
    /// Gives you back a LoggingHandle
    pub fn init(log_path: PathBuf) -> LoggingHandle {
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            while let Ok(msg) = rx.recv() {
                match msg {
                    LogType::Error(msg) => Self::log_error(log_path.as_ref(), &msg),
                    LogType::Debug(msg) => Self::log_debug(log_path.as_ref(), &msg),
                    LogType::Info(msg) => Self::log_info(log_path.as_ref(), &msg),
                    LogType::Warn(msg) => Self::log_warn(log_path.as_ref(), &msg),
                    LogType::HangUp => {
                        drop(rx);
                        break;
                    }
                }
            }
        });

        LoggingHandle { sender: tx }
    }

    /// Creates a useless, empty (as the receiver is hung up before you get Self back), a nice dummy for when the app runs without logging.
    pub fn empty() -> LoggingHandle {
        let (tx, rx) = mpsc::channel();
        drop(rx);
        LoggingHandle { sender: tx }
    }

    pub fn error(&self, msg: &str) {
        _ = self.sender.send(LogType::Error(msg.to_string()));
    }
    pub fn info(&self, msg: &str) {
        _ = self.sender.send(LogType::Info(msg.to_string()));
    }
    pub fn debug(&self, msg: &str) {
        _ = self.sender.send(LogType::Debug(msg.to_string()));
    }
    pub fn warn(&self, msg: &str) {
        _ = self.sender.send(LogType::Warn(msg.to_string()));
    }

    #[inline(always)]
    fn log_error(log_path: &Path, msg: &str) {
        let log_line = format!(" ERROR {}\n", msg);
        Self::write_to_log(log_path, &log_line);
    }
    #[inline(always)]
    fn log_info(log_path: &Path, msg: &str) {
        let log_line = format!(" INFO  {}\n", msg);
        Self::write_to_log(log_path, &log_line);
    }

    #[inline(always)]
    fn log_debug(log_path: &Path, msg: &str) {
        let log_line = format!(" DEBUG {}\n", msg);
        Self::write_to_log(log_path, &log_line);
    }

    #[inline(always)]
    fn log_warn(log_path: &Path, msg: &str) {
        let log_line = format!(" WARN {}\n", msg);
        Self::write_to_log(log_path, &log_line);
    }

    fn write_to_log(log_path: &Path, msg: &str) {
        // Open the log file in append mode, creating it if it doesn't exist
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path).unwrap_or_else(|_| panic!("Unable to create {}.\nAs a result of this we cannot write logs. So the app will crash.", log_path.display()));

        // Write the message to the log file
        file.write_all(msg.as_bytes())
            .expect("Unable to write log to file -- We chose to exit the app when this happens.");
    }
}
