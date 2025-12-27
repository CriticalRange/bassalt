//! Bind group management for wgpu-core 27
//!
//! Handles creation of bind groups and bind group layouts for binding
//! textures, samplers, and uniform buffers to shaders.

use std::borrow::Cow;
use std::collections::HashMap;
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
    pub fn new(context: Arc<BasaltContext>, device_id: id::DeviceId) -> Self {
        Self {
            context,
            device_id,
            entries: Vec::new(),
        }
    }

    /// Add a texture binding
    pub fn add_texture(
        mut self,
        binding: u32,
        view_id: id::TextureViewId,
        sampler_id: Option<id::SamplerId>,
    ) -> Self {
        self.entries.push((
            binding,
            BindingEntry::Texture { view_id, sampler_id },
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

    /// Build the bind group
    ///
    /// This creates both a bind group layout and the bind group itself.
    /// The layout is created dynamically based on the provided entries.
    pub fn build(self) -> Result<id::BindGroupId> {
        let global = self.context.inner();

        // Step 1: Create bind group layout entries
        let mut layout_entries = Vec::new();
        for (binding, entry) in &self.entries {
            match entry {
                BindingEntry::Texture { sampler_id, .. } => {
                    // Texture binding
                    layout_entries.push(wgt::BindGroupLayoutEntry {
                        binding: *binding,
                        visibility: wgt::ShaderStages::FRAGMENT,
                        ty: wgt::BindingType::Texture {
                            sample_type: wgt::TextureSampleType::Float { filterable: true },
                            view_dimension: wgt::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    });

                    // If there's a sampler, add it as the next binding
                    if sampler_id.is_some() {
                        layout_entries.push(wgt::BindGroupLayoutEntry {
                            binding: *binding + 1,
                            visibility: wgt::ShaderStages::FRAGMENT,
                            ty: wgt::BindingType::Sampler(wgt::SamplerBindingType::Filtering),
                            count: None,
                        });
                    }
                }
                BindingEntry::UniformBuffer { .. } => {
                    layout_entries.push(wgt::BindGroupLayoutEntry {
                        binding: *binding,
                        visibility: wgt::ShaderStages::VERTEX | wgt::ShaderStages::FRAGMENT,
                        ty: wgt::BindingType::Buffer {
                            ty: wgt::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    });
                }
            }
        }

        // Step 2: Create bind group layout
        let layout_desc = binding_model::BindGroupLayoutDescriptor {
            label: Some(Cow::Borrowed("Bassalt Dynamic Bind Group Layout")),
            entries: Cow::Owned(layout_entries),
        };

        let (layout_id, layout_error) =
            global.device_create_bind_group_layout(self.device_id, &layout_desc, None);

        if let Some(e) = layout_error {
            return Err(BasaltError::Device(format!(
                "Failed to create bind group layout: {:?}",
                e
            )));
        }

        // Step 3: Create bind group entries
        let mut bind_entries = Vec::new();
        for (binding, entry) in &self.entries {
            match entry {
                BindingEntry::Texture { view_id, sampler_id } => {
                    // Texture view binding
                    bind_entries.push(binding_model::BindGroupEntry {
                        binding: *binding,
                        resource: binding_model::BindingResource::TextureView(*view_id),
                    });

                    // Sampler binding (if present)
                    if let Some(sampler_id) = sampler_id {
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

        // Step 4: Create bind group
        let bind_group_desc = binding_model::BindGroupDescriptor {
            label: Some(Cow::Borrowed("Bassalt Dynamic Bind Group")),
            layout: layout_id,
            entries: Cow::Owned(bind_entries),
        };

        let (bind_group_id, bind_group_error) =
            global.device_create_bind_group(self.device_id, &bind_group_desc, None);

        if let Some(e) = bind_group_error {
            return Err(BasaltError::Device(format!(
                "Failed to create bind group: {:?}",
                e
            )));
        }

        log::debug!(
            "Created bind group with {} entries",
            self.entries.len()
        );

        Ok(bind_group_id)
    }
}
