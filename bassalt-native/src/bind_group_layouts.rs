//! Pre-defined bind group layouts for Bassalt
//! Combines approaches from wgpu-mc, Bevy, and rend3:
//! - Pre-defined layouts at startup (wgpu-mc)
//! - State tracking to avoid redundant calls (Bevy)
//! - Builder pattern with auto-indexing (rend3)

use std::collections::HashMap;
use wgpu_core::id;
use wgpu_types as wgt;
use std::borrow::Cow;

use crate::context::BasaltContext;

/// Pre-defined bind group layout types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindGroupLayoutType {
    /// Texture + Sampler (group 0) - for textured rendering
    TextureSampler,
    /// Single texture only
    Texture,
    /// Single sampler only  
    Sampler,
    /// Uniform buffer (for matrices, etc.)
    Uniform,
    /// Storage buffer (SSBO)
    Storage,
    /// Empty (no bindings)
    Empty,
}

/// Collection of pre-defined bind group layouts
pub struct BindGroupLayouts {
    pub layouts: HashMap<BindGroupLayoutType, id::BindGroupLayoutId>,
}

impl BindGroupLayouts {
    /// Create all pre-defined bind group layouts
    pub fn new(context: &BasaltContext, device_id: id::DeviceId) -> Self {
        let mut layouts = HashMap::new();
        
        // TextureSampler layout: texture at binding 0, sampler at binding 1
        // Use VERTEX | FRAGMENT visibility for maximum compatibility
        let texture_sampler_entries = [
            wgt::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgt::ShaderStages::VERTEX | wgt::ShaderStages::FRAGMENT,
                ty: wgt::BindingType::Texture {
                    sample_type: wgt::TextureSampleType::Float { filterable: true },
                    view_dimension: wgt::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgt::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgt::ShaderStages::VERTEX | wgt::ShaderStages::FRAGMENT,
                ty: wgt::BindingType::Sampler(wgt::SamplerBindingType::Filtering),
                count: None,
            },
        ];
        
        let texture_sampler_desc = wgpu_core::binding_model::BindGroupLayoutDescriptor {
            label: Some(Cow::Borrowed("Bassalt TextureSampler Layout")),
            entries: Cow::Borrowed(&texture_sampler_entries),
        };
        
        let (texture_sampler_id, _) = context.inner().device_create_bind_group_layout(
            device_id,
            &texture_sampler_desc,
            None,
        );
        layouts.insert(BindGroupLayoutType::TextureSampler, texture_sampler_id);
        
        // Uniform layout: uniform buffer at binding 0
        let uniform_entries = [
            wgt::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgt::ShaderStages::VERTEX | wgt::ShaderStages::FRAGMENT,
                ty: wgt::BindingType::Buffer {
                    ty: wgt::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ];
        
        let uniform_desc = wgpu_core::binding_model::BindGroupLayoutDescriptor {
            label: Some(Cow::Borrowed("Bassalt Uniform Layout")),
            entries: Cow::Borrowed(&uniform_entries),
        };
        
        let (uniform_id, _) = context.inner().device_create_bind_group_layout(
            device_id,
            &uniform_desc,
            None,
        );
        layouts.insert(BindGroupLayoutType::Uniform, uniform_id);
        
        // Storage layout: storage buffer at binding 0
        let storage_entries = [
            wgt::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgt::ShaderStages::VERTEX | wgt::ShaderStages::FRAGMENT,
                ty: wgt::BindingType::Buffer {
                    ty: wgt::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ];
        
        let storage_desc = wgpu_core::binding_model::BindGroupLayoutDescriptor {
            label: Some(Cow::Borrowed("Bassalt Storage Layout")),
            entries: Cow::Borrowed(&storage_entries),
        };
        
        let (storage_id, _) = context.inner().device_create_bind_group_layout(
            device_id,
            &storage_desc,
            None,
        );
        layouts.insert(BindGroupLayoutType::Storage, storage_id);
        
        // Empty layout: no bindings
        let empty_desc = wgpu_core::binding_model::BindGroupLayoutDescriptor {
            label: Some(Cow::Borrowed("Bassalt Empty Layout")),
            entries: Cow::Borrowed(&[]),
        };
        
        let (empty_id, _) = context.inner().device_create_bind_group_layout(
            device_id,
            &empty_desc,
            None,
        );
        layouts.insert(BindGroupLayoutType::Empty, empty_id);
        
        log::info!("Created {} pre-defined bind group layouts", layouts.len());
        
        Self { layouts }
    }
    
    /// Get a layout by type
    pub fn get(&self, layout_type: BindGroupLayoutType) -> Option<id::BindGroupLayoutId> {
        self.layouts.get(&layout_type).copied()
    }
}

/// Determine which bind group layout type to use based on resource name
pub fn get_layout_type_for_resource(name: &str) -> BindGroupLayoutType {
    match name {
        // Texture resources
        "Sampler0" | "Sampler1" | "Sampler2" | "InSampler" | "DiffuseSampler" | "Texture" => {
            BindGroupLayoutType::TextureSampler
        }
        // Uniform resources
        "Globals" | "Projection" | "ModelViewMat" | "ProjMat" | "Fog" | "Lighting" | 
        "BlurConfig" | "SamplerInfo" | "ColorModulator" => {
            BindGroupLayoutType::Uniform
        }
        // Storage resources
        "DynamicTransforms" => {
            BindGroupLayoutType::Storage
        }
        // Default to texture+sampler for unknown textures
        _ if name.contains("Sampler") || name.contains("Texture") => {
            BindGroupLayoutType::TextureSampler
        }
        // Default to uniform for unknown uniforms
        _ => {
            BindGroupLayoutType::Uniform
        }
    }
}

/// Determine bind group index for a resource based on wgpu-mc convention
/// Group 0: Textures + Samplers
/// Group 1: Dynamic uniforms (transforms, etc.)
/// Group 2: Projection/static uniforms
pub fn get_bind_group_index_for_resource(name: &str) -> u32 {
    match name {
        // Textures at group 0
        "Sampler0" | "Sampler1" | "Sampler2" | "InSampler" | "DiffuseSampler" | "Texture" => 0,
        // Dynamic transforms at group 1
        "DynamicTransforms" | "Globals" | "ModelViewMat" | "Lighting" | "Fog" | 
        "BlurConfig" | "SamplerInfo" | "ColorModulator" => 1,
        // Projection at group 2
        "Projection" | "ProjMat" => 2,
        // Default based on type
        _ if name.contains("Sampler") || name.contains("Texture") => 0,
        _ => 1,
    }
}

// ============================================================================
// STATE TRACKING (inspired by Bevy's TrackedRenderPass)
// ============================================================================

/// Tracks render pass state to avoid redundant wgpu calls
/// This is similar to Bevy's DrawState pattern
#[derive(Default)]
pub struct RenderPassState {
    current_pipeline: Option<id::RenderPipelineId>,
    bind_groups: [Option<id::BindGroupId>; 4], // Support up to 4 bind groups
    vertex_buffers: [Option<(id::BufferId, u64)>; 4], // buffer id + offset
    index_buffer: Option<(id::BufferId, u64)>,
}

impl RenderPassState {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Check if pipeline is already set
    pub fn is_pipeline_set(&self, pipeline_id: id::RenderPipelineId) -> bool {
        self.current_pipeline == Some(pipeline_id)
    }
    
    /// Mark pipeline as set
    pub fn set_pipeline(&mut self, pipeline_id: id::RenderPipelineId) {
        self.current_pipeline = Some(pipeline_id);
        // Clear bind groups when pipeline changes (they may be incompatible)
        self.bind_groups = [None; 4];
    }
    
    /// Check if bind group is already set at index
    pub fn is_bind_group_set(&self, index: usize, bind_group_id: id::BindGroupId) -> bool {
        if index >= self.bind_groups.len() {
            return false;
        }
        self.bind_groups[index] == Some(bind_group_id)
    }
    
    /// Mark bind group as set
    pub fn set_bind_group(&mut self, index: usize, bind_group_id: id::BindGroupId) {
        if index < self.bind_groups.len() {
            self.bind_groups[index] = Some(bind_group_id);
        }
    }
    
    /// Check if vertex buffer is already set
    pub fn is_vertex_buffer_set(&self, slot: usize, buffer_id: id::BufferId, offset: u64) -> bool {
        if slot >= self.vertex_buffers.len() {
            return false;
        }
        self.vertex_buffers[slot] == Some((buffer_id, offset))
    }
    
    /// Mark vertex buffer as set
    pub fn set_vertex_buffer(&mut self, slot: usize, buffer_id: id::BufferId, offset: u64) {
        if slot < self.vertex_buffers.len() {
            self.vertex_buffers[slot] = Some((buffer_id, offset));
        }
    }
    
    /// Check if index buffer is already set
    pub fn is_index_buffer_set(&self, buffer_id: id::BufferId, offset: u64) -> bool {
        self.index_buffer == Some((buffer_id, offset))
    }
    
    /// Mark index buffer as set
    pub fn set_index_buffer(&mut self, buffer_id: id::BufferId, offset: u64) {
        self.index_buffer = Some((buffer_id, offset));
    }
    
    /// Reset state (call at start of new render pass)
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
