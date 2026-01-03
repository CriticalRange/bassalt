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

    /// Enhanced wgpu error with full context preservation
    ///
    /// This variant preserves the original error context from wgpu-core
    /// instead of losing it in `format!("{:?}", e)` calls.
    /// Use this for all wgpu-core 27.0 errors to get better debugging.
    #[error("WGPU error in '{context}': {error}")]
    WgpuWithContext {
        /// What operation was being attempted when the error occurred
        context: String,
        /// The underlying wgpu-core error message
        error: String,
        /// Error type category for filtering/handling
        error_type: WgpuErrorType,
        /// Original source error if available
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

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

/// Error type categories for wgpu-core errors
///
/// These categories allow filtering and handling of specific error types
/// without needing to match on error strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WgpuErrorType {
    /// Device-level errors (creation, lost, etc.)
    Device,
    /// Surface/swapchain errors
    Surface,
    /// Shader compilation/validation errors
    Shader,
    /// Pipeline creation errors
    Pipeline,
    /// Resource creation errors (buffers, textures, etc.)
    Resource,
    /// Binding/layout errors
    Binding,
    /// Memory allocation errors
    Memory,
    /// Validation errors (usage constraints, etc.)
    Validation,
    /// Render pass errors
    RenderPass,
    /// Unknown or uncategorized error
    Unknown,
}

impl WgpuErrorType {
    /// Infer error type from common error message patterns
    pub fn from_error_message(msg: &str) -> Self {
        let msg_lower = msg.to_lowercase();

        if msg_lower.contains("device") || msg_lower.contains("adapter") {
            Self::Device
        } else if msg_lower.contains("surface") || msg_lower.contains("swapchain") {
            Self::Surface
        } else if msg_lower.contains("shader") || msg_lower.contains("wgsl") {
            Self::Shader
        } else if msg_lower.contains("pipeline") || msg_lower.contains("layout") {
            Self::Pipeline
        } else if msg_lower.contains("buffer") || msg_lower.contains("texture") {
            Self::Resource
        } else if msg_lower.contains("binding") || msg_lower.contains("bind group") {
            Self::Binding
        } else if msg_lower.contains("memory") || msg_lower.contains("allocation") {
            Self::Memory
        } else if msg_lower.contains("valid") || msg_lower.contains("constraint") {
            Self::Validation
        } else if msg_lower.contains("render pass") {
            Self::RenderPass
        } else {
            Self::Unknown
        }
    }
}

// Helper constructors for common error patterns
impl BasaltError {
    /// Create an enhanced wgpu error with full context preservation
    ///
    /// Use this instead of `Self::Wgpu(format!("{:?}", e))` to preserve
    /// error context from wgpu-core 27.0.
    ///
    /// # Example
    /// ```rust
    /// let (buffer_id, error) = context.device_create_buffer(...);
    /// if let Some(e) = error {
    ///     return Err(BasaltError::wgpu_context(
    ///         "buffer creation",
    ///         format!("{:?}", e),
    ///     ));
    /// }
    /// ```
    pub fn wgpu_context(context: impl fmt::Display, error: impl fmt::Display) -> Self {
        let error_str = error.to_string();
        let error_type = WgpuErrorType::from_error_message(&error_str);

        Self::WgpuWithContext {
            context: context.to_string(),
            error: error_str,
            error_type,
            source: None,
        }
    }

    /// Create an enhanced wgpu error with explicit error type
    ///
    /// Use when you know the specific error category for better filtering.
    pub fn wgpu_context_with_type(
        context: impl fmt::Display,
        error: impl fmt::Display,
        error_type: WgpuErrorType,
    ) -> Self {
        Self::WgpuWithContext {
            context: context.to_string(),
            error: error.to_string(),
            error_type,
            source: None,
        }
    }

    /// Create an enhanced wgpu error from a wgpu-core error with source chain
    pub fn wgpu_context_with_source(
        context: impl fmt::Display,
        error: impl fmt::Display,
        source: Box<dyn std::error::Error + Send + Sync>,
    ) -> Self {
        let error_str = error.to_string();
        let error_type = WgpuErrorType::from_error_message(&error_str);

        Self::WgpuWithContext {
            context: context.to_string(),
            error: error_str,
            error_type,
            source: Some(source),
        }
    }
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
