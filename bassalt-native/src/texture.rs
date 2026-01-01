//! Texture management

use wgpu_types as wgt;

/// Texture descriptor for creating textures
#[derive(Debug, Clone)]
pub struct TextureDescriptor {
    pub label: Option<String>,
    pub size: wgt::Extent3d,
    pub mip_level_count: u32,
    pub sample_count: u32,
    pub dimension: wgt::TextureDimension,
    pub format: wgt::TextureFormat,
    pub usage: wgt::TextureUsages,
    pub view_formats: Vec<wgt::TextureFormat>,
}

impl Default for TextureDescriptor {
    fn default() -> Self {
        Self {
            label: None,
            size: wgt::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgt::TextureDimension::D2,
            format: wgt::TextureFormat::Rgba8Unorm,
            usage: wgt::TextureUsages::TEXTURE_BINDING,
            view_formats: vec![],
        }
    }
}
