use crate::types::ProtocolPort;
use thiserror::Error;

pub type ProcCtlResult<T> = Result<T, ProcCtlError>;

#[derive(Error, Debug)]
pub enum ProcCtlError {
    #[error("network error")]
    NetworkError(#[from] netstat2::error::Error),

    #[error("configuration error {0}")]
    ConfigurationError(String),

    #[error("too few ports, got {0:?} but expected {1}")]
    TooFewPorts(Vec<ProtocolPort>, u32),
}
