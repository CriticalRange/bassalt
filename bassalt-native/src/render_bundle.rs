//! RenderBundle support for wgpu-core 27
//!
//! RenderBundles are pre-recorded sequences of rendering commands that can be
//! replayed efficiently. This is useful for repeated draw calls like particles,
//! entities, or UI elements that don't change between frames.
//!
//! Based on wgpu example: examples/features/src/msaa_line/mod.rs

use std::borrow::Cow;
use std::sync::Arc;
use wgpu_core::command;
use wgpu_core::id;
use wgpu_types as wgt;

use crate::context::BasaltContext;
use crate::error::{BasaltError, Result};

// Import RenderBundleEncoderDescriptor from wgpu_core::command
use wgpu_core::command::RenderBundleEncoderDescriptor;

/// A RenderBundle contains pre-recorded rendering commands
///
/// RenderBundles are useful for:
/// - Repeated draw calls with the same resources
/// - Particle systems
/// - UI elements
/// - Entity batches
///
/// The bundle is created once and can be executed multiple times with minimal overhead.
pub struct BasaltRenderBundle {
    pub id: id::RenderBundleId,
}

impl BasaltRenderBundle {
    /// Create a new render bundle encoder
    ///
    /// The encoder records commands that will be bundled together.
    /// Once finished, the bundle can be executed multiple times.
    pub fn create_encoder(
        context: &Arc<BasaltContext>,
        device_id: id::DeviceId,
        descriptor: &RenderBundleEncoderDescriptor,
    ) -> Result<command::RenderBundleEncoder> {
        let global = context.inner();

        // wgpu 27.0 uses RenderBundleEncoderDescriptor directly
        let (encoder, error) = global
            .device_create_render_bundle_encoder(device_id, &descriptor);

        if let Some(e) = error {
            return Err(BasaltError::resource_creation("render bundle encoder", format!("{:?}", e)));
        }

        Ok(unsafe { *Box::from_raw(encoder) })
    }

    /// Create a render bundle from an encoder
    ///
    /// The encoder must have already recorded all desired commands.
    pub fn finish(
        context: &Arc<BasaltContext>,
        encoder: command::RenderBundleEncoder,
        descriptor: &wgt::RenderBundleDescriptor<Option<Cow<'_, str>>>,
    ) -> Result<id::RenderBundleId> {
        let global = context.inner();

        let (bundle_id, error) = global
            .render_bundle_encoder_finish(encoder, descriptor, None);

        if let Some(e) = error {
            return Err(BasaltError::resource_creation("render bundle", format!("{:?}", e)));
        }

        log::debug!("Created render bundle {:?}", bundle_id);
        Ok(bundle_id)
    }
}

/// Builder for creating a RenderBundle
///
/// # Example
/// ```no_run
/// use bassalt_native::render_bundle::RenderBundleBuilder;
///
/// let bundle = RenderBundleBuilder::new()
///     .label("MyBundle")
///     .color_formats(&[wgt::TextureFormat::Bgra8Unorm])
///     .depth_stencil(None)
///     .sample_count(1)
///     .build(&context, device_id)?;
/// ```
#[derive(Clone, Debug)]
pub struct RenderBundleBuilder {
    label: Option<String>,
    color_formats: Vec<Option<wgt::TextureFormat>>,
    depth_stencil: Option<wgt::RenderBundleDepthStencil>,
    sample_count: u32,
    multiview: Option<std::num::NonZero<u32>>,
}

impl Default for RenderBundleBuilder {
    fn default() -> Self {
        Self {
            label: None,
            color_formats: vec![None],
            depth_stencil: None,
            sample_count: 1,
            multiview: None,
        }
    }
}

impl RenderBundleBuilder {
    /// Create a new RenderBundleBuilder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the label for debugging
    pub fn label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }

    /// Set the color attachment formats
    ///
    /// Each attachment corresponds to a color target in the render pass.
    pub fn color_formats(mut self, formats: &[wgt::TextureFormat]) -> Self {
        self.color_formats = formats.iter().map(|&f| Some(f)).collect();
        self
    }

    /// Set the depth-stencil attachment format
    pub fn depth_stencil(mut self, depth_stencil: wgt::RenderBundleDepthStencil) -> Self {
        self.depth_stencil = Some(depth_stencil);
        self
    }

    /// Set the sample count for MSAA
    ///
    /// Must match the sample count of the render pass where this bundle will be used.
    /// - 1 = No MSAA
    /// - 4 = 4x MSAA
    /// - 8 = 8x MSAA
    pub fn sample_count(mut self, count: u32) -> Self {
        self.sample_count = count;
        self
    }

    /// Set the multiview count for VR
    pub fn multiview(mut self, count: std::num::NonZero<u32>) -> Self {
        self.multiview = Some(count);
        self
    }

    /// Build the RenderBundle encoder
    ///
    /// Returns an encoder that can record commands.
    /// Call `finish()` on the encoder to create the actual bundle.
    pub fn build_encoder(
        self,
        context: &Arc<BasaltContext>,
        device_id: id::DeviceId,
    ) -> Result<command::RenderBundleEncoder> {
        let descriptor = RenderBundleEncoderDescriptor {
            label: self.label.map(Cow::Owned),
            color_formats: Cow::from(self.color_formats),
            depth_stencil: self.depth_stencil,
            sample_count: self.sample_count,
            multiview: self.multiview,
        };

        BasaltRenderBundle::create_encoder(context, device_id, &descriptor)
    }
}

/// Convenience function to create a simple render bundle
///
/// This is the easiest way to create a bundle for common cases.
///
/// # Arguments
/// - `context` - The wgpu context
/// - `device_id` - The device to create the bundle on
/// - `color_format` - The color attachment format
/// - `sample_count` - MSAA sample count (1 = no MSAA)
///
/// # Example
/// ```no_run
/// # use bassalt_native::render_bundle;
/// # let context = unimplemented!();
/// # let device_id = unimplemented!();
/// let encoder = render_bundle::create_simple_encoder(
///     &context,
///     device_id,
///     wgt::TextureFormat::Bgra8Unorm,
///     1,
/// )?;
/// ```
pub fn create_simple_encoder(
    context: &Arc<BasaltContext>,
    device_id: id::DeviceId,
    color_format: wgt::TextureFormat,
    sample_count: u32,
) -> Result<command::RenderBundleEncoder> {
    RenderBundleBuilder::new()
        .color_formats(&[color_format])
        .sample_count(sample_count)
        .build_encoder(context, device_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_bundle_builder() {
        let builder = RenderBundleBuilder::new()
            .label("TestBundle")
            .color_formats(&[wgt::TextureFormat::Bgra8Unorm])
            .sample_count(4);

        assert_eq!(builder.label, Some("TestBundle".to_string()));
        assert_eq!(builder.sample_count, 4);
    }
}
