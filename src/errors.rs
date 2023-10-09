use std::fmt::Display;
use thiserror::Error;

/// Our error type, wrapping miscellaneous errors from the libraries used.
#[derive(Error, Debug)]
pub enum NvTopError {
    Nvml(#[from] nvml_wrapper::error::NvmlError),
    Io(#[from] std::io::Error),
}

impl Display for NvTopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:#?}")
    }
}
