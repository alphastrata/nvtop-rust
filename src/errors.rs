use std::fmt::Display;

use thiserror::Error;

// #[derive(Error, Debug)]
// pub enum DataStoreError {
//     #[error("data store disconnected")]
//     Disconnect(#[from] io::Error),
//     #[error("the data for key `{0}` is not available")]
//     Redaction(String),
//     #[error("invalid header (expected {expected:?}, found {found:?})")]
//     InvalidHeader {
//         expected: String,
//         found: String,
//     },
//     #[error("unknown data store error")]
//     Unknown,
// }

#[derive(Error, Debug)]
pub enum NvTopError {
    #[error(transparent)]
    NvmlError(nvml_wrapper::error::NvmlError),
    Io(#[from] std::io::Error),
}

impl Display for NvTopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:#?}")
    }
}
