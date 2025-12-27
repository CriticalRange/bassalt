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

        let instance_desc = wgt::InstanceDescriptor {
            backends: wgt::Backends::all(),
            ..Default::default()
        };

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
        // This would be populated when an adapter is selected
        // For now, return basic info
        format!(
            "Basalt Renderer (WebGPU)\nAvailable backends: Vulkan, Metal, DX12, OpenGL"
        )
    }

    /// Get supported backends
    pub fn supported_backends(&self) -> wgt::Backends {
        wgt::Backends::from_bits(
            wgt::Backends::Vulkan.bits()
                | wgt::Backends::METAL.bits()
                | wgt::Backends::DX12.bits()
                | wgt::Backends::GL.bits(),
        )
        .unwrap_or_else(|| wgt::Backends::all())
    }
}

impl Default for BasaltContext {
    fn default() -> Self {
        Self::new()
    }
}
