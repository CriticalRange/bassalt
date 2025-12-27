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
            .device_limits(device_id);

        let info = format!(
            "Basalt Renderer (wgpu-core)\nAdapter: {}",
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

    /// Create a buffer
    pub fn create_buffer(&self, size: u64, usage: u32) -> Result<id::BufferId> {
        let wgpu_usage = self.map_buffer_usage(usage);

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

        let extent = wgt::Extent3d {
            width,
            height,
            depth_or_array_layers: depth,
        };

        let desc = wgt::TextureDescriptor {
            label: Some(Cow::Borrowed("Basalt Texture")),
            size: extent,
            mip_level_count: mip_levels,
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

        Ok(texture_id)
    }

    /// Destroy a texture
    pub fn destroy_texture(&self, texture_id: id::TextureId) {
        self.context.inner().texture_drop(texture_id);
    }

    /// Create a texture view
    pub fn create_texture_view(&self, texture_id: id::TextureId) -> Result<id::TextureViewId> {
        let desc = wgpu_core::resource::TextureViewDescriptor::default();
        let (view_id, error) = self
            .context
            .inner()
            .texture_create_view(texture_id, &desc, None);

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("{:?}", e)));
        }

        Ok(view_id)
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
    width: u32,
    height: u32,
) -> Result<BasaltDevice> {
    use raw_window_handle::RawWindowHandle;

    // Create a raw window handle from the GLFW window pointer
    #[cfg(target_os = "linux")]
    let _raw_handle = {
        use raw_window_handle::XlibWindowHandle;
        let handle = XlibWindowHandle::new(window_ptr);
        raw_window_handle::RawWindowHandle::Xlib(handle)
    };

    #[cfg(target_os = "windows")]
    let raw_handle = {
        use raw_window_handle::Win32WindowHandle;
        use std::num::NonZeroIsize;
        let handle = Win32WindowHandle::new(NonZeroIsize::new(window_ptr as isize).unwrap());
        RawWindowHandle::Win32(handle)
    };

    #[cfg(target_os = "macos")]
    let raw_handle = {
        use raw_window_handle::AppKitWindowHandle;
        use std::ptr::NonNull;
        let handle = AppKitWindowHandle::new(NonNull::new(window_ptr as *mut _).unwrap());
        RawWindowHandle::AppKit(handle)
    };

    // For now, skip surface creation and create a headless device
    // Full surface support requires proper window handle integration
    
    // Request adapter
    let adapter_opts: wgt::RequestAdapterOptions<id::SurfaceId> = wgt::RequestAdapterOptions {
        power_preference: wgt::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    };

    let adapters = context
        .inner()
        .enumerate_adapters(wgt::Backends::all());
    
    let adapter_id = adapters
        .first()
        .copied()
        .ok_or_else(|| BasaltError::Device("No suitable adapter found".into()))?;

    // Request device
    let device_desc = wgt::DeviceDescriptor::default();

    let (device_id, queue_id) = context
        .inner()
        .adapter_request_device(adapter_id, &device_desc, None, None)
        .map_err(|e| BasaltError::Device(format!("Failed to create device: {:?}", e)))?;

    BasaltDevice::new(context, device_id, queue_id, None)
}
