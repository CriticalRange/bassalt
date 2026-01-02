//! Bind group management for wgpu-core 27
//!
//! Handles creation of bind groups and bind group layouts for binding
//! textures, samplers, and uniform buffers to shaders.

use std::borrow::Cow;
use std::num::NonZero;
use std::sync::Arc;
use wgpu_core::{binding_model, id};
use wgpu_types as wgt;

use crate::context::BasaltContext;
use crate::error::{BasaltError, Result};

/// A binding entry for a bind group
#[derive(Debug, Clone)]
pub enum BindingEntry {
    Texture {
        view_id: id::TextureViewId,
        sampler_id: Option<id::SamplerId>,
        dimension: wgt::TextureViewDimension,
        /// The underlying texture, used to create views with different dimensions
        texture_id: id::TextureId,
    },
    UniformBuffer {
        buffer_id: id::BufferId,
        offset: u64,
        size: NonZero<u64>,
    },
}

/// Builder for creating bind groups dynamically
pub struct BindGroupBuilder {
    context: Arc<BasaltContext>,
    device_id: id::DeviceId,
    entries: Vec<(u32, BindingEntry)>,
}

impl BindGroupBuilder {
    /// Create a new bind group builder
    pub fn new(
        context: Arc<BasaltContext>,
        device_id: id::DeviceId,
    ) -> Self {
        Self {
            context,
            device_id,
            entries: Vec::new(),
        }
    }

    /// Add a texture binding with explicit dimension and texture_id for re-view creation
    pub fn add_texture(
        mut self,
        binding: u32,
        view_id: id::TextureViewId,
        sampler_id: Option<id::SamplerId>,
        dimension: wgt::TextureViewDimension,
        texture_id: id::TextureId,
    ) -> Self {
        self.entries.push((
            binding,
            BindingEntry::Texture { view_id, sampler_id, dimension, texture_id },
        ));
        self
    }

    /// Add a uniform buffer binding
    pub fn add_uniform_buffer(
        mut self,
        binding: u32,
        buffer_id: id::BufferId,
        offset: u64,
        size: u64,
    ) -> Self {
        if let Some(size) = NonZero::new(size) {
            self.entries.push((
                binding,
                BindingEntry::UniformBuffer {
                    buffer_id,
                    offset,
                    size,
                },
            ));
        }
        self
    }

    /// Build the bind group, creating a layout based on actual bindings
    pub fn build(self) -> Result<id::BindGroupId> {
        let global = self.context.inner();

        // First, create bind group layout based on the entries we have
        let mut layout_entries = Vec::new();
        let mut bind_entries = Vec::new();

        for (binding, entry) in &self.entries {
            match entry {
                BindingEntry::Texture { view_id, sampler_id, dimension, .. } => {
                    // Add texture layout entry with actual dimension
                    layout_entries.push(wgt::BindGroupLayoutEntry {
                        binding: *binding,
                        visibility: wgt::ShaderStages::VERTEX | wgt::ShaderStages::FRAGMENT,
                        ty: wgt::BindingType::Texture {
                            sample_type: wgt::TextureSampleType::Float { filterable: true },
                            view_dimension: *dimension,
                            multisampled: false,
                        },
                        count: None,
                    });

                    // Add texture binding entry
                    bind_entries.push(binding_model::BindGroupEntry {
                        binding: *binding,
                        resource: binding_model::BindingResource::TextureView(*view_id),
                    });

                    // Sampler binding (if present)
                    if let Some(sampler_id) = sampler_id {
                        layout_entries.push(wgt::BindGroupLayoutEntry {
                            binding: *binding + 1,
                            visibility: wgt::ShaderStages::VERTEX | wgt::ShaderStages::FRAGMENT,
                            ty: wgt::BindingType::Sampler(wgt::SamplerBindingType::Filtering),
                            count: None,
                        });

                        bind_entries.push(binding_model::BindGroupEntry {
                            binding: *binding + 1,
                            resource: binding_model::BindingResource::Sampler(*sampler_id),
                        });
                    }
                }
                BindingEntry::UniformBuffer {
                    buffer_id,
                    offset,
                    size,
                } => {
                    // WebGPU has a 64KB limit for uniform buffers
                    // For larger buffers, use storage buffer with read_only access
                    const MAX_UNIFORM_BUFFER_SIZE: u64 = 65536;
                    let buffer_size = size.get();
                    
                    let buffer_binding_type = if buffer_size > MAX_UNIFORM_BUFFER_SIZE {
                        log::debug!(
                            "Buffer at binding {} is {} bytes, using storage buffer (limit: {})",
                            binding, buffer_size, MAX_UNIFORM_BUFFER_SIZE
                        );
                        wgt::BufferBindingType::Storage { read_only: true }
                    } else {
                        wgt::BufferBindingType::Uniform
                    };

                    // Add buffer layout entry
                    layout_entries.push(wgt::BindGroupLayoutEntry {
                        binding: *binding,
                        visibility: wgt::ShaderStages::VERTEX | wgt::ShaderStages::FRAGMENT,
                        ty: wgt::BindingType::Buffer {
                            ty: buffer_binding_type,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    });

                    // Add buffer binding entry
                    bind_entries.push(binding_model::BindGroupEntry {
                        binding: *binding,
                        resource: binding_model::BindingResource::Buffer(
                            binding_model::BufferBinding {
                                buffer: *buffer_id,
                                offset: *offset,
                                size: Some(*size),
                            },
                        ),
                    });
                }
            }
        }

        // Create bind group layout
        let layout_desc = binding_model::BindGroupLayoutDescriptor {
            label: Some(Cow::Borrowed("Dynamic Bind Group Layout")),
            entries: Cow::Owned(layout_entries),
        };

        let (layout_id, layout_error) = global.device_create_bind_group_layout(
            self.device_id,
            &layout_desc,
            None,
        );

        if let Some(e) = layout_error {
            return Err(BasaltError::resource_creation("bind group layout", format!("{:?}", e)));
        }

        // Create bind group using the dynamically created layout
        let bind_group_desc = binding_model::BindGroupDescriptor {
            label: Some(Cow::Borrowed("Bassalt Dynamic Bind Group")),
            layout: layout_id,
            entries: Cow::Owned(bind_entries),
        };

        let (bind_group_id, bind_group_error) =
            global.device_create_bind_group(self.device_id, &bind_group_desc, None);

        if let Some(e) = bind_group_error {
            return Err(BasaltError::resource_creation("bind group", format!("{:?}", e)));
        }

        log::debug!(
            "Created bind group with {} entries",
            self.entries.len()
        );

        Ok(bind_group_id)
    }

    /// Build the bind group using an existing layout (from a pipeline)
    /// Uses binding_layouts to determine what type each slot expects
    pub fn build_with_layout(
        self, 
        layout_id: id::BindGroupLayoutId, 
        binding_layouts: &[crate::resource_handles::BindingLayoutEntry]
    ) -> Result<id::BindGroupId> {
        use crate::resource_handles::BindingLayoutType;
        
        let global = self.context.inner();

        // Collect our available resources by type, including texture_id for re-view creation
        let texture_entries: Vec<_> = self.entries.iter()
            .filter_map(|(binding, e)| match e {
                BindingEntry::Texture { view_id, dimension, texture_id, .. } => 
                    Some((*binding, *view_id, *dimension, *texture_id)),
                _ => None,
            })
            .collect();
        
        let sampler_entries: Vec<_> = self.entries.iter()
            .filter_map(|(binding, e)| match e {
                BindingEntry::Texture { sampler_id: Some(s), .. } => Some((*binding, *s)),
                _ => None,
            })
            .collect();
        
        // Collect our available uniform entries WITH binding indices for proper lookup
        let uniform_entries: Vec<_> = self.entries.iter()
            .filter_map(|(binding, e)| match e {
                BindingEntry::UniformBuffer { buffer_id, offset, size } =>
                    Some((*binding, *buffer_id, *offset, size.get())),
                _ => None,
            })
            .collect();
        
        log::info!("build_with_layout: {} textures, {} samplers, {} uniforms available; layout expects {} bindings",
            texture_entries.len(), sampler_entries.len(), uniform_entries.len(), binding_layouts.len());
        
        for (i, layout) in binding_layouts.iter().enumerate() {
            log::info!("  Layout binding {}: {:?} at slot {}", i, layout.ty, layout.binding);
        }

        // Build bind entries by matching layout expectations to our resources
        let mut bind_entries = Vec::new();
        let mut texture_idx = 0;
        let mut sampler_idx = 0;

        const MAX_UNIFORM_BUFFER_SIZE: u64 = 65536;

        for layout_entry in binding_layouts {
            match layout_entry.ty {
                BindingLayoutType::Texture => {
                    if texture_idx < texture_entries.len() {
                        let (_, view_id, current_dimension, texture_id) = texture_entries[texture_idx];
                        
                        // Check if we need to create a new view with different dimension
                        let final_view_id = if let Some(expected_dim) = layout_entry.expected_dimension {
                            if expected_dim != current_dimension {
                                // Create a new view with the correct dimension
                                log::debug!("Texture dimension mismatch at binding {}: expected {:?}, got {:?}. Creating new view.",
                                           layout_entry.binding, expected_dim, current_dimension);
                                
                                let view_desc = wgpu_core::resource::TextureViewDescriptor {
                                    label: Some(Cow::Borrowed("Rebind Texture View")),
                                    format: None,
                                    dimension: Some(expected_dim),
                                    usage: None,
                                    range: wgt::ImageSubresourceRange::default(),
                                };
                                
                                let (new_view_id, error) = global.texture_create_view(
                                    texture_id,
                                    &view_desc,
                                    None,
                                );
                                
                                if let Some(e) = error {
                                    log::error!("Failed to create texture view with dimension {:?}: {:?}", expected_dim, e);
                                    view_id // Fall back to original view
                                } else {
                                    new_view_id
                                }
                            } else {
                                view_id // Dimension matches, use original view
                            }
                        } else {
                            view_id // No expected dimension specified, use original view
                        };
                        
                        bind_entries.push(binding_model::BindGroupEntry {
                            binding: layout_entry.binding,
                            resource: binding_model::BindingResource::TextureView(final_view_id),
                        });
                        texture_idx += 1;
                        log::debug!("Bound texture to slot {}", layout_entry.binding);
                    } else {
                        log::warn!("No texture available for binding {}", layout_entry.binding);
                    }
                }
                BindingLayoutType::Sampler => {
                    if sampler_idx < sampler_entries.len() {
                        let (_, sampler_id) = sampler_entries[sampler_idx];
                        bind_entries.push(binding_model::BindGroupEntry {
                            binding: layout_entry.binding,
                            resource: binding_model::BindingResource::Sampler(sampler_id),
                        });
                        sampler_idx += 1;
                        log::debug!("Bound sampler to slot {}", layout_entry.binding);
                    } else {
                        log::warn!("No sampler available for binding {}", layout_entry.binding);
                    }
                }
                BindingLayoutType::UniformBuffer | BindingLayoutType::StorageBuffer => {
                    // Look up the entry by binding index (not sequential access)
                    let uniform_entry = uniform_entries.iter()
                        .find(|(binding, _, _, _)| *binding == layout_entry.binding);

                    if let Some((_, buffer_id, offset, buffer_size)) = uniform_entry {
                        // Check if buffer size meets shader's minimum requirement
                        if let Some(min_size) = layout_entry.min_binding_size {
                            if *buffer_size < min_size {
                                log::warn!(
                                    "Buffer size {} is smaller than shader expects {} for binding {}, skipping",
                                    buffer_size, min_size, layout_entry.binding
                                );
                                continue;
                            }
                        }

                        // Determine the actual size to bind
                        let effective_size = if layout_entry.ty == BindingLayoutType::UniformBuffer {
                            // For uniform buffers, clamp to 64KB limit
                            (*buffer_size).min(MAX_UNIFORM_BUFFER_SIZE)
                        } else {
                            // Storage buffers don't have this limit
                            *buffer_size
                        };

                        // Use explicit size instead of None to allow smaller buffers than shader declares
                        let binding_size = NonZero::new(effective_size);

                        if binding_size.is_some() {
                            bind_entries.push(binding_model::BindGroupEntry {
                                binding: layout_entry.binding,
                                resource: binding_model::BindingResource::Buffer(
                                    binding_model::BufferBinding {
                                        buffer: *buffer_id,
                                        offset: *offset,
                                        size: binding_size,
                                    },
                                ),
                            });
                            log::debug!(
                                "Bound {} buffer to slot {} (size={})",
                                if layout_entry.ty == BindingLayoutType::StorageBuffer { "storage" } else { "uniform" },
                                layout_entry.binding,
                                effective_size
                            );
                        } else {
                            log::warn!("Buffer size is 0 for binding {}, skipping", layout_entry.binding);
                        }
                    } else {
                        log::debug!("No buffer available for binding {} (expected by shader but not provided)", layout_entry.binding);
                    }
                }
            }
        }
        
        log::debug!("Building bind group with {} entries (layout expects {})", 
                   bind_entries.len(), binding_layouts.len());

        // Create bind group using the provided layout
        let bind_group_desc = binding_model::BindGroupDescriptor {
            label: Some(Cow::Borrowed("Bassalt Pipeline Bind Group")),
            layout: layout_id,
            entries: Cow::Owned(bind_entries),
        };

        let (bind_group_id, bind_group_error) =
            global.device_create_bind_group(self.device_id, &bind_group_desc, None);

        if let Some(e) = bind_group_error {
            log::error!("Failed to create bind group with pipeline layout: {:?}", e);
            return Err(BasaltError::resource_creation("pipeline bind group", format!("{:?}", e)));
        }

        log::debug!(
            "Created bind group {:?} using pipeline layout",
            bind_group_id
        );

        Ok(bind_group_id)
    }
}
