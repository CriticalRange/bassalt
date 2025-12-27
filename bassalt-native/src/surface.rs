//! Window surface handling

use std::sync::Arc;
use wgpu_core::id;
use wgpu_types as wgt;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

use crate::context::BasaltContext;
use crate::error::Result;

/// Wrapper for a surface
pub struct BasaltSurface {
    context: Arc<BasaltContext>,
    surface_id: id::SurfaceId,
    config: Option<wgt::SurfaceConfiguration>,
}

impl BasaltSurface {
    /// Create a surface from a raw window handle
    pub fn from_raw_window_handle(
        context: Arc<BasaltContext>,
        window_handle: RawWindowHandle,
    ) -> Result<Self> {
        let desc = wgt::SurfaceDescriptor {
            label: Some("Basalt Surface"),
            desired_format: wgt::TextureFormat::Bgra8UnormSrgb,
            usage: wgt::TextureUsages::RENDER_ATTACHMENT,
            present_mode: wgt::PresentMode::Fifo,
            alpha_mode: wgt::CompositeAlphaMode::Opaque,
            view_formats: vec![],
        };

        let surface_id = context
            .inner()
            .instance_create_surface(window_handle, desc)?;

        Ok(Self {
            context,
            surface_id,
            config: None,
        })
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
        let capabilities = self
            .context
            .inner()
            .surface_get_capabilities(self.surface_id, adapter_id);

        capabilities
            .map(|caps| caps.formats.to_vec())
            .unwrap_or_default()
    }

    /// Get the supported modes for this surface
    pub fn get_supported_modes(
        &self,
        adapter_id: id::AdapterId,
    ) -> Vec<wgt::PresentMode> {
        let capabilities = self
            .context
            .inner()
            .surface_get_capabilities(self.surface_id, adapter_id);

        capabilities
            .map(|caps| caps.present_modes.to_vec())
            .unwrap_or_default()
    }

    /// Configure the surface
    pub fn configure(
        &mut self,
        device_id: id::DeviceId,
        config: wgt::SurfaceConfiguration,
    ) -> Result<()> {
        self.context
            .inner()
            .surface_configure(self.surface_id, device_id, &config)?;

        self.config = Some(config);
        Ok(())
    }

    /// Get the current texture
    pub fn get_current_texture(&self, device_id: id::DeviceId) -> Result<id::TextureId> {
        let texture_id = self
            .context
            .inner()
            .surface_get_current_texture(self.surface_id, device_id)
            .map_err(|e| crate::error::BasaltError::Surface(format!("Failed to get texture: {:?}", e)))?;

        Ok(texture_id)
    }

    /// Present the surface
    pub fn present(&self, device_id: id::DeviceId) -> Result<()> {
        self.context
            .inner()
            .surface_present(self.surface_id, device_id)
            .map_err(|e| crate::error::BasaltError::Surface(format!("Failed to present: {:?}", e)))
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
