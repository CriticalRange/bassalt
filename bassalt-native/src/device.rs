//! GPU device wrapper - main interface for rendering operations

use std::borrow::Cow;
use std::sync::Arc;
use wgpu_core::id;
use wgpu_core::pipeline;
use wgpu_core::command;
use wgpu_types as wgt;

use crate::context::BasaltContext;
use crate::surface::BasaltSurface;
use crate::pipeline::RenderPipelineDescriptor;
use crate::error::{BasaltError, Result};

/// Main device wrapper
pub struct BasaltDevice {
    context: Arc<BasaltContext>,
    device_id: id::DeviceId,
    queue_id: id::QueueId,
    surface: Option<BasaltSurface>,
    limits: wgt::Limits,
    info: String,
    // Cached swapchain state
    current_swapchain_texture: parking_lot::Mutex<Option<id::TextureId>>,
    // Track the main framebuffer that should be presented
    main_framebuffer: parking_lot::Mutex<Option<id::TextureId>>,
    swapchain_width: u32,
    swapchain_height: u32,
    swapchain_format: wgt::TextureFormat,
    // Cached blit pipeline for format conversion
    blit_bind_group_layout: parking_lot::Mutex<Option<id::BindGroupLayoutId>>,
    blit_pipeline: parking_lot::Mutex<Option<id::RenderPipelineId>>,
    // Shared bind group layout and pipeline layout for Minecraft rendering
    shared_bind_group_layout: id::BindGroupLayoutId,
    shared_pipeline_layout: id::PipelineLayoutId,
}

impl BasaltDevice {
    /// Create a new device
    pub fn new(
        context: Arc<BasaltContext>,
        device_id: id::DeviceId,
        queue_id: id::QueueId,
        surface: Option<BasaltSurface>,
        width: u32,
        height: u32,
        swapchain_format: wgt::TextureFormat,
    ) -> Result<Self> {
        let limits = context
            .inner()
            .device_limits(device_id);

        let info = format!(
            "Basalt Renderer (wgpu-core)\nAdapter: {}",
            "Unknown"
        );

        // Create shared bind group layout and pipeline layout
        let (shared_bind_group_layout, shared_pipeline_layout) =
            Self::create_shared_layouts(&context, device_id)?;

        log::info!("Created shared pipeline layout for Minecraft rendering");

        Ok(Self {
            context,
            device_id,
            queue_id,
            surface,
            limits,
            info,
            current_swapchain_texture: parking_lot::Mutex::new(None),
            main_framebuffer: parking_lot::Mutex::new(None),
            swapchain_width: width,
            swapchain_height: height,
            swapchain_format,
            blit_bind_group_layout: parking_lot::Mutex::new(None),
            blit_pipeline: parking_lot::Mutex::new(None),
            shared_bind_group_layout,
            shared_pipeline_layout,
        })
    }

    /// Create shared bind group layout and pipeline layout
    /// This creates a single layout that can handle all of Minecraft's binding needs
    fn create_shared_layouts(
        context: &Arc<BasaltContext>,
        device_id: id::DeviceId,
    ) -> Result<(id::BindGroupLayoutId, id::PipelineLayoutId)> {
        let global = context.inner();

        // Create a bind group layout with enough bindings for Minecraft:
        // - Bindings 0-31: 16 texture+sampler pairs (even=texture, odd=sampler)
        // - Bindings 32-35: 4 uniform buffers
        let mut layout_entries = Vec::new();

        // Add 16 texture+sampler pairs
        // Make them visible to both vertex and fragment stages (some shaders use textures in VS)
        for i in 0..16 {
            // Texture binding (even numbers: 0, 2, 4, ...)
            layout_entries.push(wgt::BindGroupLayoutEntry {
                binding: i * 2,
                visibility: wgt::ShaderStages::VERTEX | wgt::ShaderStages::FRAGMENT,
                ty: wgt::BindingType::Texture {
                    sample_type: wgt::TextureSampleType::Float { filterable: true },
                    view_dimension: wgt::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });

            // Sampler binding (odd numbers: 1, 3, 5, ...)
            layout_entries.push(wgt::BindGroupLayoutEntry {
                binding: i * 2 + 1,
                visibility: wgt::ShaderStages::VERTEX | wgt::ShaderStages::FRAGMENT,
                ty: wgt::BindingType::Sampler(wgt::SamplerBindingType::Filtering),
                count: None,
            });
        }

        // Add 4 uniform buffer bindings
        for i in 0..4 {
            layout_entries.push(wgt::BindGroupLayoutEntry {
                binding: 32 + i,
                visibility: wgt::ShaderStages::VERTEX | wgt::ShaderStages::FRAGMENT,
                ty: wgt::BindingType::Buffer {
                    ty: wgt::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
        }

        // Create bind group layout
        let bgl_desc = wgpu_core::binding_model::BindGroupLayoutDescriptor {
            label: Some(Cow::Borrowed("Bassalt Shared Bind Group Layout")),
            entries: Cow::Owned(layout_entries),
        };

        let (bgl_id, bgl_error) = global.device_create_bind_group_layout(device_id, &bgl_desc, None);

        if let Some(e) = bgl_error {
            return Err(BasaltError::Device(format!(
                "Failed to create shared bind group layout: {:?}",
                e
            )));
        }

        // Create pipeline layout from the bind group layout
        let pl_desc = wgpu_core::binding_model::PipelineLayoutDescriptor {
            label: Some(Cow::Borrowed("Bassalt Shared Pipeline Layout")),
            bind_group_layouts: Cow::Owned(vec![bgl_id]),
            push_constant_ranges: Cow::Borrowed(&[]),
        };

        let (pl_id, pl_error) = global.device_create_pipeline_layout(device_id, &pl_desc, None);

        if let Some(e) = pl_error {
            return Err(BasaltError::Device(format!(
                "Failed to create shared pipeline layout: {:?}",
                e
            )));
        }

        Ok((bgl_id, pl_id))
    }

    /// Get the device ID
    pub fn id(&self) -> id::DeviceId {
        self.device_id
    }

    /// Get the queue ID
    pub fn queue_id(&self) -> id::QueueId {
        self.queue_id
    }

    /// Get the context
    pub fn context(&self) -> &Arc<BasaltContext> {
        &self.context
    }

    /// Get the shared bind group layout for Minecraft rendering
    pub fn shared_bind_group_layout(&self) -> id::BindGroupLayoutId {
        self.shared_bind_group_layout
    }

    /// Get the shared pipeline layout for Minecraft rendering
    pub fn shared_pipeline_layout(&self) -> id::PipelineLayoutId {
        self.shared_pipeline_layout
    }

    /// Acquire the swapchain texture for rendering
    pub fn acquire_swapchain_texture(&self) -> Result<id::TextureId> {
        // Check if we already have one
        if let Some(texture_id) = *self.current_swapchain_texture.lock() {
            log::debug!("Using cached swapchain texture: {:?}", texture_id);
            return Ok(texture_id);
        }

        let surface = self.surface.as_ref()
            .ok_or_else(|| BasaltError::Surface("No surface available".into()))?;

        // Get the current swapchain texture
        let output = self.context.inner().surface_get_current_texture(
            surface.id(),
            None,
        ).map_err(|e| BasaltError::Surface(format!("Failed to acquire swapchain texture: {:?}", e)))?;

        let texture_id = output.texture
            .ok_or_else(|| BasaltError::Surface("Swapchain texture not available".into()))?;

        // Cache it
        *self.current_swapchain_texture.lock() = Some(texture_id);

        log::info!("Acquired swapchain texture: {:?}", texture_id);
        Ok(texture_id)
    }

    /// Get the current swapchain texture if available
    pub fn get_swapchain_texture(&self) -> Option<id::TextureId> {
        *self.current_swapchain_texture.lock()
    }

    /// Blit from source texture to swapchain using a render pass
    /// This handles format conversion (e.g., RGBA -> BGRA)
    fn blit_to_swapchain(
        &self,
        src_texture: id::TextureId,
        dst_texture: id::TextureId,
    ) -> Result<()> {
        // Use render-based blit for format conversion
        // The blit shader samples from the source texture and renders to the swapchain,
        // handling RGBA -> BGRA conversion automatically
        self.render_blit(src_texture, dst_texture)
    }

    /// Render-based blit for format conversion
    fn render_blit(
        &self,
        src_texture: id::TextureId,
        dst_texture: id::TextureId,
    ) -> Result<()> {
        // Create blit shader and pipeline (cached in device)
        let blit_pipeline = self.get_or_create_blit_pipeline()?;

        // Create texture views
        let src_view_desc = wgpu_core::resource::TextureViewDescriptor {
            label: Some(Cow::Borrowed("Blit Source View")),
            format: None,
            dimension: None,
            usage: Some(wgt::TextureUsages::TEXTURE_BINDING),
            range: wgt::ImageSubresourceRange {
                aspect: wgt::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            },
        };

        let dst_view_desc = wgpu_core::resource::TextureViewDescriptor {
            label: Some(Cow::Borrowed("Blit Dest View")),
            format: None,
            dimension: None,
            usage: Some(wgt::TextureUsages::RENDER_ATTACHMENT),
            range: wgt::ImageSubresourceRange {
                aspect: wgt::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            },
        };

        let (src_view, error) = self.context.inner().texture_create_view(
            src_texture,
            &src_view_desc,
            None,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("Failed to create source view: {:?}", e)));
        }

        let (dst_view, error) = self.context.inner().texture_create_view(
            dst_texture,
            &dst_view_desc,
            None,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("Failed to create dest view: {:?}", e)));
        }

        // Create sampler
        let sampler_desc = wgpu_core::resource::SamplerDescriptor {
            label: Some(Cow::Borrowed("Blit Sampler")),
            address_modes: [
                wgt::AddressMode::ClampToEdge,
                wgt::AddressMode::ClampToEdge,
                wgt::AddressMode::ClampToEdge,
            ],
            mag_filter: wgt::FilterMode::Linear,
            min_filter: wgt::FilterMode::Linear,
            mipmap_filter: wgt::FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        };

        let (sampler_id, error) = self.context.inner().device_create_sampler(
            self.device_id,
            &sampler_desc,
            None,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("Failed to create sampler: {:?}", e)));
        }

        // Create bind group
        let bind_group_entries = vec![
            wgpu_core::binding_model::BindGroupEntry {
                binding: 0,
                resource: wgpu_core::binding_model::BindingResource::TextureView(src_view),
            },
            wgpu_core::binding_model::BindGroupEntry {
                binding: 1,
                resource: wgpu_core::binding_model::BindingResource::Sampler(sampler_id),
            },
        ];

        let bind_group_desc = wgpu_core::binding_model::BindGroupDescriptor {
            label: Some(Cow::Borrowed("Blit Bind Group")),
            layout: blit_pipeline.0, // bind group layout
            entries: Cow::Borrowed(&bind_group_entries),
        };

        let (bind_group_id, error) = self.context.inner().device_create_bind_group(
            self.device_id,
            &bind_group_desc,
            None,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("Failed to create bind group: {:?}", e)));
        }

        // Create command encoder
        let encoder_desc = wgt::CommandEncoderDescriptor {
            label: Some(Cow::Borrowed("Blit Encoder")),
        };

        let (encoder_id, error) = self.context.inner().device_create_command_encoder(
            self.device_id,
            &encoder_desc,
            None,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("Failed to create encoder: {:?}", e)));
        }

        // Create render pass
        let color_attachments = vec![Some(wgpu_core::command::RenderPassColorAttachment {
            view: dst_view,
            resolve_target: None,
            load_op: wgpu_core::command::LoadOp::Load, // Don't clear, we'll overwrite everything
            store_op: wgpu_core::command::StoreOp::Store,
            depth_slice: None,
        })];

        let pass_desc = wgpu_core::command::RenderPassDescriptor {
            label: Some(Cow::Borrowed("Blit Pass")),
            color_attachments: Cow::Borrowed(&color_attachments),
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        };

        // Begin render pass
        let (mut render_pass, error) = self.context.inner().command_encoder_begin_render_pass(
            encoder_id,
            &pass_desc,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("Failed to begin render pass: {:?}", e)));
        }

        // Set pipeline and bind group
        if let Err(e) = self.context.inner().render_pass_set_pipeline(&mut render_pass, blit_pipeline.1) {
            return Err(BasaltError::Wgpu(format!("Failed to set pipeline: {:?}", e)));
        }

        if let Err(e) = self.context.inner().render_pass_set_bind_group(
            &mut render_pass,
            0,
            Some(bind_group_id),
            &[],
        ) {
            return Err(BasaltError::Wgpu(format!("Failed to set bind group: {:?}", e)));
        }

        // Draw fullscreen triangle (3 vertices, no vertex buffer needed - generated in shader)
        if let Err(e) = self.context.inner().render_pass_draw(
            &mut render_pass,
            3, // vertex count
            1, // instance count
            0, // first vertex
            0, // first instance
        ) {
            return Err(BasaltError::Wgpu(format!("Failed to draw: {:?}", e)));
        }

        // End render pass
        if let Err(e) = self.context.inner().render_pass_end(&mut render_pass) {
            return Err(BasaltError::Wgpu(format!("Failed to end render pass: {:?}", e)));
        }

        // Finish and submit
        let (command_buffer, error) = self.context.inner().command_encoder_finish(
            encoder_id,
            &wgt::CommandBufferDescriptor::default(),
            None,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("Failed to finish encoder: {:?}", e)));
        }

        self.context
            .inner()
            .queue_submit(self.queue_id, &[command_buffer])
            .map_err(|e| BasaltError::Wgpu(format!("Failed to submit: {:?}", e)))?;

        log::debug!("Render-based blit completed with shader sampling");
        Ok(())
    }

    /// Get or create the blit pipeline (cached)
    fn get_or_create_blit_pipeline(&self) -> Result<(id::BindGroupLayoutId, id::RenderPipelineId)> {
        // Check if we already have a cached pipeline
        {
            let bgl_lock = self.blit_bind_group_layout.lock();
            let pipeline_lock = self.blit_pipeline.lock();
            if let (Some(bgl_id), Some(pipeline_id)) = (*bgl_lock, *pipeline_lock) {
                return Ok((bgl_id, pipeline_id));
            }
        }

        // Create bind group layout
        let bgl_entries = vec![
            wgt::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgt::ShaderStages::FRAGMENT,
                ty: wgt::BindingType::Texture {
                    sample_type: wgt::TextureSampleType::Float { filterable: true },
                    view_dimension: wgt::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgt::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgt::ShaderStages::FRAGMENT,
                ty: wgt::BindingType::Sampler(wgt::SamplerBindingType::Filtering),
                count: None,
            },
        ];

        let bgl_desc = wgpu_core::binding_model::BindGroupLayoutDescriptor {
            label: Some(Cow::Borrowed("Blit BGL")),
            entries: Cow::Borrowed(&bgl_entries),
        };

        let (bgl_id, error) = self.context.inner().device_create_bind_group_layout(
            self.device_id,
            &bgl_desc,
            None,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("Failed to create bind group layout: {:?}", e)));
        }

        // Create pipeline layout
        let pipeline_layout_desc = wgpu_core::binding_model::PipelineLayoutDescriptor {
            label: Some(Cow::Borrowed("Blit Pipeline Layout")),
            bind_group_layouts: Cow::Borrowed(&[bgl_id]),
            push_constant_ranges: Cow::Borrowed(&[]),
        };

        let (pipeline_layout_id, error) = self.context.inner().device_create_pipeline_layout(
            self.device_id,
            &pipeline_layout_desc,
            None,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("Failed to create pipeline layout: {:?}", e)));
        }

        // Create shader module with blit shader
        let blit_shader_source = r#"
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    // Fullscreen triangle
    let x = f32((vertex_index << 1u) & 2u);
    let y = f32(vertex_index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
}

@group(0) @binding(0) var src_texture: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let tex_size = vec2<f32>(textureDimensions(src_texture));
    let uv = position.xy / tex_size;
    return textureSample(src_texture, src_sampler, uv);
}
"#;

        let shader_module = self.parse_wgsl(blit_shader_source)?;
        let shader_module_desc = wgpu_core::pipeline::ShaderModuleDescriptor {
            label: Some(Cow::Borrowed("Blit Shader")),
            runtime_checks: wgt::ShaderRuntimeChecks::default(),
        };

        let shader_source = wgpu_core::pipeline::ShaderModuleSource::Naga(Cow::Owned(shader_module));

        let (shader_module_id, error) = self.context.inner().device_create_shader_module(
            self.device_id,
            &shader_module_desc,
            shader_source,
            None,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("Failed to create shader module: {:?}", e)));
        }

        // Create render pipeline
        use hashbrown::HashMap;
        let pipeline_desc = wgpu_core::pipeline::RenderPipelineDescriptor {
            label: Some(Cow::Borrowed("Blit Pipeline")),
            layout: Some(pipeline_layout_id),
            vertex: wgpu_core::pipeline::VertexState {
                stage: wgpu_core::pipeline::ProgrammableStageDescriptor {
                    module: shader_module_id,
                    entry_point: Some(Cow::Borrowed("vs_main")),
                    constants: HashMap::<String, f64>::new(),
                    zero_initialize_workgroup_memory: true,
                },
                buffers: Cow::Borrowed(&[]),
            },
            primitive: wgt::PrimitiveState {
                topology: wgt::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgt::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgt::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgt::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu_core::pipeline::FragmentState {
                stage: wgpu_core::pipeline::ProgrammableStageDescriptor {
                    module: shader_module_id,
                    entry_point: Some(Cow::Borrowed("fs_main")),
                    constants: HashMap::<String, f64>::new(),
                    zero_initialize_workgroup_memory: true,
                },
                targets: Cow::Borrowed(&[Some(wgt::ColorTargetState {
                    format: self.swapchain_format, // Use actual swapchain format
                    blend: None,
                    write_mask: wgt::ColorWrites::ALL,
                })]),
            }),
            multiview: None,
            cache: None,
        };

        let (pipeline_id, error) = self.context.inner().device_create_render_pipeline(
            self.device_id,
            &pipeline_desc,
            None,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("Failed to create render pipeline: {:?}", e)));
        }

        // Cache the pipeline and bind group layout for future use
        *self.blit_bind_group_layout.lock() = Some(bgl_id);
        *self.blit_pipeline.lock() = Some(pipeline_id);

        log::info!("Created blit pipeline (cached for future frames)");
        Ok((bgl_id, pipeline_id))
    }

    /// Present the current frame
    pub fn present_frame(&self) -> Result<()> {
        let surface = match &self.surface {
            Some(s) => s,
            None => {
                log::debug!("No surface, skipping present");
                return Ok(());
            }
        };

        // Acquire the swapchain texture
        let swapchain_texture = match self.acquire_swapchain_texture() {
            Ok(t) => t,
            Err(e) => {
                log::warn!("Failed to acquire swapchain texture: {}", e);
                return Ok(()); // Don't fail, just skip this frame
            }
        };

        // Get the main framebuffer to blit from (if we have one)
        if let Some(main_fb) = *self.main_framebuffer.lock() {
            log::info!("Blitting main framebuffer {:?} to swapchain {:?}", main_fb, swapchain_texture);

            // Blit using a render pass (handles format conversion)
            if let Err(e) = self.blit_to_swapchain(main_fb, swapchain_texture) {
                log::error!("Failed to blit to swapchain: {}", e);
                // Continue anyway and try to present
            } else {
                log::info!("Blit completed successfully");
            }
        } else {
            log::warn!("No main framebuffer detected - nothing to present");
        }

        // Present the surface
        match self.context.inner().surface_present(surface.id()) {
            Ok(status) => {
                log::info!("Presented frame with status: {:?}", status);

                // Clear the cached texture - it's been presented
                *self.current_swapchain_texture.lock() = None;

                Ok(())
            }
            Err(e) => {
                log::error!("Failed to present frame: {:?}", e);

                // Clear the cached texture even on error
                *self.current_swapchain_texture.lock() = None;

                Err(BasaltError::Surface(format!("Failed to present: {:?}", e)))
            }
        }
    }

    /// Set vsync mode
    pub fn set_vsync(&self, enabled: bool) -> Result<()> {
        if let Some(_surface) = &self.surface {
            let present_mode = if enabled {
                wgt::PresentMode::Fifo
            } else {
                wgt::PresentMode::Immediate
            };
            log::debug!("Setting vsync: {} (mode: {:?})", enabled, present_mode);
        }
        Ok(())
    }

    /// Get implementation information
    pub fn get_implementation_info(&self) -> String {
        self.info.clone()
    }

    /// Get vendor name
    pub fn get_vendor(&self) -> String {
        "Unknown".to_string()
    }

    /// Get renderer name
    pub fn get_renderer(&self) -> String {
        "Basalt WebGPU Renderer".to_string()
    }

    /// Get driver version
    pub fn get_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    /// Get device limits
    pub fn get_limits(&self) -> &wgt::Limits {
        &self.limits
    }

    /// Get the device context
    pub fn get_context(&self) -> &Arc<BasaltContext> {
        &self.context
    }

    /// Get the device ID
    pub fn get_device_id(&self) -> id::DeviceId {
        self.device_id
    }

    /// Create a buffer
    pub fn create_buffer(&self, size: u64, usage: u32) -> Result<id::BufferId> {
        let mut wgpu_usage = self.map_buffer_usage(usage);

        // WebGPU has a 64KB limit for uniform buffers
        // For larger buffers with UNIFORM usage, also add STORAGE usage
        // so they can be bound as storage buffers at runtime
        const MAX_UNIFORM_BUFFER_SIZE: u64 = 65536;
        if size > MAX_UNIFORM_BUFFER_SIZE && wgpu_usage.contains(wgt::BufferUsages::UNIFORM) {
            wgpu_usage |= wgt::BufferUsages::STORAGE;
            log::debug!(
                "Buffer size {} exceeds uniform limit {}, adding STORAGE usage",
                size, MAX_UNIFORM_BUFFER_SIZE
            );
        }

        let desc = wgt::BufferDescriptor {
            label: Some(Cow::Borrowed("Basalt Buffer")),
            size,
            usage: wgpu_usage,
            mapped_at_creation: false,
        };

        let (buffer_id, error) = self
            .context
            .inner()
            .device_create_buffer(self.device_id, &desc, None);

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("{:?}", e)));
        }

        Ok(buffer_id)
    }

    /// Write data to a buffer
    pub fn write_buffer(&self, buffer_id: id::BufferId, offset: u64, data: &[u8]) -> Result<()> {
        self.context
            .inner()
            .queue_write_buffer(self.queue_id, buffer_id, offset, data)
            .map_err(|e| BasaltError::Wgpu(format!("{:?}", e)))?;

        Ok(())
    }

    /// Destroy a buffer
    pub fn destroy_buffer(&self, buffer_id: id::BufferId) {
        self.context.inner().buffer_drop(buffer_id);
    }

    /// Create a texture
    pub fn create_texture(
        &self,
        width: u32,
        height: u32,
        depth: u32,
        mip_levels: u32,
        format: u32,
        usage: u32,
    ) -> Result<id::TextureId> {
        let texture_format = self.map_texture_format(format)?;
        let texture_usage = self.map_texture_usage(usage);

        // Filter out STORAGE_BINDING for formats that don't support it
        // WebGPU only supports storage textures for certain formats (Rgba32Float, Rgba16Float, etc.)
        // NOT for:
        // - 8-bit color formats (Rgba8UnormSrgb, Bgra8UnormSrgb, Rg8Unorm, R8Unorm, etc.)
        // - Depth/stencil formats (Depth32Float, Depth24Plus, etc.)
        let filtered_usage = match texture_format {
            // 8-bit color formats
            wgt::TextureFormat::Rgba8UnormSrgb
            | wgt::TextureFormat::Bgra8UnormSrgb
            | wgt::TextureFormat::Rgba8Unorm
            | wgt::TextureFormat::Bgra8Unorm
            | wgt::TextureFormat::Rg8Unorm
            | wgt::TextureFormat::R8Unorm
            | wgt::TextureFormat::Rg8Snorm
            | wgt::TextureFormat::R8Snorm
            | wgt::TextureFormat::Rg8Uint
            | wgt::TextureFormat::R8Uint
            | wgt::TextureFormat::Rg8Sint
            | wgt::TextureFormat::R8Sint
            // Depth/stencil formats (none support storage binding)
            | wgt::TextureFormat::Depth24Plus
            | wgt::TextureFormat::Depth32Float
            | wgt::TextureFormat::Depth24PlusStencil8
            | wgt::TextureFormat::Stencil8
            | wgt::TextureFormat::Depth32FloatStencil8 => {
                texture_usage - wgt::TextureUsages::STORAGE_BINDING
            }
            _ => texture_usage,
        };

        // Calculate maximum allowed mip levels for this texture size
        // Max mip levels = floor(log2(max(width, height))) + 1
        let max_dimension = width.max(height);
        let max_mip_levels = (max_dimension as f32).log2().floor() as u32 + 1;
        
        // Clamp requested mip levels to the valid range
        let actual_mip_levels = if mip_levels > max_mip_levels {
            log::debug!(
                "Clamping mip levels from {} to {} for {}x{} texture",
                mip_levels, max_mip_levels, width, height
            );
            max_mip_levels
        } else if mip_levels == 0 {
            1 // Minimum 1 mip level
        } else {
            mip_levels
        };

        let extent = wgt::Extent3d {
            width,
            height,
            depth_or_array_layers: depth,
        };

        let desc = wgt::TextureDescriptor {
            label: Some(Cow::Borrowed("Basalt Texture")),
            size: extent,
            mip_level_count: actual_mip_levels,
            sample_count: 1,
            dimension: wgt::TextureDimension::D2,
            format: texture_format,
            usage: filtered_usage,
            view_formats: vec![],
        };

        let (texture_id, error) = self
            .context
            .inner()
            .device_create_texture(self.device_id, &desc, None);

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("{:?}", e)));
        }

        // Detect if this is likely the main framebuffer (matches swapchain size + has RENDER_ATTACHMENT)
        if width == self.swapchain_width && height == self.swapchain_height
            && filtered_usage.contains(wgt::TextureUsages::RENDER_ATTACHMENT) {
            log::info!("Detected main framebuffer: {:?} ({}x{})", texture_id, width, height);
            *self.main_framebuffer.lock() = Some(texture_id);
        }

        Ok(texture_id)
    }

    /// Destroy a texture
    pub fn destroy_texture(&self, texture_id: id::TextureId) {
        self.context.inner().texture_drop(texture_id);
    }

    /// Create a texture view, returns (view_id, dimension)
    /// array_layers is used to determine if this is a D2 or D2Array texture
    pub fn create_texture_view(
        &self,
        texture_id: id::TextureId,
        array_layers: u32,
    ) -> Result<(id::TextureViewId, wgt::TextureViewDimension)> {
        // Determine the view dimension based on array layers
        // - 1 layer = D2 (regular 2D texture)
        // - 6 layers = Cube (cubemap) - but could also be D2Array, Minecraft uses D2Array for cubemaps
        // - >1 layers = D2Array
        let view_dimension = if array_layers > 1 {
            wgt::TextureViewDimension::D2Array
        } else {
            wgt::TextureViewDimension::D2
        };
        
        let desc = wgpu_core::resource::TextureViewDescriptor {
            dimension: Some(view_dimension),
            ..Default::default()
        };
        
        let (view_id, error) = self
            .context
            .inner()
            .texture_create_view(texture_id, &desc, None);

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("{:?}", e)));
        }

        log::debug!(
            "Created texture view for texture {:?} with {} layers -> dimension {:?}",
            texture_id, array_layers, view_dimension
        );
        
        Ok((view_id, view_dimension))
    }

    /// Create a sampler
    pub fn create_sampler(
        &self,
        address_mode_u: u32,
        address_mode_v: u32,
        address_mode_w: u32,
        min_filter: u32,
        mag_filter: u32,
        mipmap_filter: u32,
        lod_min_clamp: f32,
        lod_max_clamp: f32,
        max_anisotropy: u32,
    ) -> Result<id::SamplerId> {
        let desc = wgpu_core::resource::SamplerDescriptor {
            label: Some(Cow::Borrowed("Basalt Sampler")),
            address_modes: [
                self.map_address_mode(address_mode_u)?,
                self.map_address_mode(address_mode_v)?,
                self.map_address_mode(address_mode_w)?,
            ],
            mag_filter: self.map_filter_mode(mag_filter)?,
            min_filter: self.map_filter_mode(min_filter)?,
            mipmap_filter: self.map_mipmap_filter(mipmap_filter)?,
            lod_min_clamp,
            lod_max_clamp,
            compare: None,
            anisotropy_clamp: max_anisotropy.min(16) as u16,
            border_color: None,
        };

        let (sampler_id, error) = self
            .context
            .inner()
            .device_create_sampler(self.device_id, &desc, None);

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("{:?}", e)));
        }

        Ok(sampler_id)
    }

    /// Write data to texture using queue
    pub fn write_texture(
        &self,
        texture_id: id::TextureId,
        data: &[u8],
        mip_level: u32,
        origin_x: u32,
        origin_y: u32,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let texture_copy = wgt::TexelCopyTextureInfo {
            texture: texture_id,
            mip_level,
            origin: wgt::Origin3d {
                x: origin_x,
                y: origin_y,
                z: 0,
            },
            aspect: wgt::TextureAspect::All,
        };

        let data_layout = wgt::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4), // Assuming RGBA8
            rows_per_image: Some(height),
        };

        let size = wgt::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        self.context
            .inner()
            .queue_write_texture(self.queue_id, &texture_copy, data, &data_layout, &size)
            .map_err(|e| BasaltError::Wgpu(format!("{:?}", e)))?;

        Ok(())
    }

    /// Copy buffer to buffer
    pub fn copy_buffer_to_buffer(
        &self,
        src_buffer: id::BufferId,
        src_offset: u64,
        dst_buffer: id::BufferId,
        dst_offset: u64,
        size: u64,
    ) -> Result<()> {
        // Create a command encoder for the copy operation
        let encoder_desc = wgt::CommandEncoderDescriptor {
            label: Some(Cow::Borrowed("Copy Command Encoder")),
        };

        let (encoder_id, error) = self
            .context
            .inner()
            .device_create_command_encoder(self.device_id, &encoder_desc, None);

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("{:?}", e)));
        }

        // Record copy command
        if let Err(e) = self.context.inner().command_encoder_copy_buffer_to_buffer(
            encoder_id,
            src_buffer,
            src_offset,
            dst_buffer,
            dst_offset,
            Some(size),
        ) {
            return Err(BasaltError::Wgpu(format!("{:?}", e)));
        }

        // Finish and submit
        let (command_buffer, error) = self.context.inner().command_encoder_finish(
            encoder_id,
            &wgt::CommandBufferDescriptor::default(),
            None,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("{:?}", e)));
        }

        self.context
            .inner()
            .queue_submit(self.queue_id, &[command_buffer])
            .map_err(|e| BasaltError::Wgpu(format!("{:?}", e)))?;

        Ok(())
    }

    /// Copy texture to buffer (readback)
    pub fn copy_texture_to_buffer(
        &self,
        texture_id: id::TextureId,
        buffer_id: id::BufferId,
        buffer_offset: u64,
        mip_level: u32,
        width: u32,
        height: u32,
    ) -> Result<()> {
        // Create a command encoder for the copy operation
        let encoder_desc = wgt::CommandEncoderDescriptor {
            label: Some(Cow::Borrowed("Readback Command Encoder")),
        };

        let (encoder_id, error) = self
            .context
            .inner()
            .device_create_command_encoder(self.device_id, &encoder_desc, None);

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("{:?}", e)));
        }

        let texture_copy = wgt::TexelCopyTextureInfo {
            texture: texture_id,
            mip_level,
            origin: wgt::Origin3d::ZERO,
            aspect: wgt::TextureAspect::All,
        };

        let bytes_per_row = width * 4; // Assuming RGBA8
        let buffer_copy = wgt::TexelCopyBufferInfo {
            buffer: buffer_id,
            layout: wgt::TexelCopyBufferLayout {
                offset: buffer_offset,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height),
            },
        };

        let size = wgt::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        // Record copy command
        if let Err(e) = self.context.inner().command_encoder_copy_texture_to_buffer(
            encoder_id,
            &texture_copy,
            &buffer_copy,
            &size,
        ) {
            return Err(BasaltError::Wgpu(format!("{:?}", e)));
        }

        // Finish and submit
        let (command_buffer, error) = self.context.inner().command_encoder_finish(
            encoder_id,
            &wgt::CommandBufferDescriptor::default(),
            None,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("{:?}", e)));
        }

        self.context
            .inner()
            .queue_submit(self.queue_id, &[command_buffer])
            .map_err(|e| BasaltError::Wgpu(format!("{:?}", e)))?;

        Ok(())
    }

    /// Clear a texture with color and/or depth values
    pub fn clear_texture(
        &self,
        texture_id: id::TextureId,
        clear_color: Option<wgt::Color>,
        clear_depth: Option<f32>,
    ) -> Result<()> {
        // Create command encoder
        let encoder_desc = wgt::CommandEncoderDescriptor {
            label: Some(Cow::Borrowed("Clear Command Encoder")),
        };

        let (encoder_id, error) = self
            .context
            .inner()
            .device_create_command_encoder(self.device_id, &encoder_desc, None);

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("{:?}", e)));
        }

        // Create a texture view for the whole texture
        // In wgpu-core 27, texture view descriptor uses ImageSubresourceRange
        let view_desc = wgpu_core::resource::TextureViewDescriptor {
            label: Some(Cow::Borrowed("Clear Texture View")),
            format: None,
            dimension: None,
            usage: Some(wgt::TextureUsages::RENDER_ATTACHMENT),
            range: wgt::ImageSubresourceRange {
                aspect: wgt::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            },
        };

        let (view_id, error) = self.context.inner().texture_create_view(
            texture_id,
            &view_desc,
            None,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("{:?}", e)));
        }

        // Create a render pass that clears the texture
        let mut color_attachments = Vec::new();
        if clear_color.is_some() {
            color_attachments.push(Some(wgpu_core::command::RenderPassColorAttachment {
                view: view_id,
                resolve_target: None,
                load_op: wgpu_core::command::LoadOp::Clear(clear_color.unwrap()),
                store_op: wgpu_core::command::StoreOp::Store,
                depth_slice: None,
            }));
        }

        let depth_stencil_attachment = clear_depth.map(|depth| {
            wgpu_core::command::RenderPassDepthStencilAttachment {
                view: view_id,
                depth: wgpu_core::command::PassChannel {
                    load_op: Some(wgpu_core::command::LoadOp::Clear(Some(depth))),
                    store_op: Some(wgpu_core::command::StoreOp::Store),
                    read_only: false,
                },
                stencil: wgpu_core::command::PassChannel {
                    load_op: Some(wgpu_core::command::LoadOp::Clear(Some(0))),
                    store_op: Some(wgpu_core::command::StoreOp::Store),
                    read_only: false,
                },
            }
        });

        let pass_desc = wgpu_core::command::RenderPassDescriptor {
            label: Some(Cow::Borrowed("Clear Render Pass")),
            color_attachments: Cow::Borrowed(&color_attachments),
            depth_stencil_attachment: depth_stencil_attachment.as_ref(),
            timestamp_writes: None,
            occlusion_query_set: None,
        };

        // Begin and immediately end the render pass (clears happen on load)
        let (mut render_pass, error) = self.context.inner().command_encoder_begin_render_pass(
            encoder_id,
            &pass_desc,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("{:?}", e)));
        }

        if let Err(e) = self.context.inner().render_pass_end(&mut render_pass) {
            return Err(BasaltError::Wgpu(format!("{:?}", e)));
        }

        // Finish and submit
        let (command_buffer, error) = self.context.inner().command_encoder_finish(
            encoder_id,
            &wgt::CommandBufferDescriptor::default(),
            None,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("{:?}", e)));
        }

        self.context
            .inner()
            .queue_submit(self.queue_id, &[command_buffer])
            .map_err(|e| BasaltError::Wgpu(format!("{:?}", e)))?;

        Ok(())
    }

    /// Copy texture to texture
    pub fn copy_texture_to_texture(
        &self,
        src_texture: id::TextureId,
        dst_texture: id::TextureId,
        mip_level: u32,
        dest_x: u32,
        dest_y: u32,
        source_x: u32,
        source_y: u32,
        width: u32,
        height: u32,
    ) -> Result<()> {
        // Create command encoder
        let encoder_desc = wgt::CommandEncoderDescriptor {
            label: Some(Cow::Borrowed("Texture Copy Command Encoder")),
        };

        let (encoder_id, error) = self
            .context
            .inner()
            .device_create_command_encoder(self.device_id, &encoder_desc, None);

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("{:?}", e)));
        }

        let src_copy = wgt::TexelCopyTextureInfo {
            texture: src_texture,
            mip_level,
            origin: wgt::Origin3d {
                x: source_x,
                y: source_y,
                z: 0,
            },
            aspect: wgt::TextureAspect::All,
        };

        let dst_copy = wgt::TexelCopyTextureInfo {
            texture: dst_texture,
            mip_level,
            origin: wgt::Origin3d {
                x: dest_x,
                y: dest_y,
                z: 0,
            },
            aspect: wgt::TextureAspect::All,
        };

        let size = wgt::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        // Record copy command
        if let Err(e) = self.context.inner().command_encoder_copy_texture_to_texture(
            encoder_id,
            &src_copy,
            &dst_copy,
            &size,
        ) {
            return Err(BasaltError::Wgpu(format!("{:?}", e)));
        }

        // Finish and submit
        let (command_buffer, error) = self.context.inner().command_encoder_finish(
            encoder_id,
            &wgt::CommandBufferDescriptor::default(),
            None,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("{:?}", e)));
        }

        self.context
            .inner()
            .queue_submit(self.queue_id, &[command_buffer])
            .map_err(|e| BasaltError::Wgpu(format!("{:?}", e)))?;

        Ok(())
    }

    /// Create a render pipeline
    pub fn create_render_pipeline(&self, desc: RenderPipelineDescriptor) -> Result<id::RenderPipelineId> {
        // Parse WGSL shaders
        let vertex_module = self.parse_wgsl(&desc.vertex_shader)?;
        let fragment_module = if let Some(fs) = &desc.fragment_shader {
            Some(self.parse_wgsl(fs)?)
        } else {
            None
        };

        // Create shader modules
        let vs_desc = pipeline::ShaderModuleDescriptor {
            label: Some(Cow::Borrowed("Vertex Shader")),
            runtime_checks: wgt::ShaderRuntimeChecks::default(),
        };

        // Shader module source would be created from the validated module
        // For now, skip the complex shader module creation
        let _ = vertex_module; // Mark as used

        // Simplified - full implementation needs proper shader module creation
        // For now, return a placeholder error
        Err(BasaltError::ShaderCompilation("Pipeline creation requires full wgpu-core 27 implementation".into()))
    }

    /// Begin a render pass - simplified stub
    pub fn begin_render_pass(
        &self,
        _color_view: Option<id::TextureViewId>,
        _depth_view: Option<id::TextureViewId>,
        _clear_color: u32,
        _clear_depth: f32,
        _clear_stencil: u32,
        _width: u32,
        _height: u32,
    ) -> Result<id::CommandEncoderId> {
        // Simplified stub - full implementation needs proper render pass setup
        Err(BasaltError::Generic("Render pass creation requires full wgpu-core 27 implementation".into()))
    }

    /// Set pipeline for render pass - stub
    pub fn set_pipeline(
        &self,
        _encoder_id: id::CommandEncoderId,
        _pipeline_id: id::RenderPipelineId,
    ) -> Result<()> {
        Ok(())
    }

    /// Set vertex buffer - stub
    pub fn set_vertex_buffer(
        &self,
        _encoder_id: id::CommandEncoderId,
        _slot: u32,
        _buffer_id: id::BufferId,
        _offset: u64,
    ) -> Result<()> {
        Ok(())
    }

    /// Set index buffer - stub
    pub fn set_index_buffer(
        &self,
        _encoder_id: id::CommandEncoderId,
        _buffer_id: id::BufferId,
        _index_type: u32,
        _offset: u64,
    ) -> Result<()> {
        Ok(())
    }

    /// Draw indexed - stub
    pub fn draw_indexed(
        &self,
        _encoder_id: id::CommandEncoderId,
        _index_count: u32,
        _instance_count: u32,
        _first_index: u32,
        _base_vertex: i32,
        _first_instance: u32,
    ) -> Result<()> {
        Ok(())
    }

    /// End render pass and submit - stub
    pub fn end_render_pass(&self, _encoder_id: id::CommandEncoderId) -> Result<()> {
        Ok(())
    }

    // Helper functions for type mapping

    fn map_buffer_usage(&self, usage: u32) -> wgt::BufferUsages {
        let mut result = wgt::BufferUsages::empty();

        const COPY_SRC: u32 = 1 << 0;
        const COPY_DST: u32 = 1 << 1;
        const VERTEX: u32 = 1 << 2;
        const INDEX: u32 = 1 << 3;
        const UNIFORM: u32 = 1 << 4;
        const STORAGE: u32 = 1 << 5;
        const INDIRECT: u32 = 1 << 6;

        if usage & COPY_SRC != 0 {
            result |= wgt::BufferUsages::COPY_SRC;
        }
        if usage & COPY_DST != 0 {
            result |= wgt::BufferUsages::COPY_DST;
        }
        if usage & VERTEX != 0 {
            result |= wgt::BufferUsages::VERTEX;
        }
        if usage & INDEX != 0 {
            result |= wgt::BufferUsages::INDEX;
        }
        if usage & UNIFORM != 0 {
            result |= wgt::BufferUsages::UNIFORM;
        }
        if usage & STORAGE != 0 {
            result |= wgt::BufferUsages::STORAGE;
        }
        if usage & INDIRECT != 0 {
            result |= wgt::BufferUsages::INDIRECT;
        }

        result
    }

    fn map_texture_usage(&self, usage: u32) -> wgt::TextureUsages {
        let mut result = wgt::TextureUsages::empty();

        const COPY_SRC: u32 = 1 << 0;
        const COPY_DST: u32 = 1 << 1;
        const TEXTURE_BINDING: u32 = 1 << 2;
        const STORAGE_BINDING: u32 = 1 << 3;
        const RENDER_ATTACHMENT: u32 = 1 << 4;

        if usage & COPY_SRC != 0 {
            result |= wgt::TextureUsages::COPY_SRC;
        }
        if usage & COPY_DST != 0 {
            result |= wgt::TextureUsages::COPY_DST;
        }
        if usage & TEXTURE_BINDING != 0 {
            result |= wgt::TextureUsages::TEXTURE_BINDING;
        }
        if usage & STORAGE_BINDING != 0 {
            result |= wgt::TextureUsages::STORAGE_BINDING;
        }
        if usage & RENDER_ATTACHMENT != 0 {
            result |= wgt::TextureUsages::RENDER_ATTACHMENT;
        }

        result
    }

    fn map_texture_format(&self, format: u32) -> Result<wgt::TextureFormat> {
        const RGBA8: u32 = 0;
        const BGRA8: u32 = 1;
        const RGB8: u32 = 2;
        const RG8: u32 = 3;
        const R8: u32 = 4;
        const RGBA16F: u32 = 5;
        const RGBA32F: u32 = 6;
        const DEPTH24: u32 = 7;
        const DEPTH32F: u32 = 8;
        const DEPTH24_STENCIL8: u32 = 9;

        Ok(match format {
            RGBA8 => wgt::TextureFormat::Rgba8UnormSrgb,
            BGRA8 => wgt::TextureFormat::Bgra8UnormSrgb,
            RGB8 => wgt::TextureFormat::Rgba8UnormSrgb,
            RG8 => wgt::TextureFormat::Rg8Unorm,
            R8 => wgt::TextureFormat::R8Unorm,
            RGBA16F => wgt::TextureFormat::Rgba16Float,
            RGBA32F => wgt::TextureFormat::Rgba32Float,
            DEPTH24 => wgt::TextureFormat::Depth24Plus,
            DEPTH32F => wgt::TextureFormat::Depth32Float,
            DEPTH24_STENCIL8 => wgt::TextureFormat::Depth24PlusStencil8,
            _ => return Err(BasaltError::InvalidParameter(format!("Unknown texture format: {}", format))),
        })
    }

    fn map_address_mode(&self, mode: u32) -> Result<wgt::AddressMode> {
        Ok(match mode {
            0 => wgt::AddressMode::Repeat,
            1 => wgt::AddressMode::MirrorRepeat,
            2 => wgt::AddressMode::ClampToEdge,
            3 => wgt::AddressMode::ClampToBorder,
            _ => return Err(BasaltError::InvalidParameter(format!("Unknown address mode: {}", mode))),
        })
    }

    fn map_filter_mode(&self, mode: u32) -> Result<wgt::FilterMode> {
        Ok(match mode {
            0 => wgt::FilterMode::Nearest,
            1 => wgt::FilterMode::Linear,
            _ => return Err(BasaltError::InvalidParameter(format!("Unknown filter mode: {}", mode))),
        })
    }

    fn map_mipmap_filter(&self, mode: u32) -> Result<wgt::FilterMode> {
        Ok(match mode {
            0 => wgt::FilterMode::Nearest,
            1 => wgt::FilterMode::Linear,
            _ => return Err(BasaltError::InvalidParameter(format!("Unknown mipmap filter: {}", mode))),
        })
    }

    pub fn map_blend_factor(&self, factor: u32) -> Result<wgt::BlendFactor> {
        Ok(match factor {
            0 => wgt::BlendFactor::Zero,
            1 => wgt::BlendFactor::One,
            2 => wgt::BlendFactor::Src,
            3 => wgt::BlendFactor::OneMinusSrc,
            4 => wgt::BlendFactor::Dst,
            5 => wgt::BlendFactor::OneMinusDst,
            6 => wgt::BlendFactor::SrcAlpha,
            7 => wgt::BlendFactor::OneMinusSrcAlpha,
            8 => wgt::BlendFactor::DstAlpha,
            9 => wgt::BlendFactor::OneMinusDstAlpha,
            _ => return Err(BasaltError::InvalidParameter(format!("Unknown blend factor: {}", factor))),
        })
    }

    pub fn map_compare_function(&self, func: u32) -> Result<wgt::CompareFunction> {
        Ok(match func {
            0 => wgt::CompareFunction::Never,
            1 => wgt::CompareFunction::Less,
            2 => wgt::CompareFunction::Equal,
            3 => wgt::CompareFunction::LessEqual,
            4 => wgt::CompareFunction::Greater,
            5 => wgt::CompareFunction::NotEqual,
            6 => wgt::CompareFunction::GreaterEqual,
            7 => wgt::CompareFunction::Always,
            _ => return Err(BasaltError::InvalidParameter(format!("Unknown compare function: {}", func))),
        })
    }

    pub fn map_primitive_topology(&self, topology: u32) -> Result<wgt::PrimitiveTopology> {
        Ok(match topology {
            0 => wgt::PrimitiveTopology::PointList,
            1 => wgt::PrimitiveTopology::LineList,
            2 => wgt::PrimitiveTopology::LineStrip,
            3 => wgt::PrimitiveTopology::TriangleList,
            4 => wgt::PrimitiveTopology::TriangleStrip,
            _ => return Err(BasaltError::InvalidParameter(format!("Unknown topology: {}", topology))),
        })
    }

    fn parse_wgsl(&self, wgsl: &str) -> Result<naga::Module> {
        naga::front::wgsl::parse_str(&wgsl).map_err(|e| BasaltError::ShaderCompilation(format!("WGSL parse error: {:?}", e)))
    }
}

/// Helper function to create a device from a GLFW window handle
pub fn create_device_from_window(
    context: Arc<BasaltContext>,
    window_ptr: u64,
    display_ptr: u64,
    _width: u32,
    _height: u32,
) -> Result<BasaltDevice> {
    use raw_window_handle::{RawWindowHandle, RawDisplayHandle};

    // Create raw window and display handles from the GLFW window pointer
    #[cfg(target_os = "linux")]
    let (raw_window_handle, raw_display_handle) = {
        use std::ptr::NonNull;
        use raw_window_handle::{XlibWindowHandle, XlibDisplayHandle};

        if display_ptr != 0 {
            // We have a valid display pointer - use X11
            let window_handle = XlibWindowHandle::new(window_ptr);
            let display_handle = XlibDisplayHandle::new(
                Some(NonNull::new(display_ptr as *mut _)
                    .ok_or_else(|| BasaltError::Surface("Invalid X11 display handle".into()))?),
                0  // screen number - 0 is the default screen
            );

            log::info!("Using X11 window system (display: {:p}, window: {:x})", display_ptr as *const (), window_ptr);
            (RawWindowHandle::Xlib(window_handle), RawDisplayHandle::Xlib(display_handle))
        } else {
            // No display handle available - cannot create surface
            return Err(BasaltError::Surface(
                "No valid display handle - GLFW must provide either X11 or Wayland handles".into()
            ));
        }
    };

    #[cfg(target_os = "windows")]
    let (raw_window_handle, raw_display_handle) = {
        use raw_window_handle::{Win32WindowHandle, WindowsDisplayHandle};
        use std::num::NonZeroIsize;

        let window_handle = Win32WindowHandle::new(NonZeroIsize::new(window_ptr as isize).unwrap());
        let display_handle = WindowsDisplayHandle::new();

        (RawWindowHandle::Win32(window_handle), RawDisplayHandle::Windows(display_handle))
    };

    #[cfg(target_os = "macos")]
    let (raw_window_handle, raw_display_handle) = {
        use raw_window_handle::{AppKitWindowHandle, AppKitDisplayHandle};
        use std::ptr::NonNull;

        let window_handle = AppKitWindowHandle::new(NonNull::new(window_ptr as *mut _).unwrap());
        let display_handle = AppKitDisplayHandle::new();

        (RawWindowHandle::AppKit(window_handle), RawDisplayHandle::AppKit(display_handle))
    };

    // Create surface from window handles
    // On macOS, Metal surface creation MUST happen on the main thread
    #[cfg(target_os = "macos")]
    let surface_id = {
        use std::sync::Mutex as StdMutex;
        use std::sync::Arc as StdArc;
        
        log::info!("macOS: Starting surface creation flow");
        
        // Check if we're already on the main thread to avoid deadlock
        let is_main_thread = unsafe {
            use objc2::{msg_send, ClassType};
            use objc2::runtime::NSObject;
            
            // Get NSThread class
            let nsthread_class: *const objc2::runtime::AnyClass = objc2::runtime::AnyClass::get("NSThread").unwrap();
            let is_main: bool = msg_send![nsthread_class, isMainThread];
            is_main
        };
        
        log::info!("macOS: Is main thread: {}", is_main_thread);
        
        if is_main_thread {
            // Already on main thread, execute directly
            log::info!("macOS: Already on main thread, executing surface creation directly");
            
            use raw_window_handle::{AppKitWindowHandle, AppKitDisplayHandle, RawWindowHandle, RawDisplayHandle};
            use std::ptr::NonNull;
            
            // CRITICAL: glfwGetCocoaWindow returns NSWindow*, but wgpu needs NSView*
            let ns_view = unsafe {
                use objc2::runtime::AnyObject;
                use objc2::msg_send;
                
                let ns_window = window_ptr as *mut AnyObject;
                let content_view: *mut AnyObject = msg_send![ns_window, contentView];
                content_view as *mut std::ffi::c_void
            };
            
            log::info!("macOS: Got NSWindow at {:p}, contentView at {:p}", 
                      window_ptr as *const (), ns_view);
            
            let window_handle = AppKitWindowHandle::new(NonNull::new(ns_view as *mut _).unwrap());
            let display_handle = AppKitDisplayHandle::new();
            let raw_window_handle = RawWindowHandle::AppKit(window_handle);
            let raw_display_handle = RawDisplayHandle::AppKit(display_handle);
            
            let surface_result = unsafe {
                context.inner().instance_create_surface(
                    raw_display_handle,
                    raw_window_handle,
                    None,
                )
            };
            
            surface_result.map_err(|e| BasaltError::Surface(format!("Failed to create surface: {:?}", e)))?
        } else {
            // On a background thread, dispatch to main
            log::info!("macOS: On background thread, dispatching to main queue");
            
            let result: StdArc<StdMutex<Option<std::result::Result<id::SurfaceId, wgpu_core::instance::CreateSurfaceError>>>> = 
                StdArc::new(StdMutex::new(None));
            
            let result_clone = StdArc::clone(&result);
            let context_clone = context.clone();
            let window_ptr_copy = window_ptr;
            
            dispatch::Queue::main().exec_sync(move || {
                log::info!("macOS: Inside GCD main queue block");
                
                use raw_window_handle::{AppKitWindowHandle, AppKitDisplayHandle, RawWindowHandle, RawDisplayHandle};
                use std::ptr::NonNull;
                
                let ns_view = unsafe {
                    use objc2::runtime::AnyObject;
                    use objc2::msg_send;
                    
                    let ns_window = window_ptr_copy as *mut AnyObject;
                    let content_view: *mut AnyObject = msg_send![ns_window, contentView];
                    content_view as *mut std::ffi::c_void
                };
                
                log::info!("macOS: Got NSWindow at {:p}, contentView at {:p}", 
                          window_ptr_copy as *const (), ns_view);
                
                let window_handle = AppKitWindowHandle::new(NonNull::new(ns_view as *mut _).unwrap());
                let display_handle = AppKitDisplayHandle::new();
                let raw_window_handle = RawWindowHandle::AppKit(window_handle);
                let raw_display_handle = RawDisplayHandle::AppKit(display_handle);
                
                let surface_result = unsafe {
                    context_clone.inner().instance_create_surface(
                        raw_display_handle,
                        raw_window_handle,
                        None,
                    )
                };
                
                *result_clone.lock().unwrap() = Some(surface_result);
            });
            
            let surface_id = result.lock()
                .unwrap()
                .take()
                .unwrap()
                .map_err(|e| BasaltError::Surface(format!("Failed to create surface: {:?}", e)))?;
            surface_id
        }
    };
    
    // On other platforms, create surface directly
    #[cfg(not(target_os = "macos"))]
    let surface_id = unsafe {
        context.inner().instance_create_surface(
            raw_display_handle,
            raw_window_handle,
            None,
        )
    }.map_err(|e| BasaltError::Surface(format!("Failed to create surface: {:?}", e)))?;

    // Request adapter compatible with the surface
    let adapter_opts = wgpu_core::instance::RequestAdapterOptions {
        power_preference: wgt::PowerPreference::HighPerformance,
        compatible_surface: Some(surface_id),
        force_fallback_adapter: false,
    };

    let adapter_id = context
        .inner()
        .request_adapter(&adapter_opts, wgt::Backends::all(), None)
        .map_err(|e| BasaltError::Device(format!("Failed to find adapter: {:?}", e)))?;

    // Request device
    let device_desc = wgt::DeviceDescriptor::default();

    let (device_id, queue_id) = context
        .inner()
        .adapter_request_device(adapter_id, &device_desc, None, None)
        .map_err(|e| BasaltError::Device(format!("Failed to create device: {:?}", e)))?;

    // Wrap surface in BasaltSurface
    let mut bassalt_surface = BasaltSurface::from_id(context.clone(), surface_id);

    // Query surface capabilities to find the best format
    let surface_caps = context
        .inner()
        .surface_get_capabilities(surface_id, adapter_id)
        .map_err(|e| BasaltError::Surface(format!("Failed to get surface capabilities: {:?}", e)))?;

    // Try to find Rgba8UnormSrgb (matches Minecraft's framebuffer format) to avoid conversion
    // Fall back to Bgra8UnormSrgb or the first supported format
    let surface_format = surface_caps
        .formats
        .iter()
        .copied()
        .find(|f| matches!(f, wgt::TextureFormat::Rgba8UnormSrgb))
        .or_else(|| {
            surface_caps
                .formats
                .iter()
                .copied()
                .find(|f| matches!(f, wgt::TextureFormat::Bgra8UnormSrgb))
        })
        .unwrap_or(surface_caps.formats[0]);

    log::info!("Selected surface format: {:?} (available: {:?})", surface_format, surface_caps.formats);

    // Configure the surface
    let surface_config = wgt::SurfaceConfiguration {
        usage: wgt::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: _width,
        height: _height,
        present_mode: wgt::PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode: wgt::CompositeAlphaMode::Auto,
        view_formats: vec![],
    };

    bassalt_surface.configure(device_id, surface_config)?;

    BasaltDevice::new(context, device_id, queue_id, Some(bassalt_surface), _width, _height, surface_format)
}
