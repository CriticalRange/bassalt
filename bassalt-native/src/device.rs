//! GPU device wrapper - main interface for rendering operations

use std::sync::Arc;
use std::collections::HashMap;
use wgpu_core::id;
use wgpu_types as wgt;

use crate::context::BasaltContext;
use crate::surface::BasaltSurface;
use crate::buffer::BufferDescriptor;
use crate::texture::TextureDescriptor;
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
}

impl BasaltDevice {
    /// Create a new device
    pub fn new(
        context: Arc<BasaltContext>,
        device_id: id::DeviceId,
        queue_id: id::QueueId,
        surface: Option<BasaltSurface>,
    ) -> Result<Self> {
        let limits = context
            .inner()
            .device_get_limits(device_id);

        let info = format!(
            "Basalt Renderer (wgpu-core)\nAdapter: {}",
            // TODO: Get actual adapter info
            "Unknown"
        );

        Ok(Self {
            context,
            device_id,
            queue_id,
            surface,
            limits,
            info,
        })
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

    /// Present the current frame
    pub fn present_frame(&self) -> Result<()> {
        if let Some(surface) = &self.surface {
            surface.present(self.queue_id)?;
        }
        Ok(())
    }

    /// Set vsync mode
    pub fn set_vsync(&self, enabled: bool) -> Result<()> {
        if let Some(surface) = &self.surface {
            // Reconfigure surface with new present mode
            let present_mode = if enabled {
                wgt::PresentMode::Fifo
            } else {
                wgt::PresentMode::Immediate
            };

            // This is simplified - in practice we'd need to get the current config
            // and modify only the present mode
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
        // Get adapter info from context
        "Unknown".to_string() // TODO: Query from adapter
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

    /// Create a buffer
    pub fn create_buffer(&self, size: u64, usage: u32) -> Result<id::BufferId> {
        let wgpu_usage = self.map_buffer_usage(usage);

        let desc = wgt::BufferDescriptor {
            label: Some("Basalt Buffer"),
            size,
            usage: wgpu_usage,
            mapped_at_creation: false,
            memory_flags: wgt::MemoryFlags::empty(),
        };

        let (buffer_id, error) = self
            .context
            .inner()
            .device_create_buffer(self.device_id, &desc, None);

        buffer_id.ok_or_else(|| {
            error.map_or_else(
                || BasaltError::Wgpu("Unknown buffer error".into()),
                |e| BasaltError::Wgpu(format!("{:?}", e)),
            )
        })
    }

    /// Write data to a buffer
    pub fn write_buffer(&self, buffer_id: id::BufferId, offset: u64, data: &[u8]) -> Result<()> {
        // Create a staging buffer if needed, or use queue write
        self.context
            .inner()
            .queue_write_buffer(self.queue_id, buffer_id, offset, data)?;

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

        let extent = wgt::Extent3d {
            width,
            height,
            depth_or_array_layers: depth,
        };

        let desc = wgt::TextureDescriptor {
            label: Some("Basalt Texture"),
            size: extent,
            mip_level_count: mip_levels,
            sample_count: 1,
            dimension: wgt::TextureDimension::D2,
            format: texture_format,
            usage: texture_usage,
            view_formats: vec![],
            memory_flags: wgt::MemoryFlags::empty(),
        };

        let (texture_id, error) = self
            .context
            .inner()
            .device_create_texture(self.device_id, &desc, None);

        texture_id.ok_or_else(|| {
            error.map_or_else(
                || BasaltError::Wgpu("Unknown texture error".into()),
                |e| BasaltError::Wgpu(format!("{:?}", e)),
            )
        })
    }

    /// Destroy a texture
    pub fn destroy_texture(&self, texture_id: id::TextureId) {
        self.context.inner().texture_drop(texture_id);
    }

    /// Create a texture view
    pub fn create_texture_view(&self, texture_id: id::TextureId) -> Result<id::TextureViewId> {
        let desc = wgt::TextureViewDescriptor::default();
        let (view_id, error) = self
            .context
            .inner()
            .texture_create_view(texture_id, &desc);

        view_id.ok_or_else(|| {
            error.map_or_else(
                || BasaltError::Wgpu("Unknown texture view error".into()),
                |e| BasaltError::Wgpu(format!("{:?}", e)),
            )
        })
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
        let desc = wgt::SamplerDescriptor {
            label: Some("Basalt Sampler"),
            address_mode_u: self.map_address_mode(address_mode_u)?,
            address_mode_v: self.map_address_mode(address_mode_v)?,
            address_mode_w: self.map_address_mode(address_mode_w)?,
            mag_filter: self.map_filter_mode(mag_filter)?,
            min_filter: self.map_filter_mode(min_filter)?,
            mipmap_filter: self.map_mipmap_filter(mipmap_filter)?,
            lod_min_clamp,
            lod_max_clamp,
            compare: None,
            anisotropy_clamp: max_anisotropy,
            border_color: None,
        };

        let (sampler_id, error) = self
            .context
            .inner()
            .device_create_sampler(self.device_id, &desc);

        sampler_id.ok_or_else(|| {
            error.map_or_else(
                || BasaltError::Wgpu("Unknown sampler error".into()),
                |e| BasaltError::Wgpu(format!("{:?}", e)),
            )
        })
    }

    /// Create a render pipeline
    pub fn create_render_pipeline(&self, desc: RenderPipelineDescriptor) -> Result<id::RenderPipelineId> {
        use naga::{ShaderStage, Module};

        // Parse WGSL shaders
        let vertex_module = self.parse_wgsl(&desc.vertex_shader, ShaderStage::Vertex)?;
        let fragment_module = if let Some(fs) = &desc.fragment_shader {
            Some(self.parse_wgsl(fs, ShaderStage::Fragment)?)
        } else {
            None
        };

        // Create shader modules
        let vs_desc = wgt::ShaderModuleDescriptor {
            label: Some("Vertex Shader"),
            shader_bound_checks: wgt::ShaderBoundChecks::default(),
        };

        let (vs_module_id, vs_error) = self
            .context
            .inner()
            .device_create_shader_module(self.device_id, &vs_desc, &vertex_module);

        let vs_module_id = vs_module_id.ok_or_else(|| {
            BasaltError::ShaderCompilation(format!("Vertex shader error: {:?}", vs_error))
        })?;

        let fs_module_id = if let Some(ref fragment_module) = fragment_module {
            let fs_desc = wgt::ShaderModuleDescriptor {
                label: Some("Fragment Shader"),
                shader_bound_checks: wgt::ShaderBoundChecks::default(),
            };

            let (fs_id, fs_error) = self
                .context
                .inner()
                .device_create_shader_module(self.device_id, &fs_desc, fragment_module);

            Some(fs_id.ok_or_else(|| {
                BasaltError::ShaderCompilation(format!("Fragment shader error: {:?}", fs_error))
            })?)
        } else {
            None
        };

        // Build pipeline descriptor
        let mut pipeline_desc = wgt::RenderPipelineDescriptor::default();

        pipeline_desc.label = Some("Basalt Pipeline");
        pipeline_desc.layout = None; // Let wgpu derive it
        pipeline_desc.vertex.module = vs_module_id;
        pipeline_desc.vertex.entry_point = "main".to_string();
        // TODO: Set vertex buffers from vertex_format

        if let Some(fs_id) = fs_module_id {
            let mut fragment_state = wgt::FragmentState::default();
            fragment_state.module = fs_id;
            fragment_state.entry_point = "main".to_string();
            fragment_state.targets = vec![Some(wgt::ColorTargetState {
                format: wgt::TextureFormat::Bgra8UnormSrgb,
                blend: if desc.blend_enabled {
                    Some(wgt::BlendState {
                        color: wgt::BlendComponent {
                            src_factor: self.map_blend_factor(desc.blend_color_factor)?,
                            dst_factor: self.map_blend_factor(desc.blend_alpha_factor)?,
                            operation: wgt::BlendOperation::Add,
                        },
                        alpha: wgt::BlendComponent {
                            src_factor: wgt::BlendFactor::One,
                            dst_factor: wgt::BlendFactor::Zero,
                            operation: wgt::BlendOperation::Add,
                        },
                    })
                } else {
                    None
                },
                write_mask: wgt::ColorWrites::ALL,
            })];
            pipeline_desc.fragment = Some(fragment_state);
        }

        pipeline_desc.primitive.topology = self.map_primitive_topology(desc.primitive_topology)?;
        pipeline_desc.primitive.strip_index_format = None;
        pipeline_desc.primitive.front_face = wgt::FrontFace::Ccw;
        pipeline_desc.primitive.cull_mode = None;

        if desc.depth_test_enabled || desc.depth_write_enabled {
            let mut depth_stencil = wgt::DepthStencilState::default();
            depth_stencil.format = wgt::TextureFormat::Depth24PlusStencil8;
            depth_stencil.depth_write_enabled = desc.depth_write_enabled;
            depth_stencil.depth_compare = self.map_compare_function(desc.depth_compare)?;
            pipeline_desc.depth_stencil = Some(depth_stencil);
        }

        pipeline_desc.multisample.count = 1;
        pipeline_desc.multisample.mask = !0;
        pipeline_desc.multisample.alpha_to_coverage_enabled = false;

        let (pipeline_id, error) = self
            .context
            .inner()
            .device_create_render_pipeline(self.device_id, &pipeline_desc, None);

        pipeline_id.ok_or_else(|| {
            BasaltError::Wgpu(format!("Pipeline creation failed: {:?}", error))
        })
    }

    /// Begin a render pass
    pub fn begin_render_pass(
        &self,
        color_view: Option<id::TextureViewId>,
        depth_view: Option<id::TextureViewId>,
        clear_color: u32,
        clear_depth: f32,
        _clear_stencil: u32,
        width: u32,
        height: u32,
    ) -> Result<id::CommandEncoderId> {
        let encoder_desc = wgt::CommandEncoderDescriptor {
            label: Some("Render Pass Encoder"),
        };

        let (encoder_id, _) = self
            .context
            .inner()
            .device_create_command_encoder(self.device_id, &encoder_desc);

        let mut pass_desc = wgt::RenderPassDescriptor::default();

        let r = ((clear_color >> 16) & 0xFF) as f32 / 255.0;
        let g = ((clear_color >> 8) & 0xFF) as f32 / 255.0;
        let b = (clear_color & 0xFF) as f32 / 255.0;
        let a = ((clear_color >> 24) & 0xFF) as f32 / 255.0;

        if let Some(cv) = color_view {
            pass_desc.color_attachments = vec![Some(wgt::RenderPassColorAttachment {
                view: cv,
                resolve_target: None,
                load_op: wgt::LoadOp::Clear,
                store_op: wgt::StoreOp::Store,
                clear_value: wgt::Color {
                    r,
                    g,
                    b,
                    a,
                },
                read_only: false,
            })];
        }

        if let Some(dv) = depth_view {
            pass_desc.depth_stencil_attachment = Some(wgt::RenderPassDepthStencilAttachment {
                view: dv,
                depth_load_op: wgt::LoadOp::Clear,
                depth_store_op: wgt::StoreOp::Store,
                depth_clear_value: clear_depth,
                stencil_load_op: wgt::LoadOp::Clear,
                stencil_store_op: wgt::StoreOp::Store,
                stencil_clear_value: 0,
                depth_read_only: false,
                stencil_read_only: false,
            });
        }

        self.context
            .inner()
            .command_encoder_begin_render_pass(encoder_id, &pass_desc);

        Ok(encoder_id)
    }

    /// Set pipeline for render pass
    pub fn set_pipeline(
        &self,
        encoder_id: id::CommandEncoderId,
        pipeline_id: id::RenderPipelineId,
    ) -> Result<()> {
        self.context
            .inner()
            .command_encoder_set_render_pipeline(encoder_id, pipeline_id);
        Ok(())
    }

    /// Set vertex buffer
    pub fn set_vertex_buffer(
        &self,
        encoder_id: id::CommandEncoderId,
        slot: u32,
        buffer_id: id::BufferId,
        offset: u64,
    ) -> Result<()> {
        self.context
            .inner()
            .command_encoder_set_vertex_buffer(encoder_id, slot, buffer_id, offset);
        Ok(())
    }

    /// Set index buffer
    pub fn set_index_buffer(
        &self,
        encoder_id: id::CommandEncoderId,
        buffer_id: id::BufferId,
        index_type: u32,
        offset: u64,
    ) -> Result<()> {
        let format = if index_type == 0 {
            wgt::IndexFormat::Uint16
        } else {
            wgt::IndexFormat::Uint32
        };

        self.context
            .inner()
            .command_encoder_set_index_buffer(encoder_id, buffer_id, offset, format);
        Ok(())
    }

    /// Draw indexed
    pub fn draw_indexed(
        &self,
        encoder_id: id::CommandEncoderId,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        base_vertex: i32,
        first_instance: u32,
    ) -> Result<()> {
        self.context.inner().command_encoder_draw_indexed(
            encoder_id,
            index_count,
            instance_count,
            first_index,
            base_vertex,
            first_instance,
        );
        Ok(())
    }

    /// End render pass and submit
    pub fn end_render_pass(&self, encoder_id: id::CommandEncoderId) -> Result<()> {
        self.context.inner().command_encoder_end_render_pass(encoder_id);

        let (command_buffer_id, _) = self
            .context
            .inner()
            .command_encoder_finish(encoder_id, &wgt::CommandBufferDescriptor::default());

        self.context
            .inner()
            .queue_submit(self.queue_id, &[command_buffer_id]);

        self.context.inner().command_buffer_drop(command_buffer_id);
        self.context.inner().command_encoder_drop(encoder_id);

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
        // Format enum mapping
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
            RGB8 => wgt::TextureFormat::Rgba8UnormSrgb, // Convert
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
            1 => wgt::AddressMode::MirroredRepeat,
            2 => wgt::AddressMode::ClampToEdge,
            3 => wgt::AddressMode::ClampToBorderColor {
                color: wgt::SamplerBorderColor::TransparentBlack,
            },
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

    fn map_blend_factor(&self, factor: u32) -> Result<wgt::BlendFactor> {
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

    fn map_compare_function(&self, func: u32) -> Result<wgt::CompareFunction> {
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

    fn map_primitive_topology(&self, topology: u32) -> Result<wgt::PrimitiveTopology> {
        Ok(match topology {
            0 => wgt::PrimitiveTopology::PointList,
            1 => wgt::PrimitiveTopology::LineList,
            2 => wgt::PrimitiveTopology::LineStrip,
            3 => wgt::PrimitiveTopology::TriangleList,
            4 => wgt::PrimitiveTopology::TriangleStrip,
            _ => return Err(BasaltError::InvalidParameter(format!("Unknown topology: {}", topology))),
        })
    }

    fn parse_wgsl(&self, wgsl: &str, stage: naga::ShaderStage) -> Result<naga::Module> {
        use naga::front::wgsl::Parser;

        let parser = Parser::default();
        parser.parse(&wgsl).map_err(|e| BasaltError::ShaderCompilation(format!("WGSL parse error: {}", e)))
    }
}

/// Helper function to create a device from a GLFW window handle
pub fn create_device_from_window(
    context: Arc<BasaltContext>,
    window_ptr: u64,
    width: u32,
    height: u32,
) -> Result<BasaltDevice> {
    use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

    // Create a raw window handle from the GLFW window pointer
    // This is platform-specific and needs to be implemented properly
    #[cfg(target_os = "windows")]
    let raw_handle = {
        use raw_window_handle::Win32WindowHandle;
        RawWindowHandle::Win32(Win32WindowHandle {
            hwnd: window_ptr as *mut std::ffi::c_void,
            ..Win32WindowHandle::default()
        })
    };

    #[cfg(target_os = "linux")]
    let raw_handle = {
        use raw_window_handle::XlibWindowHandle;
        RawWindowHandle::Xlib(XlibWindowHandle {
            window: window_ptr as *mut std::ffi::c_void,
            ..XlibWindowHandle::default()
        })
    };

    #[cfg(target_os = "macos")]
    let raw_handle = {
        use raw_window_handle::AppKitWindowHandle;
        RawWindowHandle::AppKit(AppKitWindowHandle {
            ns_window: window_ptr as *mut std::ffi::c_void,
            ..AppKitWindowHandle::default()
        })
    };

    // Create surface
    let surface = BasaltSurface::from_raw_window_handle(context.clone(), raw_handle)?;

    // Request adapter
    let adapter_opts = wgt::RequestAdapterOptions {
        power_preference: wgt::PowerPreference::HighPerformance,
        compatible_surface: Some(surface.id()),
        force_fallback_adapter: false,
    };

    let adapter_id = context
        .inner()
        .request_adapter(&adapter_opts)
        .ok_or_else(|| BasaltError::Device("No suitable adapter found".into()))?;

    let adapter_info = context
        .inner()
        .adapter_get_info(adapter_id);

    // Request device
    let device_desc = wgt::DeviceDescriptor {
        label: Some("Basalt Device"),
        required_features: wgt::Features::empty(),
        required_limits: wgt::Limits::default(),
        memory_hints: wgt::MemoryHints::default(),
    };

    let (device_id, queue_id) = context
        .inner()
        .request_device(adapter_id, &device_desc)
        .map_err(|e| BasaltError::Device(format!("Failed to create device: {:?}", e)))?;

    // Configure surface
    let supported_formats = surface.get_supported_formats(adapter_id);
    let format = supported_formats
        .first()
        .copied()
        .unwrap_or(wgt::TextureFormat::Bgra8UnormSrgb);

    let config = wgt::SurfaceConfiguration {
        usage: wgt::TextureUsages::RENDER_ATTACHMENT,
        format,
        width,
        height,
        present_mode: wgt::PresentMode::Fifo,
        alpha_mode: wgt::CompositeAlphaMode::Opaque,
        view_formats: vec![],
    };

    let mut surface_obj = surface;
    surface_obj.configure(device_id, config)?;

    BasaltDevice::new(context, device_id, queue_id, Some(surface_obj))
}
