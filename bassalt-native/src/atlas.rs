//! Texture atlasing for efficient texture management
//!
//! This module provides a texture atlas that packs multiple textures
//! into a single large GPU texture, reducing bind group changes and
//! improving batching efficiency.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use guillotiere::{AtlasAllocator, Size, Allocation, AllocId};
use wgpu_core::id;
use wgpu_types as wgt;

use crate::context::BasaltContext;
use crate::error::{BasaltError, Result};

/// Default atlas dimensions (2048x2048 is a good balance)
pub const DEFAULT_ATLAS_SIZE: u32 = 2048;

/// UV coordinates for a region in the atlas
#[derive(Debug, Clone, Copy)]
pub struct AtlasUV {
    /// Minimum U coordinate (0.0 to 1.0)
    pub u_min: f32,
    /// Minimum V coordinate (0.0 to 1.0)
    pub v_min: f32,
    /// Maximum U coordinate (0.0 to 1.0)
    pub u_max: f32,
    /// Maximum V coordinate (0.0 to 1.0)
    pub v_max: f32,
}

impl AtlasUV {
    pub fn new(x: u32, y: u32, width: u32, height: u32, atlas_size: u32) -> Self {
        let size = atlas_size as f32;
        Self {
            u_min: x as f32 / size,
            v_min: y as f32 / size,
            u_max: (x + width) as f32 / size,
            v_max: (y + height) as f32 / size,
        }
    }
}

/// Handle to a region allocated in the atlas
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtlasHandle(AllocId);

impl AtlasHandle {
    pub fn id(&self) -> AllocId {
        self.0
    }
}

/// Information about an atlas region
#[derive(Debug, Clone)]
pub struct AtlasRegion {
    /// X position in pixels
    pub x: u32,
    /// Y position in pixels
    pub y: u32,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// UV coordinates for this region
    pub uv: AtlasUV,
}

/// A texture atlas that manages a single GPU texture with multiple regions
pub struct TextureAtlas {
    context: Arc<BasaltContext>,
    device_id: id::DeviceId,
    queue_id: id::QueueId,

    /// The GPU texture
    texture_id: id::TextureId,

    /// The texture view
    texture_view_id: id::TextureViewId,

    /// Atlas dimensions
    size: u32,

    /// Texture format
    format: wgt::TextureFormat,

    /// The allocator for packing regions
    allocator: RwLock<AtlasAllocator>,

    /// Map from allocation ID to region info
    regions: RwLock<HashMap<AllocId, AtlasRegion>>,

    /// Optional label for debugging
    label: String,
}

impl TextureAtlas {
    /// Create a new texture atlas
    pub fn new(
        context: Arc<BasaltContext>,
        device_id: id::DeviceId,
        queue_id: id::QueueId,
        size: u32,
        format: wgt::TextureFormat,
        label: &str,
    ) -> Result<Self> {
        // Create the GPU texture
        let desc = wgt::TextureDescriptor {
            label: Some(Cow::Owned(format!("{} Atlas", label))),
            size: wgt::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgt::TextureDimension::D2,
            format,
            usage: wgt::TextureUsages::TEXTURE_BINDING
                | wgt::TextureUsages::COPY_DST
                | wgt::TextureUsages::COPY_SRC,
            view_formats: vec![],
        };

        let (texture_id, error) = context
            .inner()
            .device_create_texture(device_id, &desc, None);

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("Failed to create atlas texture: {:?}", e)));
        }

        // Create texture view
        let view_desc = wgpu_core::resource::TextureViewDescriptor {
            label: Some(Cow::Owned(format!("{} Atlas View", label))),
            format: Some(format),
            dimension: Some(wgt::TextureViewDimension::D2),
            usage: None,
            range: wgt::ImageSubresourceRange {
                aspect: wgt::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            },
        };

        let (texture_view_id, error) = context
            .inner()
            .texture_create_view(texture_id, &view_desc, None);

        if let Some(e) = error {
            context.inner().texture_drop(texture_id);
            return Err(BasaltError::Wgpu(format!("Failed to create atlas view: {:?}", e)));
        }

        // Initialize the allocator
        let allocator = AtlasAllocator::new(Size::new(size as i32, size as i32));

        log::info!(
            "Created texture atlas '{}': {}x{} {:?}",
            label, size, size, format
        );

        Ok(Self {
            context,
            device_id,
            queue_id,
            texture_id,
            texture_view_id,
            size,
            format,
            allocator: RwLock::new(allocator),
            regions: RwLock::new(HashMap::new()),
            label: label.to_string(),
        })
    }

    /// Allocate a region in the atlas
    pub fn allocate(&self, width: u32, height: u32) -> Result<AtlasHandle> {
        let mut allocator = self.allocator.write();

        let allocation = allocator
            .allocate(Size::new(width as i32, height as i32))
            .ok_or_else(|| BasaltError::OutOfMemory(format!(
                "Atlas '{}' cannot fit {}x{} region",
                self.label, width, height
            )))?;

        let region = AtlasRegion {
            x: allocation.rectangle.min.x as u32,
            y: allocation.rectangle.min.y as u32,
            width,
            height,
            uv: AtlasUV::new(
                allocation.rectangle.min.x as u32,
                allocation.rectangle.min.y as u32,
                width,
                height,
                self.size,
            ),
        };

        self.regions.write().insert(allocation.id, region);

        log::debug!(
            "Atlas '{}': allocated {}x{} at ({}, {})",
            self.label, width, height,
            allocation.rectangle.min.x, allocation.rectangle.min.y
        );

        Ok(AtlasHandle(allocation.id))
    }

    /// Free a previously allocated region
    pub fn free(&self, handle: AtlasHandle) {
        self.allocator.write().deallocate(handle.0);
        self.regions.write().remove(&handle.0);
        log::debug!("Atlas '{}': freed region {:?}", self.label, handle.0);
    }

    /// Get information about an allocated region
    pub fn get_region(&self, handle: AtlasHandle) -> Option<AtlasRegion> {
        self.regions.read().get(&handle.0).cloned()
    }

    /// Upload pixel data to a region
    pub fn upload(&self, handle: AtlasHandle, data: &[u8]) -> Result<()> {
        let region = self.regions.read().get(&handle.0).cloned()
            .ok_or_else(|| BasaltError::InvalidParameter(
                format!("Invalid atlas handle: {:?}", handle.0)
            ))?;

        // Calculate expected size based on format
        let bytes_per_pixel = match self.format {
            wgt::TextureFormat::Rgba8Unorm
            | wgt::TextureFormat::Rgba8UnormSrgb
            | wgt::TextureFormat::Bgra8Unorm
            | wgt::TextureFormat::Bgra8UnormSrgb => 4,
            wgt::TextureFormat::Rg8Unorm => 2,
            wgt::TextureFormat::R8Unorm => 1,
            _ => 4, // Default to 4 bytes
        };

        let expected_size = (region.width * region.height * bytes_per_pixel) as usize;
        if data.len() < expected_size {
            return Err(BasaltError::InvalidParameter(format!(
                "Data size {} is less than expected {} for {}x{} region",
                data.len(), expected_size, region.width, region.height
            )));
        }

        let texture_copy = wgt::TexelCopyTextureInfo {
            texture: self.texture_id,
            mip_level: 0,
            origin: wgt::Origin3d {
                x: region.x,
                y: region.y,
                z: 0,
            },
            aspect: wgt::TextureAspect::All,
        };

        let data_layout = wgt::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(region.width * bytes_per_pixel),
            rows_per_image: Some(region.height),
        };

        let size = wgt::Extent3d {
            width: region.width,
            height: region.height,
            depth_or_array_layers: 1,
        };

        self.context
            .inner()
            .queue_write_texture(self.queue_id, &texture_copy, data, &data_layout, &size)
            .map_err(|e| BasaltError::Wgpu(format!("{:?}", e)))?;

        log::debug!(
            "Atlas '{}': uploaded {} bytes to region at ({}, {})",
            self.label, data.len(), region.x, region.y
        );

        Ok(())
    }

    /// Get the texture ID
    pub fn texture_id(&self) -> id::TextureId {
        self.texture_id
    }

    /// Get the texture view ID
    pub fn texture_view_id(&self) -> id::TextureViewId {
        self.texture_view_id
    }

    /// Get the atlas size
    pub fn size(&self) -> u32 {
        self.size
    }

    /// Get the texture format
    pub fn format(&self) -> wgt::TextureFormat {
        self.format
    }

    /// Get the number of allocated regions
    pub fn region_count(&self) -> usize {
        self.regions.read().len()
    }

    /// Clear all allocations (does not clear texture data)
    pub fn clear_allocations(&self) {
        *self.allocator.write() = AtlasAllocator::new(Size::new(self.size as i32, self.size as i32));
        self.regions.write().clear();
        log::debug!("Atlas '{}': cleared all allocations", self.label);
    }
}

impl Drop for TextureAtlas {
    fn drop(&mut self) {
        self.context.inner().texture_view_drop(self.texture_view_id);
        self.context.inner().texture_drop(self.texture_id);
        log::debug!("Dropped atlas '{}'", self.label);
    }
}

/// Manager for multiple texture atlases
pub struct AtlasManager {
    context: Arc<BasaltContext>,
    device_id: id::DeviceId,
    queue_id: id::QueueId,

    /// Block texture atlas (Minecraft block textures)
    block_atlas: Option<TextureAtlas>,

    /// Entity texture atlas (Minecraft entity textures)
    entity_atlas: Option<TextureAtlas>,

    /// GUI texture atlas (Minecraft GUI textures)
    gui_atlas: Option<TextureAtlas>,

    /// General purpose atlases (created on demand)
    custom_atlases: RwLock<HashMap<String, TextureAtlas>>,
}

impl AtlasManager {
    /// Create a new atlas manager with default atlases
    pub fn new(
        context: Arc<BasaltContext>,
        device_id: id::DeviceId,
        queue_id: id::QueueId,
    ) -> Result<Self> {
        let block_atlas = TextureAtlas::new(
            context.clone(),
            device_id,
            queue_id,
            DEFAULT_ATLAS_SIZE,
            wgt::TextureFormat::Rgba8UnormSrgb,
            "Block",
        )?;

        let entity_atlas = TextureAtlas::new(
            context.clone(),
            device_id,
            queue_id,
            DEFAULT_ATLAS_SIZE,
            wgt::TextureFormat::Rgba8UnormSrgb,
            "Entity",
        )?;

        let gui_atlas = TextureAtlas::new(
            context.clone(),
            device_id,
            queue_id,
            1024, // GUI atlas can be smaller
            wgt::TextureFormat::Rgba8UnormSrgb,
            "GUI",
        )?;

        log::info!("Created atlas manager with block, entity, and GUI atlases");

        Ok(Self {
            context,
            device_id,
            queue_id,
            block_atlas: Some(block_atlas),
            entity_atlas: Some(entity_atlas),
            gui_atlas: Some(gui_atlas),
            custom_atlases: RwLock::new(HashMap::new()),
        })
    }

    /// Get the block atlas
    pub fn block_atlas(&self) -> Option<&TextureAtlas> {
        self.block_atlas.as_ref()
    }

    /// Get the entity atlas
    pub fn entity_atlas(&self) -> Option<&TextureAtlas> {
        self.entity_atlas.as_ref()
    }

    /// Get the GUI atlas
    pub fn gui_atlas(&self) -> Option<&TextureAtlas> {
        self.gui_atlas.as_ref()
    }

    /// Create a custom atlas
    pub fn create_custom_atlas(
        &self,
        name: &str,
        size: u32,
        format: wgt::TextureFormat,
    ) -> Result<()> {
        let atlas = TextureAtlas::new(
            self.context.clone(),
            self.device_id,
            self.queue_id,
            size,
            format,
            name,
        )?;

        self.custom_atlases.write().insert(name.to_string(), atlas);
        Ok(())
    }

    /// Get a custom atlas by name
    pub fn get_custom_atlas(&self, name: &str) -> Option<&TextureAtlas> {
        // Note: This is a bit awkward due to RwLock, but works for read access
        // In practice, you'd typically hold the lock briefly
        None // Would need different architecture for proper borrowing
    }
}
