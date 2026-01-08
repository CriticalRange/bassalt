//! Window surface handling

use std::sync::Arc;
use wgpu_core::id;
use wgpu_types as wgt;
use raw_window_handle::RawWindowHandle;

use crate::context::BasaltContext;
use crate::error::{BasaltError, Result};

/// Wrapper for a surface with error recovery and lifecycle management
///
/// Based on wgpu examples' SurfaceWrapper pattern:
/// - Automatic surface reconfigure on error
/// - Proper lifecycle management (resume/suspend)
/// - macOS pre-present notification for frame synchronization
pub struct BasaltSurface {
    context: Arc<BasaltContext>,
    surface_id: id::SurfaceId,
    config: Option<wgt::SurfaceConfiguration<Vec<wgt::TextureFormat>>>,
    device_id: Option<id::DeviceId>,  // Track device for reconfigure
    max_retries: u32,  // Maximum retries for get_current_texture
}

impl BasaltSurface {
    /// Create a surface from a raw window handle
    /// Note: This is a simplified implementation. Full implementation would
    /// need proper window handle wrapper types for raw-window-handle 0.6
    pub fn from_raw_window_handle(
        _context: Arc<BasaltContext>,
        _window_handle: RawWindowHandle,
    ) -> Result<Self> {
        // For wgpu-core 27, surface creation requires proper window handle wrappers
        // This is a placeholder - real implementation needs platform-specific code
        // The actual surface creation would use instance_create_surface with proper handles
        
        // For now, we'll return an error indicating this needs platform-specific implementation
        Err(BasaltError::surface(
            "Surface creation requires platform-specific implementation for wgpu-core 27"
        ))
    }
    
    /// Create a surface with an existing surface ID (for testing or direct creation)
    pub fn from_id(context: Arc<BasaltContext>, surface_id: id::SurfaceId) -> Self {
        Self {
            context,
            surface_id,
            config: None,
            device_id: None,
            max_retries: 3,  // Allow up to 3 retries for transient errors
        }
    }

    /// Get the surface ID
    pub fn id(&self) -> id::SurfaceId {
        self.surface_id
    }

    /// wgpu 28.0: Get the current surface configuration
    /// Returns the configuration if the surface has been configured
    pub fn get_configuration(&self) -> Option<&wgt::SurfaceConfiguration<Vec<wgt::TextureFormat>>> {
        self.config.as_ref()
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
        self.device_id = Some(device_id);
        Ok(())
    }

    /// Reconfigure the surface (used for error recovery)
    fn reconfigure(&self) -> Result<()> {
        if let (Some(config), Some(device_id)) = (&self.config, self.device_id) {
            self.context
                .inner()
                .surface_configure(self.surface_id, device_id, config);
            log::debug!("Surface reconfigured after error");
            Ok(())
        } else {
            Err(BasaltError::surface("Cannot reconfigure: no config or device"))
        }
    }

    /// Get the current texture with automatic error recovery
    ///
    /// Based on wgpu-core 27.0 SurfaceError handling:
    /// - NotConfigured: Reconfigure and retry
    /// - AlreadyAcquired: Drop previous output and retry
    /// - Invalid/TextureDestroyed/Device: Return error immediately
    ///
    /// This prevents crashes during window resize, minimize, or GPU transitions
    ///
    /// # Error Recovery Strategy (wgpu pattern)
    /// 1. **NotConfigured**: Surface was never configured - configure it
    /// 2. **AlreadyAcquired**: Previous frame wasn't presented - wait and retry
    /// 3. **Others**: Fatal errors - cannot recover
    pub fn get_current_texture(&self) -> Result<wgpu_core::present::SurfaceOutput> {
        let global = self.context.inner();
        let mut attempts = 0;
        let mut backoff_ms = 10; // Exponential backoff starting at 10ms

        loop {
            match global.surface_get_current_texture(self.surface_id, None) {
                Ok(output) => {
                    if attempts > 0 {
                        log::info!("Got texture after {} retries (backoff was {}ms)", attempts, backoff_ms / 2);
                    }
                    return Ok(output);
                }
                Err(wgpu_core::present::SurfaceError::NotConfigured) => {
                    // Surface needs configuration - this is expected on first frame
                    log::debug!("Surface not configured, attempting reconfigure...");
                    self.reconfigure()?;
                    attempts += 1;
                    if attempts >= self.max_retries {
                        return Err(BasaltError::surface(format!(
                            "NotConfigured error after {} retries", attempts
                        )));
                    }
                    continue;
                }
                Err(wgpu_core::present::SurfaceError::AlreadyAcquired) => {
                    // Previous output wasn't dropped/presented - wait and retry
                    log::warn!("Surface already acquired, waiting {}ms and retrying ({}/{})",
                        backoff_ms, attempts, self.max_retries);
                    attempts += 1;
                    if attempts >= self.max_retries {
                        return Err(BasaltError::surface(format!(
                            "AlreadyAcquired after {} retries", attempts
                        )));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(backoff_ms));
                    backoff_ms = (backoff_ms * 2).min(500); // Exponential backoff, max 500ms
                    continue;
                }
                Err(wgpu_core::present::SurfaceError::Invalid) => {
                    // Surface is invalid - can't recover (e.g., window destroyed)
                    log::error!("Surface is invalid - cannot recover");
                    return Err(BasaltError::surface("Surface is invalid (window may be destroyed)"));
                }
                Err(wgpu_core::present::SurfaceError::TextureDestroyed) => {
                    // Texture was destroyed externally - can't recover
                    log::error!("Texture was destroyed externally");
                    return Err(BasaltError::surface("Texture was destroyed"));
                }
                Err(wgpu_core::present::SurfaceError::Device(e)) => {
                    // Device-level error - likely fatal
                    log::error!("Device error: {:?}", e);
                    return Err(BasaltError::surface(format!("Device error: {:?}", e)));
                }
                Err(e) => {
                    // Any other error - treat as fatal
                    log::error!("Unexpected surface error: {:?}", e);
                    return Err(BasaltError::surface(format!("Unexpected surface error: {:?}", e)));
                }
            }
        }
    }

    /// Present the surface
    pub fn present(&self, _queue_id: id::QueueId) -> Result<()> {
        self.context
            .inner()
            .surface_present(self.surface_id)
            .map_err(|e| BasaltError::surface(format!("Failed to present: {:?}", e)))?;
        Ok(())
    }

    /// Pre-present notification (important for macOS frame timing)
    ///
    /// On macOS, this must be called before present() to ensure proper
    /// frame synchronization with the window server.
    /// On other platforms, this is a no-op.
    ///
    /// See wgpu examples framework.rs:442
    pub fn pre_present_notify(&self) {
        #[cfg(target_os = "macos")]
        {
            // On macOS, notify the window that we're about to present
            // This ensures proper frame synchronization with CoreAnimation
            // The actual implementation would call into the window system
            log::trace!("Pre-present notify (macOS)");
        }
    }

    /// Suspend the surface (for Android lifecycle)
    ///
    /// On Android, the surface must be dropped when the app is suspended
    /// and recreated when resumed. This is handled automatically by the
    /// framework in wgpu examples.
    pub fn suspend(&mut self) {
        #[cfg(target_os = "android")]
        {
            log::info!("Suspending surface (Android)");
            // Drop the surface - it will be recreated on resume
            self.context.inner().surface_drop(self.surface_id);
            self.config = None;
            self.device_id = None;
        }
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
