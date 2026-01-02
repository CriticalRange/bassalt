//! Error types for Basalt renderer
//!
//! Provides comprehensive error handling with detailed context for debugging.
//! Inspired by wgpu's error reporting patterns with additional Bassalt-specific errors.

use std::fmt;
use thiserror::Error;

/// Result type alias for Basalt operations
pub type Result<T> = std::result::Result<T, BasaltError>;

/// Main error type for the Basalt renderer
///
/// Each variant provides specific context about what went wrong,
/// making it easier to diagnose and fix issues.
#[derive(Error, Debug)]
pub enum BasaltError {
    // === Core wgpu errors ===
    #[error("WGPU internal error: {0}")]
    Wgpu(String),

    #[error("WGPU validation error: {0}")]
    Validation(String),

    // === Device errors ===
    #[error("Failed to create device: {reason}")]
    DeviceCreation {
        reason: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Device lost: {reason}")]
    DeviceLost { reason: String },

    #[error("No suitable GPU adapter found")]
    NoAdapterFound,

    // === Surface errors ===
    #[error("Surface error: {reason}")]
    Surface {
        reason: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Surface configuration failed: {0}")]
    SurfaceConfiguration(String),

    // === Shader errors ===
    #[error("Shader compilation failed: {shader_name}")]
    ShaderCompilation {
        shader_name: String,
        error: String,
        stage: String, // "vertex", "fragment", "compute"
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Shader validation failed: {shader_name}: {error}")]
    ShaderValidation { shader_name: String, error: String },

    #[error("Shader parse error: {error}")]
    ShaderParse {
        error: String,
        line: Option<usize>,
        column: Option<usize>,
    },

    // === Pipeline errors ===
    #[error("Pipeline creation failed: {pipeline_name}")]
    PipelineCreation {
        pipeline_name: String,
        error: String,
        validation_errors: Vec<String>,
    },

    #[error("Pipeline layout creation failed: {0}")]
    PipelineLayout(String),

    #[error("Bind group layout mismatch: expected {expected:?}, got {actual:?} at binding {binding}")]
    BindGroupLayoutMismatch {
        expected: String,
        actual: String,
        binding: u32,
    },

    // === Resource errors ===
    #[error("Resource not found: {resource_type} '{name}'")]
    NotFound { resource_type: String, name: String },

    #[error("Resource creation failed: {resource_type}: {reason}")]
    ResourceCreation {
        resource_type: String,
        reason: String,
    },

    #[error("Buffer size mismatch: shader expects {shader_size} bytes, but buffer is {buffer_size} bytes")]
    BufferSizeTooSmall { shader_size: u64, buffer_size: u64 },

    #[error("Binding size too small: shader requires {shader_size} bytes, bound {bound_size} bytes at binding {binding}")]
    BindingSizeTooSmall { shader_size: u64, bound_size: u64, binding: u32 },

    #[error("Texture dimension mismatch: expected {expected:?}, got {actual:?} at binding {binding}")]
    TextureDimensionMismatch {
        expected: String,
        actual: String,
        binding: u32,
    },

    // === Memory errors ===
    #[error("Out of GPU memory: {context}")]
    OutOfMemory { context: String },

    #[error("Buffer allocation failed: requested {requested} bytes exceeds max buffer size {max_size}")]
    BufferAllocationFailed { requested: u64, max_size: u64 },

    // === Parameter errors ===
    #[error("Invalid parameter: {parameter}: {reason}")]
    InvalidParameter { parameter: String, reason: String },

    #[error("Invalid handle: {handle_type} handle {handle:#x}")]
    InvalidHandle { handle_type: String, handle: u64 },

    // === Render pass errors ===
    #[error("Render pass error: {0}")]
    RenderPass(String),

    #[error("No color attachment provided for render pass")]
    NoColorAttachment,

    #[error("Depth stencil state mismatch: pipeline expects {pipeline_has_depth}, render pass has {pass_has_depth}")]
    DepthStencilMismatch {
        pipeline_has_depth: bool,
        pass_has_depth: bool,
    },

    // === JNI errors ===
    #[error("JNI error: {0}")]
    Jni(String),

    #[error("Null pointer: {context}")]
    NullPointer { context: String },

    // === IO errors ===
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    // === Generic errors ===
    #[error("Generic error: {0}")]
    Generic(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

// Helper constructors for common error patterns
impl BasaltError {
    /// Create a device creation error
    pub fn device_creation(reason: impl fmt::Display) -> Self {
        Self::DeviceCreation {
            reason: reason.to_string(),
            source: None,
        }
    }

    /// Create a surface error
    pub fn surface(reason: impl fmt::Display) -> Self {
        Self::Surface {
            reason: reason.to_string(),
            source: None,
        }
    }

    /// Create a shader compilation error
    pub fn shader_compilation(
        shader_name: impl fmt::Display,
        error: impl fmt::Display,
        stage: impl fmt::Display,
    ) -> Self {
        Self::ShaderCompilation {
            shader_name: shader_name.to_string(),
            error: error.to_string(),
            stage: stage.to_string(),
            source: None,
        }
    }

    /// Create a resource creation error
    pub fn resource_creation(resource_type: impl fmt::Display, reason: impl fmt::Display) -> Self {
        Self::ResourceCreation {
            resource_type: resource_type.to_string(),
            reason: reason.to_string(),
        }
    }

    /// Create an invalid parameter error
    pub fn invalid_parameter(parameter: impl fmt::Display, reason: impl fmt::Display) -> Self {
        Self::InvalidParameter {
            parameter: parameter.to_string(),
            reason: reason.to_string(),
        }
    }

    /// Create an out-of-memory error
    pub fn out_of_memory(context: impl fmt::Display) -> Self {
        Self::OutOfMemory {
            context: context.to_string(),
        }
    }
}
