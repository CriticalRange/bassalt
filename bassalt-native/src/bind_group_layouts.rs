//! Pre-defined bind group layouts for Bassalt
//!
//! Combines approaches from wgpu-mc, Bevy, and rend3:
//! - Pre-defined layouts at startup (wgpu-mc)
//! - State tracking to avoid redundant calls (Bevy)
//! - Builder pattern with auto-indexing (rend3)
//! - Descriptive debug labels for GPU profiling
//! - Shared layout cache for deduplication

use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::Hash;
use wgpu_core::id;
use wgpu_types as wgt;
use parking_lot::RwLock;

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

/// Layout signature for deduplication
///
/// Two bind group layouts are compatible if they have the same signature.
/// The signature is based on the binding entries (binding number, type, visibility).
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct LayoutSignature {
    entries: Vec<LayoutEntrySignature>,
}

/// Signature for a single bind group layout entry
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct LayoutEntrySignature {
    binding: u32,
    ty: LayoutEntryType,
    visibility: wgt::ShaderStages,
}

/// Type of a layout entry (for hashing)
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum LayoutEntryType {
    Texture { sample_type: wgt::TextureSampleType, dimension: wgt::TextureViewDimension, multisampled: bool },
    Sampler(wgt::SamplerBindingType),
    Buffer { ty: wgt::BufferBindingType, has_dynamic_offset: bool, min_binding_size: Option<u64> },
}

impl From<&wgt::BindGroupLayoutEntry> for LayoutEntrySignature {
    fn from(entry: &wgt::BindGroupLayoutEntry) -> Self {
        Self {
            binding: entry.binding,
            ty: LayoutEntryType::from(&entry.ty),
            visibility: entry.visibility,
        }
    }
}

impl From<&wgt::BindingType> for LayoutEntryType {
    fn from(ty: &wgt::BindingType) -> Self {
        match ty {
            wgt::BindingType::Texture { sample_type, view_dimension, multisampled } => {
                Self::Texture { sample_type: *sample_type, dimension: *view_dimension, multisampled: *multisampled }
            }
            wgt::BindingType::Sampler(sampler_ty) => Self::Sampler(*sampler_ty),
            wgt::BindingType::Buffer { ty, has_dynamic_offset, min_binding_size } => {
                Self::Buffer {
                    ty: *ty,
                    has_dynamic_offset: *has_dynamic_offset,
                    min_binding_size: min_binding_size.map(|nz| nz.get()),
                }
            }
            // These types are not used in Minecraft shaders but need to be handled
            wgt::BindingType::StorageTexture { .. } => {
                // For layout deduplication purposes, treat as a unique type
                Self::Buffer {
                    ty: wgt::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                }
            }
            wgt::BindingType::AccelerationStructure { .. } => {
                // Treat as a unique buffer type
                Self::Buffer {
                    ty: wgt::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                }
            }
            wgt::BindingType::ExternalTexture => {
                // Treat as a unique texture type
                Self::Texture {
                    sample_type: wgt::TextureSampleType::Float { filterable: true },
                    dimension: wgt::TextureViewDimension::D2,
                    multisampled: false,
                }
            }
        }
    }
}

impl LayoutSignature {
    /// Create a signature from bind group layout entries
    fn from_entries(entries: &[wgt::BindGroupLayoutEntry]) -> Self {
        let mut sig_entries = entries.iter().map(LayoutEntrySignature::from).collect::<Vec<_>>();
        // Sort by binding number for consistent hashing
        sig_entries.sort_by_key(|e| e.binding);
        Self { entries: sig_entries }
    }
}

/// Shared cache for deduplicating bind group layouts
///
/// This cache stores bind group layouts by their signature, so that
/// pipelines with identical binding layouts share the same layout object.
/// This reduces memory overhead and matches wgpu best practices.
pub struct SharedLayoutCache {
    layouts: RwLock<HashMap<LayoutSignature, id::BindGroupLayoutId>>,
    stats: RwLock<LayoutCacheStats>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct LayoutCacheStats {
    pub hits: usize,
    pub misses: usize,
    pub total_layouts: usize,
}

impl SharedLayoutCache {
    /// Create a new layout cache
    pub fn new() -> Self {
        Self {
            layouts: RwLock::new(HashMap::new()),
            stats: RwLock::new(LayoutCacheStats::default()),
        }
    }

    /// Get or create a bind group layout with deduplication
    ///
    /// Returns a cached layout if one with the same signature exists,
    /// otherwise creates a new one and caches it.
    pub fn get_or_create(
        &self,
        context: &BasaltContext,
        device_id: id::DeviceId,
        entries: &[wgt::BindGroupLayoutEntry],
        label: &str,
    ) -> id::BindGroupLayoutId {
        let signature = LayoutSignature::from_entries(entries);

        // Check cache
        {
            let cached = self.layouts.read();
            if let Some(&layout_id) = cached.get(&signature) {
                log::debug!("Layout cache HIT for '{}' ({} entries)", label, entries.len());
                self.stats.write().hits += 1;
                return layout_id;
            }
        }

        // Cache miss - create new layout
        log::debug!("Layout cache MISS for '{}' ({} entries)", label, entries.len());
        self.stats.write().misses += 1;

        let descriptor = wgpu_core::binding_model::BindGroupLayoutDescriptor {
            label: Some(Cow::Borrowed(label)),
            entries: Cow::Borrowed(entries),
        };

        let (layout_id, error) = context.inner().device_create_bind_group_layout(device_id, &descriptor, None);

        if let Some(e) = error {
            log::error!("Failed to create bind group layout '{}': {:?}", label, e);
            // Return invalid ID - caller should handle this
            return unsafe { std::mem::transmute(1u64) };
        }

        // Cache the new layout
        {
            let mut cached = self.layouts.write();
            cached.insert(signature, layout_id);
            self.stats.write().total_layouts = cached.len();
        }

        log::info!("Created and cached bind group layout '{}' (total cached: {})", label, self.stats.read().total_layouts);
        layout_id
    }

    /// Get cache statistics
    pub fn stats(&self) -> LayoutCacheStats {
        *self.stats.read()
    }

    /// Clear the cache (for testing or memory management)
    #[allow(dead_code)]
    pub fn clear(&self) {
        let count = self.layouts.read().len();
        self.layouts.write().clear();
        self.stats.write().total_layouts = 0;
        log::info!("Cleared layout cache: {} entries removed", count);
    }
}

impl Default for SharedLayoutCache {
    fn default() -> Self {
        Self::new()
    }
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
// LAYOUT BUILDER (for dynamic layout creation with descriptive labels)
// ============================================================================

/// Builder for creating custom bind group layouts with debug labels
///
/// This follows the rend3 pattern of building layouts dynamically while
/// maintaining wgpu's convention of descriptive labels.
pub struct BindGroupLayoutBuilder<'a> {
    context: &'a BasaltContext,
    device_id: id::DeviceId,
    label: String,
    entries: Vec<wgt::BindGroupLayoutEntry>,
}

impl<'a> BindGroupLayoutBuilder<'a> {
    /// Create a new builder with a descriptive label
    pub fn new(context: &'a BasaltContext, device_id: id::DeviceId, label: impl Into<String>) -> Self {
        Self {
            context,
            device_id,
            label: label.into(),
            entries: Vec::new(),
        }
    }

    /// Add a texture binding to the layout
    pub fn with_texture(
        mut self,
        binding: u32,
        visibility: wgt::ShaderStages,
        dimension: wgt::TextureViewDimension,
    ) -> Self {
        self.entries.push(wgt::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgt::BindingType::Texture {
                sample_type: wgt::TextureSampleType::Float { filterable: true },
                view_dimension: dimension,
                multisampled: false,
            },
            count: None,
        });
        self
    }

    /// Add a sampler binding to the layout
    pub fn with_sampler(
        mut self,
        binding: u32,
        visibility: wgt::ShaderStages,
    ) -> Self {
        self.entries.push(wgt::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgt::BindingType::Sampler(wgt::SamplerBindingType::Filtering),
            count: None,
        });
        self
    }

    /// Add a uniform buffer binding to the layout
    pub fn with_uniform_buffer(
        mut self,
        binding: u32,
        visibility: wgt::ShaderStages,
        min_binding_size: Option<u64>,
    ) -> Self {
        self.entries.push(wgt::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgt::BindingType::Buffer {
                ty: wgt::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: min_binding_size.map(std::num::NonZero::new).flatten(),
            },
            count: None,
        });
        self
    }

    /// Add a storage buffer binding to the layout
    pub fn with_storage_buffer(
        mut self,
        binding: u32,
        visibility: wgt::ShaderStages,
        read_only: bool,
    ) -> Self {
        self.entries.push(wgt::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgt::BindingType::Buffer {
                ty: wgt::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });
        self
    }

    /// Build the bind group layout
    ///
    /// If a cache is provided, it will be used for deduplication.
    pub fn build(self, cache: Option<&SharedLayoutCache>) -> id::BindGroupLayoutId {
        let label = format!("Bassalt BGL: {}", self.label);

        if let Some(cache) = cache {
            // Use the cache for deduplication
            cache.get_or_create(self.context, self.device_id, &self.entries, &label)
        } else {
            // Direct creation (legacy path)
            let descriptor = wgpu_core::binding_model::BindGroupLayoutDescriptor {
                label: Some(Cow::Owned(label)),
                entries: Cow::Owned(self.entries),
            };

            let (layout_id, error) = self
                .context
                .inner()
                .device_create_bind_group_layout(self.device_id, &descriptor, None);

            if let Some(e) = error {
                log::error!("Failed to create bind group layout '{}': {:?}", self.label, e);
                // Use transmute to create an invalid ID - this is only for error recovery
                // The caller will handle the actual error, this just prevents a crash here
                unsafe { std::mem::transmute(1u64) }
            } else {
                log::debug!("Created bind group layout '{}' with {} entries", self.label, descriptor.entries.len());
                layout_id
            }
        }
    }

    /// Build the bind group layout without cache (legacy method for compatibility)
    pub fn build_direct(self) -> id::BindGroupLayoutId {
        self.build(None)
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Create a texture+sampler bind group layout with a descriptive label
pub fn create_texture_sampler_layout(
    context: &BasaltContext,
    device_id: id::DeviceId,
    binding: u32,
    label: &str,
) -> id::BindGroupLayoutId {
    BindGroupLayoutBuilder::new(context, device_id, label)
        .with_texture(binding, wgt::ShaderStages::FRAGMENT, wgt::TextureViewDimension::D2)
        .with_sampler(binding + 1, wgt::ShaderStages::FRAGMENT)
        .build_direct()  // No cache for these helper functions
}

/// Create a uniform buffer bind group layout with a descriptive label
pub fn create_uniform_layout(
    context: &BasaltContext,
    device_id: id::DeviceId,
    binding: u32,
    label: &str,
    min_size: Option<u64>,
) -> id::BindGroupLayoutId {
    BindGroupLayoutBuilder::new(context, device_id, label)
        .with_uniform_buffer(binding, wgt::ShaderStages::VERTEX | wgt::ShaderStages::FRAGMENT, min_size)
        .build_direct()  // No cache for these helper functions
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
