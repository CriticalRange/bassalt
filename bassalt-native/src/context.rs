//! Global WebGPU context wrapper

use std::sync::Arc;
use wgpu_core::global::Global;
use wgpu_types as wgt;

/// Wrapper around the global WebGPU context
pub struct BasaltContext {
    inner: Arc<Global>,
    instance_desc: wgt::InstanceDescriptor,
}

impl BasaltContext {
    /// Create a new Basalt context
    pub fn new() -> Self {
        log::debug!("Initializing Basalt context");

        // Enable comprehensive validation and debugging in debug builds
        let flags = if cfg!(debug_assertions) {
            log::info!("Debug build detected - enabling advanced validation");
            wgt::InstanceFlags::advanced_debugging()
        } else {
            log::info!("Release build - using standard validation");
            wgt::InstanceFlags::debugging()
        };

        let instance_desc = wgt::InstanceDescriptor {
            backends: wgt::Backends::all(),
            flags,
            ..Default::default()
        };

        log::debug!("Instance flags: {:?}", flags);
        let global = Global::new("basalt", &instance_desc);

        Self {
            inner: Arc::new(global),
            instance_desc,
        }
    }

    /// Get the inner global context
    pub fn inner(&self) -> &Arc<Global> {
        &self.inner
    }

    /// Get adapter information as a string
    pub fn get_adapter_info(&self) -> String {
        format!(
            "Basalt Renderer (WebGPU)\nAvailable backends: Vulkan, Metal, DX12, OpenGL"
        )
    }

    /// Get supported backends
    pub fn supported_backends(&self) -> wgt::Backends {
        wgt::Backends::VULKAN
            | wgt::Backends::METAL
            | wgt::Backends::DX12
            | wgt::Backends::GL
    }
}

impl Default for BasaltContext {
    fn default() -> Self {
        Self::new()
    }
}
