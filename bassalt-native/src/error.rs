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

impl From<wgpu_core::instance::RequestDeviceError> for BasaltError {
    fn from(e: wgpu_core::instance::RequestDeviceError) -> Self {
        BasaltError::Device(format!("Failed to request device: {}", e))
    }
}

impl From<wgpu_core::instance::Error> for BasaltError {
    fn from(e: wgpu_core::instance::Error) -> Self {
        BasaltError::Wgpu(format!("Instance error: {}", e))
    }
}

impl From<wgpu_core::device::CreateBufferError> for BasaltError {
    fn from(e: wgpu_core::device::CreateBufferError) -> Self {
        BasaltError::Wgpu(format!("Failed to create buffer: {}", e))
    }
}

impl From<wgpu_core::device::CreateTextureError> for BasaltError {
    fn from(e: wgpu_core::device::CreateTextureError) -> Self {
        BasaltError::Wgpu(format!("Failed to create texture: {}", e))
    }
}

impl From<wgpu_core::device::CreateSamplerError> for BasaltError {
    fn from(e: wgpu_core::device::CreateSamplerError) -> Self {
        BasaltError::Wgpu(format!("Failed to create sampler: {}", e))
    }
}

impl From<naga::front::glsl::ParseError> for BasaltError {
    fn from(e: naga::front::glsl::ParseError) -> Self {
        BasaltError::ShaderCompilation(format!("GLSL parse error: {}", e))
    }
}

impl From<naga::valid::ValidationError> for BasaltError {
    fn from(e: naga::valid::ValidationError) -> Self {
        BasaltError::ShaderValidation(format!("Validation error: {}", e))
    }
}

impl From<naga::back::wgsl::Error> for BasaltError {
    fn from(e: naga::back::wgsl::Error) -> Self {
        BasaltError::ShaderCompilation(format!("WGSL generation error: {}", e))
    }
}
