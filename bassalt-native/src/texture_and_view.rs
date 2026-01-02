//! Combined texture and view management
//!
//! This module provides a combined `TextureAndView` struct that keeps texture
//! and view together, following the pattern from wgpu-mc.
//!
//! Benefits:
//! - Easier resource management (view lifetime tied to texture)
//! - Reduced chance of using wrong view with texture
//! - Simplified cleanup (drop both together)
//! - Better cache locality

use std::sync::Arc;
use wgpu_core::id;
use wgpu_types as wgt;
use crate::context::BasaltContext;

/// Combined texture and view
///
/// Keeps texture ID and view ID together for easier management.
/// The view is expected to have the same lifetime as the texture.
#[derive(Debug, Clone)]
pub struct TextureAndView {
    /// The texture ID
    pub texture: id::TextureId,
    /// The texture view ID
    pub view: id::TextureViewId,
    /// Texture format
    pub format: wgt::TextureFormat,
    /// Texture dimension (D2, D2Array, Cube, etc.)
    pub dimension: wgt::TextureViewDimension,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
    /// Depth or layer count
    pub depth_or_layers: u32,
    /// Mip levels
    pub mip_levels: u32,
    /// Label for debugging
    pub label: String,
}

impl TextureAndView {
    /// Create a new TextureAndView from separate IDs
    pub fn new(
        texture: id::TextureId,
        view: id::TextureViewId,
        format: wgt::TextureFormat,
        dimension: wgt::TextureViewDimension,
        width: u32,
        height: u32,
        depth_or_layers: u32,
        mip_levels: u32,
        label: String,
    ) -> Self {
        Self {
            texture,
            view,
            format,
            dimension,
            width,
            height,
            depth_or_layers,
            mip_levels,
            label,
        }
    }

    /// Create a texture and view from a texture descriptor
    pub fn create(
        context: &Arc<BasaltContext>,
        device_id: id::DeviceId,
        descriptor: &wgt::TextureDescriptor<Option<std::borrow::Cow<'_, str>>, Vec<wgt::TextureFormat>>,
    ) -> Result<Self, crate::error::BasaltError> {
        let global = context.inner();

        // Create the texture
        let (texture_id, error) = global.device_create_texture(
            device_id,
            descriptor,
            None,
        );

        if let Some(e) = error {
            let label = descriptor.label.as_deref().unwrap_or("unnamed");
            return Err(crate::error::BasaltError::ResourceCreation {
                resource_type: "texture".to_string(),
                reason: format!("Failed to create texture '{}': {:?}", label, e),
            });
        }

        // Create the view
        let label = descriptor.label.as_deref().unwrap_or("unnamed");
        let view_descriptor = wgpu_core::resource::TextureViewDescriptor {
            label: Some(std::borrow::Cow::Borrowed(label)),
            format: Some(descriptor.format),
            dimension: None, // Use texture's dimension
            range: wgt::ImageSubresourceRange {
                aspect: wgt::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None, // All mip levels
                base_array_layer: 0,
                array_layer_count: None, // All layers
            },
            usage: None, // Not used in wgpu-core 27.0
        };

        let (view_id, view_error) = global.texture_create_view(
            texture_id,
            &view_descriptor,
            None,
        );

        if let Some(e) = view_error {
            // Clean up texture on failure
            let _ = global.texture_destroy(texture_id);
            return Err(crate::error::BasaltError::ResourceCreation {
                resource_type: "texture view".to_string(),
                reason: format!("Failed to create texture view for '{}': {:?}", label, e),
            });
        }

        // Determine view dimension
        let dimension = match descriptor.dimension {
            wgt::TextureDimension::D1 => wgt::TextureViewDimension::D1,
            wgt::TextureDimension::D2 => {
                if descriptor.size.depth_or_array_layers > 1 {
                    if descriptor.size.width == descriptor.size.height && descriptor.size.depth_or_array_layers == 6 {
                        wgt::TextureViewDimension::Cube
                    } else {
                        wgt::TextureViewDimension::D2Array
                    }
                } else {
                    wgt::TextureViewDimension::D2
                }
            }
            wgt::TextureDimension::D3 => wgt::TextureViewDimension::D3,
        };

        Ok(Self {
            texture: texture_id,
            view: view_id,
            format: descriptor.format,
            dimension,
            width: descriptor.size.width,
            height: descriptor.size.height,
            depth_or_layers: descriptor.size.depth_or_array_layers,
            mip_levels: descriptor.mip_level_count,
            label: label.to_string(),
        })
    }

    /// Create a texture and view for a render target (color attachment)
    pub fn create_color_attachment(
        context: &Arc<BasaltContext>,
        device_id: id::DeviceId,
        width: u32,
        height: u32,
        format: wgt::TextureFormat,
        label: &str,
    ) -> Result<Self, crate::error::BasaltError> {
        let descriptor = wgt::TextureDescriptor {
            label: Some(std::borrow::Cow::Borrowed(label)),
            size: wgt::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgt::TextureDimension::D2,
            format,
            usage: wgt::TextureUsages::RENDER_ATTACHMENT
                | wgt::TextureUsages::TEXTURE_BINDING
                | wgt::TextureUsages::COPY_SRC,
            view_formats: vec![],
        };

        Self::create(context, device_id, &descriptor)
    }

    /// Create a texture and view for a depth attachment
    pub fn create_depth_attachment(
        context: &Arc<BasaltContext>,
        device_id: id::DeviceId,
        width: u32,
        height: u32,
        format: wgt::TextureFormat,
        label: &str,
    ) -> Result<Self, crate::error::BasaltError> {
        let descriptor = wgt::TextureDescriptor {
            label: Some(std::borrow::Cow::Borrowed(label)),
            size: wgt::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgt::TextureDimension::D2,
            format,
            usage: wgt::TextureUsages::RENDER_ATTACHMENT | wgt::TextureUsages::TEXTURE_BINDING,
            view_formats: vec![],
        };

        Self::create(context, device_id, &descriptor)
    }

    /// Get the texture ID
    pub fn texture_id(&self) -> id::TextureId {
        self.texture
    }

    /// Get the view ID
    pub fn view_id(&self) -> id::TextureViewId {
        self.view
    }

    /// Get the format
    pub fn format(&self) -> wgt::TextureFormat {
        self.format
    }

    /// Get the dimension
    pub fn dimension(&self) -> wgt::TextureViewDimension {
        self.dimension
    }

    /// Check if this is a depth texture
    pub fn is_depth(&self) -> bool {
        matches!(
            self.format,
            wgt::TextureFormat::Depth24Plus
                | wgt::TextureFormat::Depth24PlusStencil8
                | wgt::TextureFormat::Depth32Float
        )
    }

    /// Check if this is a color texture
    pub fn is_color(&self) -> bool {
        !self.is_depth()
    }

    /// Get width
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get height
    pub fn height(&self) -> u32 {
        self.height
    }
}

/// TextureAndView registry for caching and reuse
///
/// Provides a cache of textures keyed by their properties.
pub struct TextureRegistry {
    textures: parking_lot::Mutex<std::collections::HashMap<
        TextureKey,
        Arc<TextureAndView>,
    >>,
}

impl TextureRegistry {
    /// Create a new texture registry
    pub fn new() -> Self {
        Self {
            textures: parking_lot::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Get or create a texture with the given properties
    pub fn get_or_create(
        &self,
        context: &Arc<BasaltContext>,
        device_id: id::DeviceId,
        width: u32,
        height: u32,
        format: wgt::TextureFormat,
        usage: wgt::TextureUsages,
        label: &str,
    ) -> Result<Arc<TextureAndView>, crate::error::BasaltError> {
        let key = TextureKey {
            width,
            height,
            format,
            usage,
        };

        // Check cache
        {
            let textures = self.textures.lock();
            if let Some(texture) = textures.get(&key) {
                return Ok(texture.clone());
            }
        }

        // Create new texture
        let descriptor = wgt::TextureDescriptor {
            label: Some(std::borrow::Cow::Borrowed(label)),
            size: wgt::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgt::TextureDimension::D2,
            format,
            usage,
            view_formats: vec![],
        };

        let texture_and_view = TextureAndView::create(context, device_id, &descriptor)?;

        // Cache it
        {
            let mut textures = self.textures.lock();
            textures.insert(key, Arc::new(texture_and_view.clone()));
        }

        Ok(Arc::new(texture_and_view))
    }

    /// Clear the cache
    ///
    /// Note: This does not destroy the textures, just removes them from the cache.
    pub fn clear(&self) {
        self.textures.lock().clear();
    }
}

impl Default for TextureRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Key for texture caching
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TextureKey {
    width: u32,
    height: u32,
    format: wgt::TextureFormat,
    usage: wgt::TextureUsages,
}
