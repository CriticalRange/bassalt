//! Error types for Basalt renderer

use thiserror::Error;

/// Result type alias for Basalt operations
pub type Result<T> = std::result::Result<T, BasaltError>;

/// Main error type for the Basalt renderer
#[derive(Error, Debug)]
pub enum BasaltError {
    #[error("WGPU error: {0}")]
    Wgpu(String),

    #[error("Device error: {0}")]
    Device(String),

    #[error("Surface error: {0}")]
    Surface(String),

    #[error("Shader compilation failed: {0}")]
    ShaderCompilation(String),

    #[error("Shader validation failed: {0}")]
    ShaderValidation(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Device lost: {0}")]
    DeviceLost(String),

    #[error("Out of memory")]
    OutOfMemory,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Generic error: {0}")]
    Generic(String),
}
