use crate::types::ProtocolPort;
use thiserror::Error;

/// A result type to return `ProcCtlError`s
pub type ProcCtlResult<T> = Result<T, ProcCtlError>;

/// Custom error type for proc-ctl
#[derive(Error, Debug)]
pub enum ProcCtlError {
    /// There was an error communicating with the network
    #[error("network error")]
    NetworkError(#[from] netstat2::error::Error),

    /// The user made an error using the API, a more specific error message will be provided
    #[error("configuration error {0}")]
    ConfigurationError(String),

    /// Fewer ports than expected were found on the matched process
    #[error("too few ports, got {0:?} but expected {1}")]
    TooFewPorts(Vec<ProtocolPort>, u32),
}
