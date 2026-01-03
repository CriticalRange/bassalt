//! Global WebGPU context wrapper

use std::sync::Arc;
use parking_lot::RwLock;
use std::collections::HashMap;
use wgpu_core::global::Global;
use wgpu_core::id;
use wgpu_types as wgt;

/// Wrapper around the global WebGPU context
pub struct BasaltContext {
    inner: Arc<Global>,
    instance_desc: wgt::InstanceDescriptor,
    /// Maps TextureViewId to parent TextureId for reliable lookups
    /// This is maintained separately from wgpu-core's internal structures
    /// because we can't reliably query parent texture from a view ID
    view_to_texture_map: RwLock<HashMap<id::TextureViewId, id::TextureId>>,
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
            view_to_texture_map: RwLock::new(HashMap::new()),
        }
    }

    /// Get the inner global context
    pub fn inner(&self) -> &Arc<Global> {
        &self.inner
    }

    /// Register a texture view with its parent texture
    /// This maintains our reliable view-to-texture mapping
    pub fn register_texture_view(&self, view_id: id::TextureViewId, texture_id: id::TextureId) {
        log::debug!("Registering texture view {:?} -> texture {:?}", view_id, texture_id);
        self.view_to_texture_map.write().insert(view_id, texture_id);
    }

    /// Get the parent texture ID for a given texture view ID
    /// Returns None if the view is not registered (may have been created externally)
    pub fn get_texture_from_view(&self, view_id: id::TextureViewId) -> Option<id::TextureId> {
        self.view_to_texture_map.read().get(&view_id).copied()
    }

    /// Remove a texture view from the mapping
    pub fn unregister_texture_view(&self, view_id: id::TextureViewId) {
        log::debug!("Unregistering texture view {:?}", view_id);
        self.view_to_texture_map.write().remove(&view_id);
    }

    /// Get adapter information as a string
    pub fn get_adapter_info(&self) -> String {
        format!(
            "Basalt Renderer (WebGPU)\nAvailable backends: Vulkan, Metal, DX12, OpenGL"
        )
    }
}

impl Default for BasaltContext {
    fn default() -> Self {
        Self::new()
    }
}
