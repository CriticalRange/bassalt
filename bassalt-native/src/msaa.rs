//! MSAA (Multisample Anti-Aliasing) support for wgpu-core 27
//!
//! MSAA improves visual quality by sampling each pixel multiple times
//! and averaging the results. This reduces jagged edges (aliasing).
//!
//! Based on wgpu example: examples/features/src/msaa_line/mod.rs
//!
//! # MSAA Overview
//!
//! MSAA works by:
//! 1. Creating a framebuffer texture with `sample_count > 1`
//! 2. Rendering to the MSAA framebuffer instead of the swapchain
//! 3. Using a `resolve_target` to copy resolved samples to the swapchain
//!
//! # Sample Counts
//!
//! - 1 = No MSAA (default)
//! - 2 = 2x MSAA
//! - 4 = 4x MSAA (common, good quality/performance balance)
//! - 8 = 8x MSAA (high quality, more expensive)
//! - 16 = 16x MSAA (maximum quality, very expensive)
//!
//! # Usage
//!
//! ```rust,no_run
//! use bassalt_native::msaa::MSAAConfig;
//!
//! // Query the maximum supported sample count
//! let max_samples = MSAAConfig::get_max_supported_samples(&context, adapter_id, format)?;
//!
//! // Create MSAA resources
//! let msaa = MSAAConfig::new(&context, device_id, width, height, format, 4)?;
//!
//! // During render pass, use the MSAA texture as color attachment
//! // and the swapchain view as resolve target
//! ```

use std::borrow::Cow;
use std::sync::Arc;
use wgpu_core::command;
use wgpu_core::id;
use wgpu_types as wgt;

use crate::context::BasaltContext;
use crate::error::{BasaltError, Result};

/// MSAA configuration and resources
///
/// Contains the multisampled framebuffer and sample count.
#[derive(Debug)]
pub struct MSAAConfig {
    /// The multisampled framebuffer texture view
    pub framebuffer_view_id: id::TextureViewId,
    /// The underlying texture (for recreation on resize)
    pub framebuffer_texture_id: id::TextureId,
    /// Sample count (1 = no MSAA, 4 = 4x MSAA, etc.)
    pub sample_count: u32,
    /// Texture format
    pub format: wgt::TextureFormat,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
}

impl MSAAConfig {
    /// Get the maximum supported sample count for a texture format
    ///
    /// Queries the adapter for the maximum MSAA sample count supported
    /// for the given format.
    ///
    /// # Arguments
    /// - `context` - The wgpu context
    /// - `adapter_id` - The adapter to query
    /// - `format` - The texture format to check
    ///
    /// # Returns
    /// The maximum supported sample count (1, 2, 4, 8, or 16)
    ///
    /// # Example
    /// ```no_run
    /// # use bassalt_native::msaa::MSAAConfig;
    /// # let context = unimplemented!();
    /// # let adapter_id = unimplemented!();
    /// let max_samples = MSAAConfig::get_max_supported_samples(
    ///     &context,
    ///     adapter_id,
    ///     wgt::TextureFormat::Bgra8Unorm,
    /// )?;
    /// println!("Max MSAA samples: {}", max_samples);
    /// ```
    pub fn get_max_supported_samples(
        context: &Arc<BasaltContext>,
        adapter_id: id::AdapterId,
        format: wgt::TextureFormat,
    ) -> Result<u32> {
        let global = context.inner();

        // Get the texture format features
        let format_features = global.adapter_get_texture_format_features(adapter_id, format);

        // Check the sample count flags in order from highest to lowest
        let flags = format_features.flags;

        let max_count = if flags.contains(wgt::TextureFormatFeatureFlags::MULTISAMPLE_X16) {
            16
        } else if flags.contains(wgt::TextureFormatFeatureFlags::MULTISAMPLE_X8) {
            8
        } else if flags.contains(wgt::TextureFormatFeatureFlags::MULTISAMPLE_X4) {
            4
        } else if flags.contains(wgt::TextureFormatFeatureFlags::MULTISAMPLE_X2) {
            2
        } else {
            1
        };

        log::info!("Max supported MSAA samples for {:?}: {}", format, max_count);
        Ok(max_count)
    }

    /// Create a new MSAA configuration
    ///
    /// Creates a multisampled framebuffer texture for MSAA rendering.
    ///
    /// # Arguments
    /// - `context` - The wgpu context
    /// - `device_id` - The device to create resources on
    /// - `width` - Width in pixels
    /// - `height` - Height in pixels
    /// - `format` - The color attachment format
    /// - `sample_count` - Desired sample count (will be clamped to max supported)
    ///
    /// # Returns
    /// The MSAA configuration with framebuffer texture
    ///
    /// # Example
    /// ```no_run
    /// # use bassalt_native::msaa::MSAAConfig;
    /// # let context = unimplemented!();
    /// # let device_id = unimplemented!();
    /// let msaa = MSAAConfig::new(
    ///     &context,
    ///     device_id,
    ///     1920,
    ///     1080,
    ///     wgt::TextureFormat::Bgra8Unorm,
    ///     4,
    /// )?;
    /// ```
    pub fn new(
        context: &Arc<BasaltContext>,
        device_id: id::DeviceId,
        width: u32,
        height: u32,
        format: wgt::TextureFormat,
        sample_count: u32,
    ) -> Result<Self> {
        // Clamp sample count to valid range
        let sample_count = sample_count.clamp(1, 16);

        // Create the multisampled framebuffer texture
        let texture_desc = wgt::TextureDescriptor {
            label: Some(Cow::Borrowed("MSAA Framebuffer")),
            size: wgt::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count,
            dimension: wgt::TextureDimension::D2,
            format,
            usage: wgt::TextureUsages::RENDER_ATTACHMENT,
            view_formats: vec![],
        };

        let global = context.inner();

        let (texture_id, error) = global.device_create_texture(device_id, &texture_desc, None);

        if let Some(e) = error {
            return Err(BasaltError::resource_creation(
                "MSAA framebuffer texture",
                format!("{:?}", e),
            ));
        }

        // Create a texture view
        let view_desc = wgpu_core::resource::TextureViewDescriptor::default();
        let (view_id, error) = global.texture_create_view(texture_id, &view_desc, None);

        if let Some(e) = error {
            return Err(BasaltError::resource_creation(
                "MSAA framebuffer view",
                format!("{:?}", e),
            ));
        }

        log::info!(
            "Created MSAA framebuffer: {}x{}, format={:?}, samples={}",
            width,
            height,
            format,
            sample_count
        );

        Ok(Self {
            framebuffer_view_id: view_id,
            framebuffer_texture_id: texture_id,
            sample_count,
            format,
            width,
            height,
        })
    }

    /// Recreate the MSAA framebuffer with new dimensions
    ///
    /// Call this when the window/surface is resized.
    ///
    /// # Arguments
    /// - `context` - The wgpu context
    /// - `device_id` - The device to create resources on
    /// - `width` - New width in pixels
    /// - `height` - New height in pixels
    pub fn resize(
        &mut self,
        context: &Arc<BasaltContext>,
        device_id: id::DeviceId,
        width: u32,
        height: u32,
    ) -> Result<()> {
        // Create new framebuffer with new dimensions
        let new_msaa = Self::new(
            context,
            device_id,
            width,
            height,
            self.format,
            self.sample_count,
        )?;

        // Replace our resources
        self.framebuffer_view_id = new_msaa.framebuffer_view_id;
        self.framebuffer_texture_id = new_msaa.framebuffer_texture_id;
        self.width = width;
        self.height = height;

        log::info!("Resized MSAA framebuffer to {}x{}", width, height);
        Ok(())
    }

    /// Get the multisample state for pipeline creation
    ///
    /// Returns a `MultisampleState` configured for this sample count.
    /// Use this when creating render pipelines.
    pub fn multisample_state(&self) -> wgt::MultisampleState {
        wgt::MultisampleState {
            count: self.sample_count,
            mask: !0, // Enable all samples
            alpha_to_coverage_enabled: false,
        }
    }

    /// Check if MSAA is enabled (sample_count > 1)
    pub fn is_enabled(&self) -> bool {
        self.sample_count > 1
    }

    /// Get the resolve target for render pass color attachment
    ///
    /// When using MSAA, the color attachment should be:
    /// - view = MSAA framebuffer (from `framebuffer_view_id`)
    /// - resolve_target = swapchain view
    ///
    /// When not using MSAA (sample_count == 1), render directly to swapchain.
    pub fn color_attachment_needs_resolve(&self) -> bool {
        self.sample_count > 1
    }
}

/// Create a render pass color attachment with MSAA resolve
///
/// This helper creates the appropriate `RenderPassColorAttachment`
/// based on whether MSAA is enabled.
///
/// # Arguments
/// - `msaa_config` - The MSAA configuration (optional)
/// - `swapchain_view` - The swapchain texture view
/// - `clear_color` - The clear color (for load op)
///
/// # Returns
/// A configured `RenderPassColorAttachment`
///
/// # Example
/// ```no_run
/// # use bassalt_native::msaa;
/// # use wgpu_types as wgt;
/// # let msaa_config = None;
/// # let swapchain_view = unimplemented!();
/// let color_attachment = msaa::create_color_attachment(
///     msaa_config.as_ref(),
///     &swapchain_view,
///     wgt::Color::BLACK,
/// );
/// ```
pub fn create_color_attachment(
    msaa_config: Option<&MSAAConfig>,
    swapchain_view: &id::TextureViewId,
    clear_color: wgt::Color,
) -> command::RenderPassColorAttachment {
    if let Some(msaa) = msaa_config {
        if msaa.is_enabled() {
            // MSAA enabled: render to MSAA framebuffer, resolve to swapchain
            command::RenderPassColorAttachment {
                view: msaa.framebuffer_view_id,
                depth_slice: None,
                resolve_target: Some(*swapchain_view),
                load_op: wgt::LoadOp::Clear(clear_color),
                store_op: wgt::StoreOp::Discard, // Discard MSAA data after resolve (saves memory on tile GPUs)
            }
        } else {
            // MSAA disabled: render directly to swapchain
            command::RenderPassColorAttachment {
                view: *swapchain_view,
                depth_slice: None,
                resolve_target: None,
                load_op: wgt::LoadOp::Clear(clear_color),
                store_op: wgt::StoreOp::Store,
            }
        }
    } else {
        // No MSAA config: render directly to swapchain
        command::RenderPassColorAttachment {
            view: *swapchain_view,
            depth_slice: None,
            resolve_target: None,
            load_op: wgt::LoadOp::Clear(clear_color),
            store_op: wgt::StoreOp::Store,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msaa_config() {
        // Test that MSAAConfig fields are correctly set
        let config = MSAAConfig {
            framebuffer_view_id: id::TextureViewId::ERROR,
            framebuffer_texture_id: id::TextureId::ERROR,
            sample_count: 4,
            format: wgt::TextureFormat::Bgra8Unorm,
            width: 1920,
            height: 1080,
        };

        assert_eq!(config.sample_count, 4);
        assert!(config.is_enabled());
        assert!(config.color_attachment_needs_resolve());
    }

    #[test]
    fn test_no_msaa() {
        let config = MSAAConfig {
            framebuffer_view_id: id::TextureViewId::ERROR,
            framebuffer_texture_id: id::TextureId::ERROR,
            sample_count: 1,
            format: wgt::TextureFormat::Bgra8Unorm,
            width: 1920,
            height: 1080,
        };

        assert_eq!(config.sample_count, 1);
        assert!(!config.is_enabled());
        assert!(!config.color_attachment_needs_resolve());
    }
}
