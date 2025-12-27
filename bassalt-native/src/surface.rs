//! Window surface handling

use std::borrow::Cow;
use std::sync::Arc;
use wgpu_core::id;
use wgpu_types as wgt;
use raw_window_handle::RawWindowHandle;

use crate::context::BasaltContext;
use crate::error::{BasaltError, Result};

/// Wrapper for a surface
pub struct BasaltSurface {
    context: Arc<BasaltContext>,
    surface_id: id::SurfaceId,
    config: Option<wgt::SurfaceConfiguration<Vec<wgt::TextureFormat>>>,
}

impl BasaltSurface {
    /// Create a surface from a raw window handle
    /// Note: This is a simplified implementation. Full implementation would
    /// need proper window handle wrapper types for raw-window-handle 0.6
    pub fn from_raw_window_handle(
        context: Arc<BasaltContext>,
        _window_handle: RawWindowHandle,
    ) -> Result<Self> {
        // For wgpu-core 27, surface creation requires proper window handle wrappers
        // This is a placeholder - real implementation needs platform-specific code
        // The actual surface creation would use instance_create_surface with proper handles
        
        // For now, we'll return an error indicating this needs platform-specific implementation
        Err(BasaltError::Surface(
            "Surface creation requires platform-specific implementation for wgpu-core 27".into()
        ))
    }
    
    /// Create a surface with an existing surface ID (for testing or direct creation)
    pub fn from_id(context: Arc<BasaltContext>, surface_id: id::SurfaceId) -> Self {
        Self {
            context,
            surface_id,
            config: None,
        }
    }

    /// Get the surface ID
    pub fn id(&self) -> id::SurfaceId {
        self.surface_id
    }

    /// Get the supported formats for this surface
    pub fn get_supported_formats(
        &self,
        adapter_id: id::AdapterId,
    ) -> Vec<wgt::TextureFormat> {
        match self
            .context
            .inner()
            .surface_get_capabilities(self.surface_id, adapter_id)
        {
            Ok(caps) => caps.formats.to_vec(),
            Err(_) => vec![],
        }
    }

    /// Get the supported modes for this surface
    pub fn get_supported_modes(
        &self,
        adapter_id: id::AdapterId,
    ) -> Vec<wgt::PresentMode> {
        match self
            .context
            .inner()
            .surface_get_capabilities(self.surface_id, adapter_id)
        {
            Ok(caps) => caps.present_modes.to_vec(),
            Err(_) => vec![],
        }
    }

    /// Configure the surface
    pub fn configure(
        &mut self,
        device_id: id::DeviceId,
        config: wgt::SurfaceConfiguration<Vec<wgt::TextureFormat>>,
    ) -> Result<()> {
        self.context
            .inner()
            .surface_configure(self.surface_id, device_id, &config);

        self.config = Some(config);
        Ok(())
    }

    /// Get the current texture
    pub fn get_current_texture(&self) -> Result<wgpu_core::present::SurfaceOutput> {
        self.context
            .inner()
            .surface_get_current_texture(self.surface_id, None)
            .map_err(|e| BasaltError::Surface(format!("Failed to get texture: {:?}", e)))
    }

    /// Present the surface
    pub fn present(&self, _queue_id: id::QueueId) -> Result<()> {
        self.context
            .inner()
            .surface_present(self.surface_id)
            .map_err(|e| BasaltError::Surface(format!("Failed to present: {:?}", e)))?;
        Ok(())
    }

    /// Drop the surface
    pub fn drop_surface(&self) {
        self.context.inner().surface_drop(self.surface_id);
    }
}

impl Drop for BasaltSurface {
    fn drop(&mut self) {
        self.drop_surface();
    }
}
