//! Pipeline caching system for render and compute pipelines
//!
//! This module provides a centralized cache for GPU pipelines to avoid redundant
//! shader compilation and pipeline creation. Inspired by wgpu's pipeline caching patterns.
//!
//! # Benefits
//! - **Reduced CPU overhead**: Shader modules are compiled once and reused
//! - **Faster startup**: Common pipelines are cached across frames
//! - **Memory efficiency**: Duplicate pipelines are eliminated
//! - **Better debugging**: Cached pipelines have descriptive labels

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use wgpu_core::{id, pipeline};
use wgpu_types as wgt;

use crate::context::BasaltContext;
use crate::error::{BasaltError, Result};
use crate::resource_handles::{BindingLayoutEntry, PipelineDepthFormat};

/// Cache key for a render pipeline
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RenderPipelineKey {
    /// Hash of the vertex shader WGSL source
    pub vertex_shader_hash: u64,
    /// Hash of the fragment shader WGSL source
    pub fragment_shader_hash: u64,
    /// Primitive topology
    pub topology: wgt::PrimitiveTopology,
    /// Whether depth test is enabled
    pub depth_test_enabled: bool,
    /// Whether depth write is enabled
    pub depth_write_enabled: bool,
    /// Depth compare function
    pub depth_compare: wgt::CompareFunction,
    /// Whether blending is enabled
    pub blend_enabled: bool,
    /// Target format (color attachment format)
    pub target_format: wgt::TextureFormat,
    /// Depth format (CRITICAL: pipelines with different depth formats are incompatible!)
    pub depth_format: PipelineDepthFormat,
    /// Depth bias constant factor (polygon offset units)
    pub depth_bias_constant: i32,
    /// Depth bias slope scale factor (polygon offset factor)
    pub depth_bias_slope_scale: u32, // Stored as bits for hashing
}

/// Cached shader module with metadata
#[derive(Clone)]
pub struct CachedShaderModule {
    /// The shader module ID
    pub module_id: id::ShaderModuleId,
    /// Entry point name
    pub entry_point: String,
    /// WGSL source hash
    pub source_hash: u64,
    /// Label for debugging
    pub label: String,
}

/// Cached render pipeline with metadata
/// Simplified to single bind group (group 0) only
#[derive(Clone)]
pub struct CachedRenderPipeline {
    /// The render pipeline ID
    pub pipeline_id: id::RenderPipelineId,
    /// Bind group layout ID (group 0)
    pub bind_group_layout_id: id::BindGroupLayoutId,
    /// Pipeline layout ID
    pub pipeline_layout_id: id::PipelineLayoutId,
    /// Binding layouts from shader reflection
    pub binding_layouts: Vec<BindingLayoutEntry>,
    /// Depth format this pipeline expects
    pub depth_format: PipelineDepthFormat,
    /// Cache key
    pub key: RenderPipelineKey,
}

/// Pipeline cache manager
///
/// Maintains caches for:
/// - Shader modules (by source hash)
/// - Render pipelines (by RenderPipelineKey)
pub struct PipelineCache {
    /// Cached shader modules
    shader_modules: RwLock<HashMap<u64, CachedShaderModule>>,
    /// Cached render pipelines
    render_pipelines: RwLock<HashMap<RenderPipelineKey, CachedRenderPipeline>>,
    /// Cache statistics
    stats: RwLock<CacheStats>,
}

/// Cache statistics for monitoring effectiveness
#[derive(Debug, Default, Clone, Copy)]
pub struct CacheStats {
    /// Number of shader module cache hits
    pub shader_hits: usize,
    /// Number of shader module cache misses
    pub shader_misses: usize,
    /// Number of render pipeline cache hits
    pub pipeline_hits: usize,
    /// Number of render pipeline cache misses
    pub pipeline_misses: usize,
    /// Total shaders cached
    pub total_shaders: usize,
    /// Total pipelines cached
    pub total_pipelines: usize,
}

impl PipelineCache {
    /// Create a new pipeline cache
    pub fn new() -> Self {
        Self {
            shader_modules: RwLock::new(HashMap::new()),
            render_pipelines: RwLock::new(HashMap::new()),
            stats: RwLock::new(CacheStats::default()),
        }
    }

    /// Get or create a shader module
    ///
    /// Returns the cached shader module if it exists, otherwise creates a new one.
    pub fn get_or_create_shader_module(
        &self,
        context: &Arc<BasaltContext>,
        device_id: id::DeviceId,
        wgsl_source: &str,
        entry_point: &str,
        label: &str,
    ) -> Result<id::ShaderModuleId> {
        let source_hash = Self::hash_wgsl(wgsl_source);

        // Check cache
        {
            let shaders = self.shader_modules.read();
            if let Some(cached) = shaders.get(&source_hash) {
                log::debug!("Shader cache HIT: '{}' (hash: {:x})", label, source_hash);
                self.stats.write().shader_hits += 1;
                return Ok(cached.module_id);
            }
        }

        // Cache miss - create new shader module
        log::debug!("Shader cache MISS: '{}' (hash: {:x})", label, source_hash);
        self.stats.write().shader_misses += 1;

        // Parse WGSL to naga module
        let naga_module = naga::front::wgsl::parse_str(wgsl_source)
            .map_err(|e| BasaltError::ShaderParse {
                error: e.to_string(),
                line: None,
                column: None,
            })?;

        // Create shader module descriptor with descriptive label
        let descriptor = pipeline::ShaderModuleDescriptor {
            label: Some(Cow::Owned(format!("Shader: {}", label))),
            runtime_checks: wgt::ShaderRuntimeChecks::default(),
        };

        let shader_source = pipeline::ShaderModuleSource::Naga(Cow::Owned(naga_module));

        let (module_id, error) = context.inner().device_create_shader_module(
            device_id,
            &descriptor,
            shader_source,
            None,
        );

        if let Some(e) = error {
            return Err(BasaltError::shader_compilation(
                label,
                format!("{:?}", e),
                "unknown",
            ));
        }

        // Cache the shader module
        let cached = CachedShaderModule {
            module_id,
            entry_point: entry_point.to_string(),
            source_hash,
            label: label.to_string(),
        };

        {
            let mut shaders = self.shader_modules.write();
            shaders.insert(source_hash, cached);
            self.stats.write().total_shaders = shaders.len();
        }

        log::info!("Created and cached shader module: '{}' (hash: {:x})", label, source_hash);
        Ok(module_id)
    }

    /// Get or create a render pipeline
    ///
    /// Returns the cached pipeline if it exists, otherwise creates a new one.
    pub fn get_or_create_render_pipeline(
        &self,
        context: &Arc<BasaltContext>,
        device_id: id::DeviceId,
        key: RenderPipelineKey,
        vertex_wgsl: &str,
        fragment_wgsl: &str,
        pipeline_layout_id: id::PipelineLayoutId,
        bind_group_layout_id: id::BindGroupLayoutId,
        binding_layouts: Vec<BindingLayoutEntry>,
        depth_format: PipelineDepthFormat,
        vertex_format_index: usize,
        label: &str,
    ) -> Result<CachedRenderPipeline> {
        // Check cache
        {
            let pipelines = self.render_pipelines.read();
            if let Some(cached) = pipelines.get(&key) {
                log::info!("Pipeline cache HIT: '{}' (hash: {:x}), cached pipeline ID={:?}, depth_format={:?}",
                    label, Self::hash_key(&key), cached.pipeline_id, cached.depth_format);
                self.stats.write().pipeline_hits += 1;
                return Ok(cached.clone());
            }
        }

        // Cache miss - create new pipeline
        log::debug!("Pipeline cache MISS: '{}' (hash: {:x})", label, Self::hash_key(&key));
        self.stats.write().pipeline_misses += 1;

        // Get or create shader modules
        let vs_module = self.get_or_create_shader_module(
            context,
            device_id,
            vertex_wgsl,
            "main",
            &format!("{} - VS", label),
        )?;

        let fs_module = self.get_or_create_shader_module(
            context,
            device_id,
            fragment_wgsl,
            "main",
            &format!("{} - FS", label),
        )?;

        // Create vertex buffer layout
        let vertex_buffers = Self::create_vertex_buffer_layout(vertex_format_index);

        // Create depth stencil state
        log::info!("About to call create_depth_stencil_state with depth_format={:?}, bias=({}, {})", 
            depth_format, key.depth_bias_constant, f32::from_bits(key.depth_bias_slope_scale));
        let depth_stencil = Self::create_depth_stencil_state(
            key.depth_test_enabled,
            key.depth_write_enabled,
            key.depth_compare,
            depth_format,
            key.depth_bias_constant,
            f32::from_bits(key.depth_bias_slope_scale),
        );
        log::info!("create_depth_stencil_state returned: {:?}", depth_stencil.is_some());

        // Create blend state
        let blend = if key.blend_enabled {
            Some(wgt::BlendState {
                color: wgt::BlendComponent {
                    src_factor: wgt::BlendFactor::SrcAlpha,
                    dst_factor: wgt::BlendFactor::OneMinusSrcAlpha,
                    operation: wgt::BlendOperation::Add,
                },
                alpha: wgt::BlendComponent {
                    src_factor: wgt::BlendFactor::One,
                    dst_factor: wgt::BlendFactor::OneMinusSrcAlpha,
                    operation: wgt::BlendOperation::Add,
                },
            })
        } else {
            None
        };

        // Build the render pipeline descriptor
        let descriptor = pipeline::RenderPipelineDescriptor {
            label: Some(Cow::Owned(format!("Render Pipeline: {}", label))),
            layout: Some(pipeline_layout_id),
            vertex: pipeline::VertexState {
                stage: pipeline::ProgrammableStageDescriptor {
                    module: vs_module,
                    entry_point: Some(Cow::Borrowed("main")),
                    constants: Default::default(),
                    zero_initialize_workgroup_memory: true,
                },
                buffers: vertex_buffers,
            },
            primitive: wgt::PrimitiveState {
                topology: key.topology,
                strip_index_format: None,
                front_face: wgt::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgt::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil,
            multisample: wgt::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(pipeline::FragmentState {
                stage: pipeline::ProgrammableStageDescriptor {
                    module: fs_module,
                    entry_point: Some(Cow::Borrowed("main")),
                    constants: Default::default(),
                    zero_initialize_workgroup_memory: true,
                },
                targets: Cow::Owned(vec![Some(wgt::ColorTargetState {
                    format: key.target_format,
                    blend,
                    write_mask: wgt::ColorWrites::ALL,
                })]),
            }),
            multiview: None,
            cache: None,
        };

        // Create the pipeline
        log::info!("Calling wgpu create_render_pipeline with depth_stencil={:?}", descriptor.depth_stencil.is_some());
        let (pipeline_id, error) = context
            .inner()
            .device_create_render_pipeline(device_id, &descriptor, None);

        if let Some(e) = error {
            return Err(BasaltError::PipelineCreation {
                pipeline_name: label.to_string(),
                error: format!("{:?}", e),
                validation_errors: vec![],
            });
        }

        // Cache the pipeline
        let cached = CachedRenderPipeline {
            pipeline_id,
            bind_group_layout_id,
            pipeline_layout_id,
            binding_layouts,
            depth_format,
            key: key.clone(),
        };

        log::info!("Created pipeline with ID {:?}, depth_format={:?}", pipeline_id, depth_format);

        {
            let mut pipelines = self.render_pipelines.write();
            pipelines.insert(key, cached.clone());
            self.stats.write().total_pipelines = pipelines.len();
        }

        log::info!("Created and cached render pipeline: '{}'", label);
        Ok(cached)
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        *self.stats.read()
    }

    /// Clear the cache
    ///
    /// Note: This does not destroy the cached GPU resources, it only clears
    /// the cache entries. The resources will be freed when no longer referenced.
    pub fn clear(&self) {
        let mut shaders = self.shader_modules.write();
        let mut pipelines = self.render_pipelines.write();
        let count = shaders.len() + pipelines.len();
        shaders.clear();
        pipelines.clear();
        *self.stats.write() = CacheStats::default();
        log::info!("Cleared pipeline cache: {} entries removed", count);
    }

    // Helper methods

    /// Hash WGSL source code
    ///
    /// Public method for generating cache keys from shader source.
    /// Used by lib.rs to create RenderPipelineKey before calling get_or_create_render_pipeline.
    pub fn hash_wgsl(wgsl: &str) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        wgsl.hash(&mut hasher);
        hasher.finish()
    }

    /// Hash a render pipeline key
    ///
    /// Public method for generating debug output showing the cache key hash.
    pub fn hash_key(key: &RenderPipelineKey) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    /// Create vertex buffer layout based on format index
    /// Matches the full implementation in lib.rs
    fn create_vertex_buffer_layout(format_index: usize) -> Cow<'static, [wgpu_core::pipeline::VertexBufferLayout<'static>]> {
        match format_index {
            // 255 = EMPTY (no vertex input - shader uses @builtin(vertex_index))
            255 => Cow::Borrowed(&[]),
            // 0 = POSITION (3 floats)
            0 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
                array_stride: 12, // 3 floats * 4 bytes
                step_mode: wgt::VertexStepMode::Vertex,
                attributes: Cow::Owned(vec![
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    },
                ]),
            }]),
            // 1 = POSITION_COLOR (3 floats + 4 floats)
            1 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
                array_stride: 28, // 12 + 16 = 28 bytes
                step_mode: wgt::VertexStepMode::Vertex,
                attributes: Cow::Owned(vec![
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x4,
                        offset: 12,
                        shader_location: 1,
                    },
                ]),
            }]),
            // 2 = POSITION_TEX (3 floats + 2 floats)
            2 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
                array_stride: 20, // 12 + 8 = 20 bytes
                step_mode: wgt::VertexStepMode::Vertex,
                attributes: Cow::Owned(vec![
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x2,
                        offset: 12,
                        shader_location: 1,
                    },
                ]),
            }]),
            // 3 = POSITION_TEX_COLOR (3 floats + 2 floats + 4 floats)
            3 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
                array_stride: 36, // 12 + 8 + 16 = 36 bytes
                step_mode: wgt::VertexStepMode::Vertex,
                attributes: Cow::Owned(vec![
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x2,
                        offset: 12,
                        shader_location: 1,
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x4,
                        offset: 20,
                        shader_location: 2,
                    },
                ]),
            }]),
            // 4 = POSITION_TEX_COLOR_NORMAL (3 floats + 2 floats + 4 floats + 3 floats)
            4 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
                array_stride: 48, // 12 + 8 + 16 + 12 = 48 bytes
                step_mode: wgt::VertexStepMode::Vertex,
                attributes: Cow::Owned(vec![
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x2,
                        offset: 12,
                        shader_location: 1,
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x4,
                        offset: 20,
                        shader_location: 2,
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x3,
                        offset: 36,
                        shader_location: 3,
                    },
                ]),
            }]),
            // 5 = POSITION_COLOR_TEX (3 floats + 4 floats + 2 floats)
            5 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
                array_stride: 36, // 12 + 16 + 8 = 36 bytes
                step_mode: wgt::VertexStepMode::Vertex,
                attributes: Cow::Owned(vec![
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0, // position
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x4,
                        offset: 12,
                        shader_location: 1, // color
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x2,
                        offset: 28,
                        shader_location: 2, // uv
                    },
                ]),
            }]),
            // 6 = POSITION_COLOR_TEX_TEX_TEX_NORMAL (position, color, uv0, uv1, uv2, normal)
            6 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
                array_stride: 64, // 12 + 16 + 8 + 8 + 8 + 12 = 64 bytes
                step_mode: wgt::VertexStepMode::Vertex,
                attributes: Cow::Owned(vec![
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0, // position
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x4,
                        offset: 12,
                        shader_location: 1, // color
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x2,
                        offset: 28,
                        shader_location: 2, // uv0
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x2,
                        offset: 36,
                        shader_location: 3, // uv1
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x2,
                        offset: 44,
                        shader_location: 4, // uv2
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x3,
                        offset: 52,
                        shader_location: 5, // normal
                    },
                ]),
            }]),
            // 7 = POSITION_COLOR_TEX_TEX_NORMAL (position, color, uv0, uv2, normal - skips uv1)
            7 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
                array_stride: 56, // 12 + 16 + 8 + 8 + 12 = 56 bytes
                step_mode: wgt::VertexStepMode::Vertex,
                attributes: Cow::Owned(vec![
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0, // position
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x4,
                        offset: 12,
                        shader_location: 1, // color
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x2,
                        offset: 28,
                        shader_location: 2, // uv0
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x2,
                        offset: 36,
                        shader_location: 3, // uv2
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x3,
                        offset: 44,
                        shader_location: 4, // normal
                    },
                ]),
            }]),
            // 8 = POSITION_COLOR_TEX_TEX (position, color, uv0, uv2 - no normal)
            8 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
                array_stride: 44, // 12 + 16 + 8 + 8 = 44 bytes
                step_mode: wgt::VertexStepMode::Vertex,
                attributes: Cow::Owned(vec![
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0, // position
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x4,
                        offset: 12,
                        shader_location: 1, // color
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x2,
                        offset: 28,
                        shader_location: 2, // uv0
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x2,
                        offset: 36,
                        shader_location: 3, // uv2
                    },
                ]),
            }]),
            // Default to POSITION_TEX_COLOR for unknown formats
            _ => {
                log::warn!("Unknown vertex format index: {}, defaulting to POSITION_TEX_COLOR", format_index);
                Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
                    array_stride: 36,
                    step_mode: wgt::VertexStepMode::Vertex,
                    attributes: Cow::Owned(vec![
                        wgt::VertexAttribute {
                            format: wgt::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgt::VertexAttribute {
                            format: wgt::VertexFormat::Float32x2,
                            offset: 12,
                            shader_location: 1,
                        },
                        wgt::VertexAttribute {
                            format: wgt::VertexFormat::Float32x4,
                            offset: 20,
                            shader_location: 2,
                        },
                    ]),
                }])
            }
        }
    }

    /// Create depth stencil state
    ///
    /// **CRITICAL FIX**: When PipelineDepthFormat::None, return None instead of a dummy state.
    /// This prevents pipeline-renderpass format mismatches that cause draw failures.
    ///
    /// Previously, we created a "no-op" depth state for pipelines without depth output,
    /// but this caused validation errors when render passes didn't have depth attachments.
    /// wgpu-core requires strict format matching between pipeline and render pass.
    fn create_depth_stencil_state(
        depth_test_enabled: bool,
        depth_write_enabled: bool,
        depth_compare: wgt::CompareFunction,
        depth_format: PipelineDepthFormat,
        depth_bias_constant: i32,
        depth_bias_slope_scale: f32,
    ) -> Option<wgt::DepthStencilState> {
        // CRITICAL: If pipeline doesn't write depth, return None
        // This ensures pipeline and render pass depth state match
        if matches!(depth_format, PipelineDepthFormat::None) {
            log::info!("Creating pipeline WITHOUT depth stencil state (shader doesn't write depth)");
            return None;
        }

        // Determine the format to use
        let format = match depth_format {
            PipelineDepthFormat::None => unreachable!(), // Already handled above
            PipelineDepthFormat::Depth32Float => wgt::TextureFormat::Depth32Float,
            PipelineDepthFormat::Depth24Plus => wgt::TextureFormat::Depth24Plus,
            PipelineDepthFormat::Depth24PlusStencil8 => wgt::TextureFormat::Depth24PlusStencil8,
        };

        log::info!("Creating pipeline WITH depth stencil state: format={:?}, bias=({}, {})", 
            format, depth_bias_constant, depth_bias_slope_scale);
        Some(wgt::DepthStencilState {
            format,
            depth_write_enabled: if depth_test_enabled { depth_write_enabled } else { false },
            depth_compare: if depth_test_enabled { depth_compare } else { wgt::CompareFunction::Always },
            stencil: wgt::StencilState::default(),
            bias: wgt::DepthBiasState {
                constant: depth_bias_constant,
                slope_scale: depth_bias_slope_scale,
                clamp: 0.0, // No clamping (matches OpenGL default)
            },
        })
    }
}

impl Default for PipelineCache {
    fn default() -> Self {
        Self::new()
    }
}

// Implement Hash for RenderPipelineKey
impl std::hash::Hash for RenderPipelineKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.vertex_shader_hash.hash(state);
        self.fragment_shader_hash.hash(state);
        self.topology.hash(state);
        self.depth_test_enabled.hash(state);
        self.depth_write_enabled.hash(state);
        self.depth_compare.hash(state);
        self.blend_enabled.hash(state);
        self.target_format.hash(state);
        self.depth_format.hash(state);  // CRITICAL: Include depth_format in hash!
        self.depth_bias_constant.hash(state);  // Include depth bias in hash
        self.depth_bias_slope_scale.hash(state);  // Stored as bits for hashing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_wgsl() {
        let wgsl1 = "fn main() -> @builtin(position) vec4<f32> { return vec4<f32>(); }";
        let wgsl2 = "fn main() -> @builtin(position) vec4<f32> { return vec4<f32>(); }";
        let wgsl3 = "fn main() -> @builtin(position) vec4<f32> { return vec4<f32>(1.0); }";

        assert_eq!(PipelineCache::hash_wgsl(wgsl1), PipelineCache::hash_wgsl(wgsl2));
        assert_ne!(PipelineCache::hash_wgsl(wgsl1), PipelineCache::hash_wgsl(wgsl3));
    }

    #[test]
    fn test_render_pipeline_key() {
        let key1 = RenderPipelineKey {
            vertex_shader_hash: 123,
            fragment_shader_hash: 456,
            topology: wgt::PrimitiveTopology::TriangleList,
            depth_test_enabled: true,
            depth_write_enabled: false,
            depth_compare: wgt::CompareFunction::Less,
            blend_enabled: false,
            target_format: wgt::TextureFormat::Rgba8UnormSrgb,
            depth_format: PipelineDepthFormat::Depth32Float,
            depth_bias_constant: 0,
            depth_bias_slope_scale: 0,
        };

        let key2 = RenderPipelineKey {
            vertex_shader_hash: 123,
            fragment_shader_hash: 456,
            topology: wgt::PrimitiveTopology::TriangleList,
            depth_test_enabled: true,
            depth_write_enabled: false,
            depth_compare: wgt::CompareFunction::Less,
            blend_enabled: false,
            target_format: wgt::TextureFormat::Rgba8UnormSrgb,
            depth_format: PipelineDepthFormat::Depth32Float,
            depth_bias_constant: 0,
            depth_bias_slope_scale: 0,
        };

        assert_eq!(key1, key2);
    }
}
