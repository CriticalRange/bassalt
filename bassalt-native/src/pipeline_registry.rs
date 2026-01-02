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
#[derive(Clone)]
pub struct CachedRenderPipeline {
    /// The render pipeline ID
    pub pipeline_id: id::RenderPipelineId,
    /// Bind group layout IDs (for all groups)
    pub bind_group_layout_ids: Vec<id::BindGroupLayoutId>,
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
        bind_group_layout_ids: Vec<id::BindGroupLayoutId>,
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
        log::info!("About to call create_depth_stencil_state with depth_format={:?}", depth_format);
        let depth_stencil = Self::create_depth_stencil_state(
            key.depth_test_enabled,
            key.depth_write_enabled,
            key.depth_compare,
            depth_format,
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
            bind_group_layout_ids,
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
    fn create_vertex_buffer_layout(format_index: usize) -> Cow<'static, [wgpu_core::pipeline::VertexBufferLayout<'static>]> {
        // Same implementation as in lib.rs - could be refactored to share
        match format_index {
            255 => Cow::Borrowed(&[]), // EMPTY
            0 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
                array_stride: 12,
                step_mode: wgt::VertexStepMode::Vertex,
                attributes: Cow::Owned(vec![wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                }]),
            }]),
            // ... other formats would be here
            _ => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
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
            }]),
        }
    }

    /// Create depth stencil state
    /// NOTE: wgpu-core appears to require depth_stencil state for all pipelines.
    /// Even when a shader doesn't write depth, we need to provide a dummy state.
    fn create_depth_stencil_state(
        depth_test_enabled: bool,
        depth_write_enabled: bool,
        depth_compare: wgt::CompareFunction,
        depth_format: PipelineDepthFormat,
    ) -> Option<wgt::DepthStencilState> {
        // Determine the format to use
        let format = match depth_format {
            PipelineDepthFormat::None => wgt::TextureFormat::Depth32Float,  // Use Depth32Float to match existing depth textures
            PipelineDepthFormat::Depth32Float => wgt::TextureFormat::Depth32Float,
            PipelineDepthFormat::Depth24Plus => wgt::TextureFormat::Depth24Plus,
            PipelineDepthFormat::Depth24PlusStencil8 => wgt::TextureFormat::Depth24PlusStencil8,
        };

        // If the shader doesn't write depth, use a no-op depth state
        let is_no_op = matches!(depth_format, PipelineDepthFormat::None);

        if is_no_op {
            log::info!("Creating pipeline with NO-OP depth state (CompareFunction::Always, no write)");
            Some(wgt::DepthStencilState {
                format,
                depth_write_enabled: false,  // Never write
                depth_compare: wgt::CompareFunction::Always,  // Always pass
                stencil: wgt::StencilState::default(),
                bias: wgt::DepthBiasState::default(),
            })
        } else {
            log::info!("Creating pipeline WITH depth stencil state: {:?}", format);
            Some(wgt::DepthStencilState {
                format,
                depth_write_enabled: if depth_test_enabled { depth_write_enabled } else { false },
                depth_compare: if depth_test_enabled { depth_compare } else { wgt::CompareFunction::Always },
                stencil: wgt::StencilState::default(),
                bias: wgt::DepthBiasState::default(),
            })
        }
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
        };

        assert_eq!(key1, key2);
    }
}
