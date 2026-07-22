use thiserror::Error;

/// Central domain error taxonomy for the Vector Core geometry engine.
#[derive(Error, Debug)]
pub enum VectorCoreError {
    #[error("Invalid geometric primitive: {0}")]
    InvalidGeometry(String),

    #[error("Topological structure error: {0}")]
    TopologyError(String),

    #[error("Calculation precision error: {0}")]
    PrecisionError(String),

    #[error("IO failure during geometry serialization: {0}")]
    IoError(String),
}

pub type CoreResult<T> = Result<T, VectorCoreError>;
