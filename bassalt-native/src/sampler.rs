//! Sampler management

use wgpu_types as wgt;

/// Sampler descriptor for creating samplers
#[derive(Debug, Clone)]
pub struct SamplerDescriptor {
    pub label: Option<String>,
    pub address_mode_u: wgt::AddressMode,
    pub address_mode_v: wgt::AddressMode,
    pub address_mode_w: wgt::AddressMode,
    pub mag_filter: wgt::FilterMode,
    pub min_filter: wgt::FilterMode,
    pub mipmap_filter: wgt::FilterMode,
    pub lod_min_clamp: f32,
    pub lod_max_clamp: f32,
    pub compare: Option<wgt::CompareFunction>,
    pub anisotropy_clamp: u16,
    pub border_color: Option<wgt::SamplerBorderColor>,
}

impl Default for SamplerDescriptor {
    fn default() -> Self {
        Self {
            label: None,
            address_mode_u: wgt::AddressMode::ClampToEdge,
            address_mode_v: wgt::AddressMode::ClampToEdge,
            address_mode_w: wgt::AddressMode::ClampToEdge,
            mag_filter: wgt::FilterMode::Linear,
            min_filter: wgt::FilterMode::Linear,
            mipmap_filter: wgt::FilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 32.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        }
    }
}
