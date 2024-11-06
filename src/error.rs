use crate::types::ProtocolPort;
use thiserror::Error;

/// A result type to return `ProcCtlError`s
pub type ProcCtlResult<T> = Result<T, ProcCtlError>;

/// Custom error type for proc-ctl
#[derive(Error, Debug)]
pub enum ProcCtlError {
    /// An error occurred while searching process information
    #[cfg(target_os = "linux")]
    #[error("process error")]
    ProcessError(#[from] procfs::ProcError),

    /// An error occurred while searching process information
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    #[error("process error")]
    ProcessError(String),

    /// The user made an error using the API, a more specific error message will be provided
    #[error("configuration error {0}")]
    ConfigurationError(String),

    /// Fewer ports than expected were found on the matched process
    #[error("too few ports, got {0:?} but expected {1}")]
    TooFewPorts(Vec<ProtocolPort>, usize),

    /// Too few children were found on the matched process
    #[error("too few children, got {0} but expected {1}")]
    TooFewChildren(usize, usize),
}
