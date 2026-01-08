//! GPU device wrapper - main interface for rendering operations

use std::borrow::Cow;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use wgpu_core::id;
use wgpu_types as wgt;

// wgpu 28.0: Import LoadOpDontCare token for DontCare load op
use wgt::LoadOpDontCare;

use crate::context::BasaltContext;
use crate::surface::BasaltSurface;
use crate::pipeline_registry::PipelineCache;
use crate::error::{BasaltError, Result};
use crate::bind_group_layouts::{BindGroupLayouts, SharedLayoutCache};

/// Current swapchain state (for lock-free updates)
#[derive(Debug, Clone)]
struct SwapchainState {
    main_framebuffer: Option<id::TextureId>,
    width: u32,
    height: u32,
}

/// Frame-in-flight tracking for proper frame synchronization
///
/// **CRITICAL FIX #2:** Actual GPU synchronization using device.poll()
///
/// Based on best practices from wgpu, Vulkan, and DirectX 12:
/// - Use device.poll() to check actual GPU completion (not just atomic counters)
/// - Track frames in flight with max limit for triple buffering
/// - Reset counter when queue is actually empty (not just decrementing)
/// - See checklist Steps 274, 281, 286-287 for GPU synchronization patterns
struct FrameTracker {
    /// Number of frames currently submitted to GPU
    frames_in_flight: AtomicUsize,
    /// Maximum frames allowed before waiting (triple buffering = 2, not 3!)
    max_frames_in_flight: usize,
}

impl FrameTracker {
    fn new() -> Self {
        Self {
            frames_in_flight: AtomicUsize::new(0),
            max_frames_in_flight: 2, // Triple buffering standard (2 frames in flight + 1 swapchain image)
        }
    }

    /// Increment frame counter (called when submitting work)
    ///
    /// Uses Release ordering to ensure all prior writes (commands, resources)
    /// are visible before the frame count increment.
    fn increment(&self) {
        let count = self.frames_in_flight.fetch_add(1, Ordering::Release) + 1;
        log::debug!("Frame tracker: {} frames in flight (max: {})", count, self.max_frames_in_flight);
    }

    /// Decrement frame counter (called when frame completes)
    ///
    /// Uses Release ordering to ensure completion updates are visible
    /// before decrementing the frame count.
    fn decrement(&self) {
        let count = self.frames_in_flight.fetch_sub(1, Ordering::Release) - 1;
        log::debug!("Frame tracker: {} frames in flight (max: {})", count, self.max_frames_in_flight);
    }

    /// **CRITICAL:** Reset frame counter to zero (called when GPU queue is actually empty)
    ///
    /// This is more accurate than decrement() because device.poll() can tell us
    /// when ALL work is done, not just one frame. This prevents counter drift.
    fn reset_if_empty(&self, is_empty: bool) {
        if is_empty {
            let prev_count = self.frames_in_flight.swap(0, Ordering::Release);
            if prev_count > 0 {
                log::debug!("Frame tracker: GPU queue empty, resetting {} -> 0", prev_count);
            }
        }
    }

    /// Get current frame count
    ///
    /// Uses Acquire ordering to see the most recent frame count updates.
    fn count(&self) -> usize {
        self.frames_in_flight.load(Ordering::Acquire)
    }

    /// Check if we need to wait before submitting more work
    ///
    /// Uses Acquire ordering to ensure we see the most recent frame count.
    fn should_wait(&self) -> bool {
        self.count() >= self.max_frames_in_flight
    }
}

/// Main device wrapper
pub struct BasaltDevice {
    context: Arc<BasaltContext>,
    device_id: id::DeviceId,
    adapter_id: id::AdapterId,
    queue_id: id::QueueId,
    surface: Option<BasaltSurface>,
    limits: wgt::Limits,
    info: String,
    // **CRITICAL FIX #3:** Proper mutex for swapchain state (not lock-free)
    // Lock-free ArcSwap doesn't provide ordering guarantees between:
    // - endRenderPass() writes (setting main framebuffer)
    // - present_frame() reads (getting main framebuffer)
    // Using Mutex ensures proper synchronization and prevents race conditions
    swapchain_state: parking_lot::Mutex<SwapchainState>,
    swapchain_format: wgt::TextureFormat,
    // Frame-in-flight tracking for synchronization
    frame_tracker: FrameTracker,
    // Cached blit pipeline for format conversion
    blit_bind_group_layout: parking_lot::Mutex<Option<id::BindGroupLayoutId>>,
    blit_pipeline: parking_lot::Mutex<Option<id::RenderPipelineId>>,
    // Shared bind group layout and pipeline layout for Minecraft rendering
    shared_bind_group_layout: id::BindGroupLayoutId,
    shared_pipeline_layout: id::PipelineLayoutId,
    // Pre-defined bind group layouts (wgpu-mc style)
    pub bind_group_layouts: BindGroupLayouts,
    // Depth texture cache by dimensions (width, height) -> (texture_id, view_id)
    depth_texture_cache: parking_lot::Mutex<std::collections::HashMap<(u32, u32), (id::TextureId, id::TextureViewId)>>,
    // Pipeline cache for fast shader compilation
    pub pipeline_cache: Arc<PipelineCache>,
    // Shared layout cache for deduplicating bind group layouts
    pub layout_cache: Arc<SharedLayoutCache>,
    // Track which textures have been rendered to (for automatic first-use clearing)
    pub(crate) initialized_textures: parking_lot::Mutex<std::collections::HashSet<id::TextureId>>,
}

impl BasaltDevice {
    /// Create a new device
    pub fn new(
        context: Arc<BasaltContext>,
        device_id: id::DeviceId,
        adapter_id: id::AdapterId,
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

        // Create pre-defined bind group layouts (wgpu-mc style)
        let bind_group_layouts = BindGroupLayouts::new(&context, device_id);

        // Create pipeline cache for fast shader compilation
        let pipeline_cache = Arc::new(PipelineCache::new());

        // Create layout cache for deduplicating bind group layouts
        let layout_cache = Arc::new(SharedLayoutCache::new());

        log::info!("Created shared pipeline layout for Minecraft rendering");
        log::info!("Initialized pipeline cache for shader compilation");
        log::info!("Initialized layout cache for bind group deduplication");

        // Initial swapchain state
        let initial_state = SwapchainState {
            main_framebuffer: None,
            width,
            height,
        };

        // Create frame tracker for triple buffering (2 frames max)
        let frame_tracker = FrameTracker::new();
        log::info!("Initialized frame tracker (max {} frames in flight for triple buffering)",
            frame_tracker.max_frames_in_flight);

        Ok(Self {
            context,
            device_id,
            adapter_id,
            queue_id,
            surface,
            limits,
            info,
            swapchain_state: parking_lot::Mutex::new(initial_state),
            swapchain_format,
            frame_tracker,
            blit_bind_group_layout: parking_lot::Mutex::new(None),
            blit_pipeline: parking_lot::Mutex::new(None),
            shared_bind_group_layout,
            shared_pipeline_layout,
            bind_group_layouts,
            depth_texture_cache: parking_lot::Mutex::new(std::collections::HashMap::new()),
            pipeline_cache,
            layout_cache,
            initialized_textures: parking_lot::Mutex::new(std::collections::HashSet::new()),
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
            return Err(BasaltError::device_creation(format!(
                "Failed to create shared bind group layout: {:?}",
                e
            )));
        }

        // Create pipeline layout from the bind group layout
        let pl_desc = wgpu_core::binding_model::PipelineLayoutDescriptor {
            label: Some(Cow::Borrowed("Bassalt Shared Pipeline Layout")),
            bind_group_layouts: Cow::Owned(vec![bgl_id]),
            // Immediates for per-draw data (128 bytes = 2 mat4x4)
            immediate_size: 128,
        };

        let (pl_id, pl_error) = global.device_create_pipeline_layout(device_id, &pl_desc, None);

        if let Some(e) = pl_error {
            return Err(BasaltError::device_creation(format!(
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

    /// Get the adapter ID
    pub fn adapter_id(&self) -> id::AdapterId {
        self.adapter_id
    }

    /// Get the queue ID
    pub fn queue_id(&self) -> id::QueueId {
        self.queue_id
    }

    /// Get the swapchain format (color attachment format)
    pub fn swapchain_format(&self) -> wgt::TextureFormat {
        self.swapchain_format
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

    /// Get or create a depth texture view for the given dimensions
    /// Used when MC doesn't provide depth texture but pipeline requires it
    pub fn get_or_create_depth_view(&self, width: u32, height: u32) -> Result<id::TextureViewId> {
        let key = (width, height);

        // Check cache first
        {
            let cache = self.depth_texture_cache.lock();
            if let Some((_, view_id)) = cache.get(&key) {
                log::debug!("Using cached depth texture for {}x{}", width, height);
                return Ok(*view_id);
            }
        }

        // Create new depth texture
        log::info!("Creating depth texture for {}x{}", width, height);

        // wgpu 28.0: Use MemoryUsage hint for depth buffers (reduces memory footprint)
        // Depth buffers are only used during rendering and don't need to persist
        // Note: memory_hints is a device-level property, not per-texture
        let depth_desc = wgt::TextureDescriptor {
            label: Some(Cow::Borrowed("Cached Depth Texture")),
            size: wgt::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgt::TextureDimension::D2,
            format: wgt::TextureFormat::Depth32Float,
            usage: wgt::TextureUsages::RENDER_ATTACHMENT,
            view_formats: vec![],
        };

        let (texture_id, err) = self.context.inner().device_create_texture(
            self.device_id,
            &depth_desc,
            None,
        );

        if let Some(e) = err {
            return Err(BasaltError::wgpu_context("depth texture creation", format!("{:?}", e)));
        }

        let view_desc = wgpu_core::resource::TextureViewDescriptor {
            label: Some(Cow::Borrowed("Cached Depth View")),
            format: Some(wgt::TextureFormat::Depth32Float),
            dimension: Some(wgt::TextureViewDimension::D2),
            usage: None,
            range: wgt::ImageSubresourceRange::default(),
        };

        let (view_id, err) = self.context.inner().texture_create_view(
            texture_id,
            &view_desc,
            None,
        );

        if let Some(e) = err {
            return Err(BasaltError::wgpu_context("depth texture view creation", format!("{:?}", e)));
        }

        // Register the view-to-texture mapping
        self.context.register_texture_view(view_id, texture_id);

        // Cache it
        self.depth_texture_cache.lock().insert(key, (texture_id, view_id));
        log::info!("Created and cached depth texture {:?} view {:?} for {}x{}", texture_id, view_id, width, height);

        Ok(view_id)
    }

    /// Clear all cached depth textures to free GPU memory
    ///
    /// This should be called when:
    /// - The window is resized (old-sized depth textures are no longer needed)
    /// - Memory pressure is detected
    /// - Explicitly requested by the application
    ///
    /// # Example
    /// ```rust
    /// device.clear_depth_cache();  // Frees all cached depth textures
    /// ```
    pub fn clear_depth_cache(&self) {
        let mut cache = self.depth_texture_cache.lock();
        let count = cache.len();

        if count == 0 {
            log::debug!("Depth cache is already empty");
            return;
        }

        log::info!("Clearing depth texture cache ({} entries)", count);

        // Drop old textures/views - wgpu-core will handle actual GPU cleanup
        for (_, (texture_id, view_id)) in cache.drain() {
            // Unregister the view-to-texture mapping
            self.context.unregister_texture_view(view_id);

            // Signal wgpu-core to release resources
            let _ = self.context.inner().texture_view_drop(view_id);
            let _ = self.context.inner().texture_drop(texture_id);
        }

        log::info!("Cleared {} depth textures from cache", count);
    }

    /// Clear depth textures matching specific dimensions
    ///
    /// Useful for cleaning up when resizing the window.
    pub fn clear_depth_cache_for_size(&self, width: u32, height: u32) {
        let key = (width, height);
        let mut cache = self.depth_texture_cache.lock();

        if let Some((texture_id, view_id)) = cache.remove(&key) {
            log::info!("Clearing depth texture for size {}x{}", width, height);

            // Unregister the view-to-texture mapping
            self.context.unregister_texture_view(view_id);

            // Signal wgpu-core to release resources
            let _ = self.context.inner().texture_view_drop(view_id);
            let _ = self.context.inner().texture_drop(texture_id);
        }
    }

    /// Get the number of entries in the depth texture cache
    pub fn depth_cache_size(&self) -> usize {
        self.depth_texture_cache.lock().len()
    }

    /// Acquire the swapchain texture for rendering
    ///
    /// Always acquires a fresh swapchain texture each frame.
    /// Swapchain textures cannot be reused - once presented, a new one must be acquired.
    pub fn acquire_swapchain_texture(&self) -> Result<id::TextureId> {
        let surface = self.surface.as_ref()
            .ok_or_else(|| BasaltError::surface("No surface available"))?;

        // Always get a fresh swapchain texture - swapchain textures must be acquired each frame
        let output = self.context.inner().surface_get_current_texture(
            surface.id(),
            None,
        ).map_err(|e| BasaltError::surface(format!("Failed to acquire swapchain texture: {:?}", e)))?;

        let texture_id = output.texture
            .ok_or_else(|| BasaltError::surface("Swapchain texture not available"))?;

        log::info!("Acquired swapchain texture: {:?}", texture_id);
        Ok(texture_id)
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
            mipmap_filter: wgt::MipmapFilterMode::Nearest,
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
        // wgpu 28.0: Use DontCare since blit shader overwrites all pixels (performance optimization)
        let color_attachments = vec![Some(wgpu_core::command::RenderPassColorAttachment {
            view: dst_view,
            resolve_target: None,
            load_op: wgpu_core::command::LoadOp::DontCare(unsafe { LoadOpDontCare::enabled() }),
            store_op: wgpu_core::command::StoreOp::Store,
            depth_slice: None,
        })];

        let pass_desc = wgpu_core::command::RenderPassDescriptor {
            label: Some(Cow::Borrowed("Blit Pass")),
            color_attachments: Cow::Borrowed(&color_attachments),
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,  // No multiview (wgpu 28.0+)
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

        // Draw fullscreen quad (6 vertices for 2 triangles, wgpu-mc style)
        if let Err(e) = self.context.inner().render_pass_draw(
            &mut render_pass,
            6, // vertex count (2 triangles = 6 vertices)
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
            immediate_size: 0,  // No immediates needed for blit shader
        };

        let (pipeline_layout_id, error) = self.context.inner().device_create_pipeline_layout(
            self.device_id,
            &pipeline_layout_desc,
            None,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("Failed to create pipeline layout: {:?}", e)));
        }

        // Blit shader - standard fullscreen triangle pattern from rend3/Bevy/wgpu-examples
        // This shader blits from a source texture to the swapchain
        let blit_shader_source = include_str!("shaders/blit.wgsl");

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
            multiview_mask: None,
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
    ///
    /// **CRITICAL FIX #2:** Proper GPU synchronization using device.poll()
    ///
    /// Frame synchronization strategy (see checklist Steps 271-285):
    /// - Track frames in flight (max 2 for triple buffering)
    /// - Use device.poll() to check ACTUAL GPU completion (not just counters)
    /// - Reset frame counter when GPU queue is empty (prevents drift)
    /// - Increment on work submission, reset when GPU confirms completion
    pub fn present_frame(&self) -> Result<()> {
        let surface = match &self.surface {
            Some(s) => s,
            None => {
                log::debug!("No surface, skipping present");
                return Ok(());
            }
        };

        // **CRITICAL FIX #2:** Frame synchronization with actual GPU completion check
        // If too many frames are in flight, wait for the GPU to complete work
        if self.frame_tracker.should_wait() {
            log::info!("Too many frames in flight ({}/{}), waiting for GPU to complete...",
                self.frame_tracker.count(), self.frame_tracker.max_frames_in_flight);

            // **CRITICAL:** Use device.poll() to check actual GPU completion status
            // This is not just a counter - it asks the GPU driver what's actually done
            match self.poll_device(true) {
                Ok(queue_empty) => {
                    // **CRITICAL FIX #2:** Use reset_if_empty instead of manual decrement
                    // When device.poll() reports queue empty, ALL frames are done
                    // This is more accurate than assuming only one frame completed
                    self.frame_tracker.reset_if_empty(queue_empty);

                    if !queue_empty {
                        // Queue has work, so at least one frame must have completed
                        // to make room for new work
                        log::debug!("GPU has work remaining, {} frames still in flight",
                            self.frame_tracker.count());
                    }
                }
                Err(e) => {
                    log::warn!("Failed to wait for GPU: {}, proceeding anyway", e);
                    // Don't reset counter on error - might cause underflow
                }
            }
        }

        // Acquire the swapchain texture
        let swapchain_texture = match self.acquire_swapchain_texture() {
            Ok(t) => t,
            Err(e) => {
                log::warn!("Failed to acquire swapchain texture: {}", e);
                return Ok(()); // Don't fail, just skip this frame
            }
        };

        // Get the main framebuffer to blit from (if we have one)
        // **CRITICAL FIX #3:** Use mutex lock for proper synchronization
        let state = self.swapchain_state.lock();
        if let Some(main_fb) = state.main_framebuffer {
            log::info!("Blitting main framebuffer {:?} to swapchain {:?}", main_fb, swapchain_texture);

            // Blit using a render pass (handles format conversion)
            if let Err(e) = self.blit_to_swapchain(main_fb, swapchain_texture) {
                log::error!("Failed to blit to swapchain: {}", e);
                // Continue anyway and try to present
            } else {
                log::info!("Blit completed successfully");
                // Increment frame tracker for work submitted during blit
                self.frame_tracker.increment();
            }
        } else {
            // No main framebuffer detected - clear the swapchain to black to avoid garbage
            log::warn!("No main framebuffer detected - clearing swapchain to black");
            if let Err(e) = self.clear_swapchain(swapchain_texture) {
                log::error!("Failed to clear swapchain: {}", e);
            } else {
                // Increment frame tracker for work submitted during clear
                self.frame_tracker.increment();
            }
        }

        // macOS pre-present notification for proper frame synchronization
        // This must be called before present() on macOS for correct timing
        surface.pre_present_notify();

        // Present the surface
        match surface.present(self.queue_id) {
            Ok(status) => {
                log::info!("Presented frame with status: {:?}", status);
                Ok(())
            }
            Err(e) => {
                log::error!("Failed to present frame: {:?}", e);
                Err(BasaltError::surface(format!("Failed to present: {:?}", e)))
            }
        }
    }

    /// Clear the swapchain texture to black (fallback when no main framebuffer)
    fn clear_swapchain(&self, swapchain_texture: id::TextureId) -> Result<()> {
        // Create texture view for the swapchain
        let view_desc = wgpu_core::resource::TextureViewDescriptor {
            label: Some(Cow::Borrowed("Swapchain Clear View")),
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
            swapchain_texture,
            &view_desc,
            None,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("Failed to create swapchain view: {:?}", e)));
        }

        // Create command encoder
        let encoder_desc = wgt::CommandEncoderDescriptor {
            label: Some(Cow::Borrowed("Clear Encoder")),
        };

        let (encoder_id, error) = self.context.inner().device_create_command_encoder(
            self.device_id,
            &encoder_desc,
            None,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("Failed to create encoder: {:?}", e)));
        }

        // Create render pass that clears to black
        let color_attachments = vec![Some(wgpu_core::command::RenderPassColorAttachment {
            view: view_id,
            resolve_target: None,
            load_op: wgpu_core::command::LoadOp::Clear(wgt::Color::BLACK),
            store_op: wgpu_core::command::StoreOp::Store,
            depth_slice: None,
        })];

        let pass_desc = wgpu_core::command::RenderPassDescriptor {
            label: Some(Cow::Borrowed("Clear Pass")),
            color_attachments: Cow::Borrowed(&color_attachments),
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,  // No multiview (wgpu 28.0+)
        };

        // Begin and immediately end the render pass (just clears)
        let (mut render_pass, error) = self.context.inner().command_encoder_begin_render_pass(
            encoder_id,
            &pass_desc,
        );

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("Failed to begin clear pass: {:?}", e)));
        }

        if let Err(e) = self.context.inner().render_pass_end(&mut render_pass) {
            return Err(BasaltError::Wgpu(format!("Failed to end clear pass: {:?}", e)));
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
            .map_err(|e| BasaltError::Wgpu(format!("Failed to submit clear: {:?}", e)))?;

        log::debug!("Cleared swapchain to black");
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

    /// Explicitly set the main framebuffer texture for presentation
    /// This should be called when a render pass targets a texture that will be presented
    pub fn set_main_framebuffer(&self, texture_id: id::TextureId) {
        log::info!("Explicitly setting main framebuffer to {:?}", texture_id);
        // **CRITICAL FIX #3:** Use mutex lock for proper synchronization
        let mut state = self.swapchain_state.lock();
        state.main_framebuffer = Some(texture_id);
        // Mutex guard is released here automatically
    }

    /// Set the main framebuffer from a texture view ID
    /// Looks up the parent texture of the view and sets it as the main framebuffer
    pub fn set_main_framebuffer_from_view(&self, view_id: id::TextureViewId) {
        // For now, we can't easily get the parent texture from a view in wgpu-core
        // So we'll need to track this separately or use a different approach
        log::debug!("set_main_framebuffer_from_view called with view {:?}", view_id);
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

    /// Poll the device and check device status (wgpu pattern)
    ///
    /// This processes any pending GPU operations and returns the device status.
    /// Based on wgpu's pattern of polling to process async operations.
    /// Use this to synchronize with the GPU and ensure operations have completed.
    ///
    /// # Arguments
    /// * `wait` - If true, wait until all operations complete. If false, just check status.
    ///
    /// # Returns
    /// * `Ok(true)` - Queue is empty (no pending work)
    /// * `Ok(false)` - Queue has work
    /// * `Err(...)` - Poll failed
    pub fn poll_device(&self, wait: bool) -> Result<bool> {
        let poll_type = if wait {
            wgt::PollType::wait_indefinitely()
        } else {
            wgt::PollType::Poll
        };

        match self.context.inner().device_poll(self.device_id, poll_type) {
            Ok(status) => {
                if status.is_queue_empty() {
                    log::debug!("Device poll: queue is empty");
                } else {
                    log::debug!("Device poll: queue has work");
                }
                Ok(status.is_queue_empty())
            }
            Err(e) => {
                log::error!("Device poll error: {:?}", e);
                Err(BasaltError::device_creation(format!("Device poll failed: {:?}", e)))
            }
        }
    }

    /// Create a buffer with a descriptive debug label based on usage
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

        // Create a descriptive label based on usage
        let label = self.buffer_usage_to_label(wgpu_usage, size);

        let desc = wgt::BufferDescriptor {
            label: Some(Cow::Owned(label)),
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
        let texture_format = self.map_texture_format_public(format)?;
        let texture_usage = self.map_texture_usage(usage);

        // For all render target textures (RENDER_ATTACHMENT), also add TEXTURE_BINDING
        // so they can be sampled as inputs in subsequent render passes (compositing, post-processing, etc.)
        // This is essential for multi-pass rendering where intermediate textures need to be sampled.
        let texture_usage = if texture_usage.contains(wgt::TextureUsages::RENDER_ATTACHMENT) {
            log::info!("Adding TEXTURE_BINDING to render target {}x{} (format={:?}) for shader sampling",
                width, height, texture_format);
            texture_usage | wgt::TextureUsages::TEXTURE_BINDING
        } else {
            texture_usage
        };

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

        // Create a descriptive label based on texture usage
        let label = self.texture_usage_to_label(filtered_usage, width, height, texture_format);

        let desc = wgt::TextureDescriptor {
            label: Some(Cow::Owned(label)),
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

        log::info!("Created texture {:?}: {}x{}x{} format={:?}",
            texture_id, width, height, depth, texture_format);

        // NOTE: main_framebuffer is now ONLY set by set_main_framebuffer() which is called
        // from endRenderPass() after a render pass completes. We no longer auto-detect it here
        // because intermediate textures (same size as swapchain) were incorrectly being marked
        // as the main framebuffer, causing the actual main framebuffer to be overwritten.

        Ok(texture_id)
    }

    /// Destroy a texture
    pub fn destroy_texture(&self, texture_id: id::TextureId) {
        self.context.inner().texture_drop(texture_id);
    }

    /// Create a texture view with descriptive debug label, returns (view_id, dimension)
    /// array_layers is used to determine if this is a D2 or D2Array texture
    pub fn create_texture_view(
        &self,
        texture_id: id::TextureId,
        array_layers: u32,
    ) -> Result<(id::TextureViewId, wgt::TextureViewDimension)> {
        // Determine the view dimension based on array layers
        // - 1 layer = D2 (regular 2D texture)
        // - 6 layers = Cube (cubemap for panorama)
        // - >1 layers (not 6) = D2Array
        let view_dimension = if array_layers == 6 {
            log::info!("Creating Cube texture view for 6-layer texture (panorama cubemap)");
            wgt::TextureViewDimension::Cube
        } else if array_layers > 1 {
            wgt::TextureViewDimension::D2Array
        } else {
            wgt::TextureViewDimension::D2
        };

        // Create a descriptive label based on dimension
        let dim_name = match view_dimension {
            wgt::TextureViewDimension::D2 => "D2",
            wgt::TextureViewDimension::D2Array => "D2Array",
            wgt::TextureViewDimension::Cube => "Cube",
            _ => "Unknown",
        };
        let label = format!("Bassalt Texture View: {} ({} layers)", dim_name, array_layers);

        let desc = wgpu_core::resource::TextureViewDescriptor {
            label: Some(Cow::Owned(label)),
            format: None,
            dimension: Some(view_dimension),
            usage: None,
            range: wgt::ImageSubresourceRange {
                aspect: wgt::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: Some(1), // Must be 1 for render targets
                base_array_layer: 0,
                array_layer_count: None,
            },
        };
        
        let (view_id, error) = self
            .context
            .inner()
            .texture_create_view(texture_id, &desc, None);

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("{:?}", e)));
        }

        // Register the view-to-texture mapping for reliable lookups
        self.context.register_texture_view(view_id, texture_id);

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
    ///
    /// **CRITICAL:** WebGPU requires bytes_per_row to be 256-byte aligned for buffer-to-texture copies.
    /// This function automatically pads the data to meet this requirement.
    /// See checklist Step 214 for details.
    pub fn write_texture(
        &self,
        texture_id: id::TextureId,
        data: &[u8],
        mip_level: u32,
        origin_x: u32,
        origin_y: u32,
        origin_z: u32,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let texture_copy = wgt::TexelCopyTextureInfo {
            texture: texture_id,
            mip_level,
            origin: wgt::Origin3d {
                x: origin_x,
                y: origin_y,
                z: origin_z,
            },
            aspect: wgt::TextureAspect::All,
        };

        // CRITICAL FIX: Align bytes_per_row to 256 bytes (WebGPU requirement)
        // See checklist Step 214: bytes_per_row must be multiple of 256 for buffer copies
        let original_bytes_per_row = width * 4; // RGBA8 = 4 bytes per pixel
        let aligned_bytes_per_row = ((original_bytes_per_row + 255) & !255) as u32;

        // If data is already aligned, use it directly
        let (data_to_upload, data_layout) = if original_bytes_per_row == aligned_bytes_per_row {
            log::trace!("Texture data is already 256-byte aligned ({} bytes/row)", original_bytes_per_row);
            (Cow::Borrowed(data), wgt::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(original_bytes_per_row),
                rows_per_image: Some(height),
            })
        } else {
            // Data is not aligned - need to pad each row to 256-byte boundary
            log::debug!("Padding texture data from {} to {} bytes/row for 256-byte alignment",
                       original_bytes_per_row, aligned_bytes_per_row);

            let padding_bytes = (aligned_bytes_per_row - original_bytes_per_row) as usize;
            let padded_size = (aligned_bytes_per_row * height) as usize;
            let mut padded_data = Vec::with_capacity(padded_size);

            for row in 0..height {
                let row_start = (row * original_bytes_per_row) as usize;
                let row_end = row_start + original_bytes_per_row as usize;
                let row_data = &data[row_start..row_end];

                // Add the actual row data
                padded_data.extend_from_slice(row_data);

                // Add padding to reach 256-byte boundary
                if row < height - 1 {
                    // Don't pad the last row (not needed)
                    padded_data.extend(std::iter::repeat(0).take(padding_bytes));
                }
            }

            log::trace!("Padded texture data: {} -> {} bytes (added {} bytes of padding)",
                       data.len(), padded_data.len(), padded_data.len() - data.len());

            (Cow::Owned(padded_data), wgt::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(aligned_bytes_per_row),
                rows_per_image: Some(height),
            })
        };

        let size = wgt::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        self.context
            .inner()
            .queue_write_texture(self.queue_id, &texture_copy, &data_to_upload, &data_layout, &size)
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
            multiview_mask: None,  // No multiview (wgpu 28.0+)
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

    /// Generate a descriptive debug label for a buffer based on its usage
    fn buffer_usage_to_label(&self, usage: wgt::BufferUsages, size: u64) -> String {
        let mut type_parts = Vec::new();

        if usage.contains(wgt::BufferUsages::VERTEX) {
            type_parts.push("Vertex");
        }
        if usage.contains(wgt::BufferUsages::INDEX) {
            type_parts.push("Index");
        }
        if usage.contains(wgt::BufferUsages::UNIFORM) {
            type_parts.push("Uniform");
        }
        if usage.contains(wgt::BufferUsages::STORAGE) {
            type_parts.push("Storage");
        }
        if usage.contains(wgt::BufferUsages::COPY_SRC) {
            type_parts.push("CopySrc");
        }
        if usage.contains(wgt::BufferUsages::COPY_DST) {
            type_parts.push("CopyDst");
        }
        if usage.contains(wgt::BufferUsages::INDIRECT) {
            type_parts.push("Indirect");
        }

        let type_str = if type_parts.is_empty() {
            "Unknown".to_string()
        } else {
            type_parts.join("+")
        };

        // Format size nicely (KB, MB)
        let size_str = if size >= 1024 * 1024 {
            format!("{:.1}MB", size as f64 / (1024.0 * 1024.0))
        } else if size >= 1024 {
            format!("{:.1}KB", size as f64 / 1024.0)
        } else {
            format!("{}B", size)
        };

        // Add alignment info for uniform buffers (wgpu pattern)
        let extra_info = if usage.contains(wgt::BufferUsages::UNIFORM) && size <= 65536 {
            let aligned = align_to(size, self.limits.min_uniform_buffer_offset_alignment as u64);
            if aligned != size {
                format!(" (aligned: {})", aligned)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        format!("Bassalt Buffer: {} ({}{})", type_str, size_str, extra_info)
    }

    /// Generate a descriptive debug label for a texture based on its usage
    /// Enhanced with wgpu-style pattern matching for better debugging
    fn texture_usage_to_label(&self, usage: wgt::TextureUsages, width: u32, height: u32, format: wgt::TextureFormat) -> String {
        let mut type_parts = Vec::new();

        // Categorize by usage type (order matters for readability)
        if usage.contains(wgt::TextureUsages::TEXTURE_BINDING) {
            type_parts.push("Texture");
        }
        if usage.contains(wgt::TextureUsages::RENDER_ATTACHMENT) {
            type_parts.push("RenderTarget");
        }
        if usage.contains(wgt::TextureUsages::STORAGE_BINDING) {
            type_parts.push("Storage");
        }
        if usage.contains(wgt::TextureUsages::COPY_SRC) {
            type_parts.push("CopySrc");
        }
        if usage.contains(wgt::TextureUsages::COPY_DST) {
            type_parts.push("CopyDst");
        }

        let type_str = if type_parts.is_empty() {
            "Unknown".to_string()
        } else {
            type_parts.join("+")
        };

        // Calculate estimated memory size based on format
        // Uses common format sizes for estimation (wgpu pattern)
        let bytes_per_pixel = match format {
            // 4-byte formats (RGBA, BGRA, etc.)
            wgt::TextureFormat::Rgba8Unorm | wgt::TextureFormat::Rgba8UnormSrgb |
            wgt::TextureFormat::Rgba8Snorm | wgt::TextureFormat::Rgba8Uint |
            wgt::TextureFormat::Rgba8Sint | wgt::TextureFormat::Bgra8Unorm |
            wgt::TextureFormat::Bgra8UnormSrgb | wgt::TextureFormat::Rgb10a2Unorm |
            wgt::TextureFormat::Rg11b10Ufloat | wgt::TextureFormat::Rgba32Float |
            wgt::TextureFormat::Rgba32Uint | wgt::TextureFormat::Rgba32Sint => 4,

            // 2-byte formats (RG, Depth16, etc.)
            wgt::TextureFormat::Rg8Unorm | wgt::TextureFormat::Rg8Snorm |
            wgt::TextureFormat::Rg8Uint | wgt::TextureFormat::Rg8Sint |
            wgt::TextureFormat::Rg16Unorm | wgt::TextureFormat::Rg16Snorm |
            wgt::TextureFormat::Rg16Uint | wgt::TextureFormat::Rg16Sint |
            wgt::TextureFormat::Rg16Float | wgt::TextureFormat::Rg32Float |
            wgt::TextureFormat::Rg32Uint | wgt::TextureFormat::Rg32Sint |
            wgt::TextureFormat::Depth16Unorm => 2,

            // 1-byte formats (R, etc.)
            wgt::TextureFormat::R8Unorm | wgt::TextureFormat::R8Snorm |
            wgt::TextureFormat::R8Uint | wgt::TextureFormat::R8Sint => 1,

            // Larger formats (8 bytes)
            wgt::TextureFormat::Rgba16Unorm | wgt::TextureFormat::Rgba16Snorm |
            wgt::TextureFormat::Rgba16Uint | wgt::TextureFormat::Rgba16Sint |
            wgt::TextureFormat::Rgba16Float => 8,

            // Depth formats (4 bytes is typical)
            wgt::TextureFormat::Depth24Plus | wgt::TextureFormat::Depth24PlusStencil8 |
            wgt::TextureFormat::Depth32Float | wgt::TextureFormat::Depth32FloatStencil8 => 4,

            // Default to 4 bytes
            _ => 4,
        };
        let estimated_bytes = (width as u64 * height as u64 * bytes_per_pixel) as u64;

        // Format size nicely (KB, MB)
        let size_str = if estimated_bytes >= 1024 * 1024 {
            format!("{:.1}MB", estimated_bytes as f64 / (1024.0 * 1024.0))
        } else if estimated_bytes >= 1024 {
            format!("{:.1}KB", estimated_bytes as f64 / 1024.0)
        } else {
            format!("{}B", estimated_bytes)
        };

        let label = format!("Bassalt Texture: {} {}x{} ({:?}) ~{}",
            type_str, width, height, format, size_str);

        // Log detailed usage breakdown for debugging
        log::debug!("Texture label breakdown: {}", label);
        if usage.contains(wgt::TextureUsages::RENDER_ATTACHMENT) {
            log::debug!("  → This texture can be used as a render target (framebuffer/output)");
        }
        if usage.contains(wgt::TextureUsages::TEXTURE_BINDING) {
            log::debug!("  → This texture can be sampled in shaders (input texture)");
        }
        if usage.contains(wgt::TextureUsages::STORAGE_BINDING) {
            log::debug!("  → This texture can be used as a storage texture (read/write in shaders)");
        }

        label
    }

    pub fn map_texture_format_public(&self, format: u32) -> Result<wgt::TextureFormat> {
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

        // NOTE: sRGB format support with RGBA/BGRA channel swapping
        // ==========================================================
        // Minecraft textures use RGBA byte order (Red=0, Green=1, Blue=2, Alpha=3)
        // Most GPUs prefer BGRA for better performance, so the swapchain uses BGRA
        // To match these, we need to create textures in BGRA format when MC sends RGBA data
        //
        // Format mapping:
        // - RGBA8 (MC's format) → Bgra8UnormSrgb (GPU's preferred format)
        // - BGRA8 → Rgba8UnormSrgb (inverse mapping for completeness)
        // - RGB8 → Bgra8UnormSrgb (add alpha channel)
        //
        // This ensures MC's RGBA (0,0,255) texture data renders as blue on BGRA framebuffer
        // instead of being incorrectly interpreted as red (255,0,0).

        Ok(match format {
            // Map MC's RGBA data to BGRA textures for correct rendering
            RGBA8 => wgt::TextureFormat::Bgra8UnormSrgb,  // MC RGBA → GPU BGRA
            BGRA8 => wgt::TextureFormat::Rgba8UnormSrgb,  // MC BGRA → GPU RGBA (inverse)
            RGB8 => wgt::TextureFormat::Bgra8UnormSrgb,   // MC RGB → GPU BGRA (add alpha)
            RG8 => wgt::TextureFormat::Rg8Unorm,
            R8 => wgt::TextureFormat::R8Unorm,
            RGBA16F => wgt::TextureFormat::Rgba16Float,
            RGBA32F => wgt::TextureFormat::Rgba32Float,
            DEPTH24 => wgt::TextureFormat::Depth24Plus,
            DEPTH32F => wgt::TextureFormat::Depth32Float,
            DEPTH24_STENCIL8 => wgt::TextureFormat::Depth24PlusStencil8,
            _ => return Err(BasaltError::invalid_parameter("format", format!("Unknown texture format: {}", format))),
        })
    }

    fn map_address_mode(&self, mode: u32) -> Result<wgt::AddressMode> {
        Ok(match mode {
            0 => wgt::AddressMode::Repeat,
            1 => wgt::AddressMode::MirrorRepeat,
            2 => wgt::AddressMode::ClampToEdge,
            3 => wgt::AddressMode::ClampToBorder,
            _ => return Err(BasaltError::invalid_parameter("mode", format!("Unknown address mode: {}", mode))),
        })
    }

    fn map_filter_mode(&self, mode: u32) -> Result<wgt::FilterMode> {
        Ok(match mode {
            0 => wgt::FilterMode::Nearest,
            1 => wgt::FilterMode::Linear,
            _ => return Err(BasaltError::invalid_parameter("mode", format!("Unknown filter mode: {}", mode))),
        })
    }

    fn map_mipmap_filter(&self, mode: u32) -> Result<wgt::MipmapFilterMode> {
        Ok(match mode {
            0 => wgt::MipmapFilterMode::Nearest,
            1 => wgt::MipmapFilterMode::Linear,
            _ => return Err(BasaltError::invalid_parameter("mode", format!("Unknown mipmap filter: {}", mode))),
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
            _ => return Err(BasaltError::invalid_parameter("factor", format!("Unknown blend factor: {}", factor))),
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
            _ => return Err(BasaltError::invalid_parameter("function", format!("Unknown compare function: {}", func))),
        })
    }

    pub fn map_primitive_topology(&self, topology: u32) -> Result<wgt::PrimitiveTopology> {
        Ok(match topology {
            0 => wgt::PrimitiveTopology::PointList,
            1 => wgt::PrimitiveTopology::LineList,
            2 => wgt::PrimitiveTopology::LineStrip,
            3 => wgt::PrimitiveTopology::TriangleList,
            4 => wgt::PrimitiveTopology::TriangleStrip,
            _ => return Err(BasaltError::invalid_parameter("topology", format!("Unknown topology: {}", topology))),
        })
    }

    fn parse_wgsl(&self, wgsl: &str) -> Result<naga::Module> {
        // Use the shader module's parse function which logs compilation info
        crate::shader::parse_wgsl_named(wgsl, "device_shader")
    }
}

/// Build view_formats for surface configuration (wgpu 27.0 best practice)
///
/// wgpu 27.0 recommends including both the base format and its sRGB variant
/// in view_formats for better compatibility. This allows render passes to
/// use either format for the same swapchain texture.
fn build_view_formats(base_format: &wgt::TextureFormat, supported_formats: &[wgt::TextureFormat]) -> Vec<wgt::TextureFormat> {
    let mut view_formats = Vec::with_capacity(2);

    // Always include the base format itself
    view_formats.push(*base_format);

    // Add sRGB variant if supported
    let srgb_variant = match base_format {
        wgt::TextureFormat::Rgba8Unorm => Some(wgt::TextureFormat::Rgba8UnormSrgb),
        wgt::TextureFormat::Bgra8Unorm => Some(wgt::TextureFormat::Bgra8UnormSrgb),
        wgt::TextureFormat::Rgba8UnormSrgb => Some(wgt::TextureFormat::Rgba8Unorm),
        wgt::TextureFormat::Bgra8UnormSrgb => Some(wgt::TextureFormat::Bgra8Unorm),
        _ => None,
    };

    if let Some(variant) = srgb_variant {
        if supported_formats.contains(&variant) {
            view_formats.push(variant);
        }
    }

    view_formats
}

/// Safely extract NSView* from NSWindow* on macOS
///
/// GLFW returns NSWindow* but wgpu-core requires NSView* for Metal surface creation.
/// This function safely extracts the contentView and validates it before returning.
///
/// # Arguments
/// * `ns_window_ptr` - Raw pointer to NSWindow (from glfwGetCocoaWindow)
///
/// # Returns
/// * `Ok(ptr)` - Valid non-null NSView* pointer
/// * `Err(...)` - If ns_window_ptr is null or contentView is nil
#[cfg(target_os = "macos")]
fn extract_ns_view_safely(ns_window_ptr: u64) -> Result<std::ptr::NonNull<std::ffi::c_void>> {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use std::ptr::NonNull;

    // Validate input pointer is not null
    let ns_window_raw = ns_window_ptr as *mut AnyObject;
    if ns_window_raw.is_null() {
        return Err(BasaltError::surface("NSWindow pointer is null - GLFW window not initialized properly"));
    }

    // SAFETY: We've verified the pointer is not null above.
    // The NSWindow reference is borrowed from GLFW and guaranteed valid during this call.
    let content_view: *mut AnyObject = unsafe {
        msg_send![ns_window_raw, contentView]
    };

    // Validate contentView is not nil (Objective-C null)
    if content_view.is_null() {
        return Err(BasaltError::surface(
            "NSWindow.contentView is nil - window may not be fully initialized or was destroyed"
        ));
    }

    // Convert to c_void pointer for raw_window_handle
    let ns_view = content_view as *mut std::ffi::c_void;

    // Final validation - ensure we have a valid NonNull
    NonNull::new(ns_view)
        .ok_or_else(|| BasaltError::surface("NSView pointer conversion failed - unexpected null"))
}

/// Check if current thread is the main thread on macOS
///
/// Uses NSThread.isMainThread to determine execution context.
/// This is needed because Metal surface creation must happen on the main thread.
#[cfg(target_os = "macos")]
fn is_main_thread() -> bool {
    use objc2::msg_send;

    // SAFETY: NSThread class lookup and isMainThread are both safe and
    // guaranteed to be available on all macOS versions we support.
    unsafe {
        let nsthread_class: *const objc2::runtime::AnyClass = objc2::runtime::AnyClass::get("NSThread")
            .expect("NSThread class not found - Objective-C runtime not available?");
        let is_main: bool = msg_send![nsthread_class, isMainThread];
        is_main
    }
}

/// Align a value to a given alignment (wgpu utility pattern)
///
/// This is used for calculating proper buffer offsets and sizes that meet
/// GPU alignment requirements. Based on wgpu's `align_to` utility.
///
/// # Arguments
/// * `value` - The value to align
/// * `alignment` - The alignment requirement (must be power of 2)
///
/// # Examples
/// ```
/// assert_eq!(align_to(100, 256), 256);
/// assert_eq!(align_to(512, 256), 512);
/// assert_eq!(align_to(257, 256), 512);
/// ```
fn align_to(value: u64, alignment: u64) -> u64 {
    debug_assert!(alignment.is_power_of_two(), "Alignment must be a power of 2");
    (value + alignment - 1) & !(alignment - 1)
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
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let (raw_window_handle, raw_display_handle) = {
        use std::ptr::NonNull;
        use raw_window_handle::{XlibWindowHandle, XlibDisplayHandle};

        if display_ptr != 0 {
            // We have a valid display pointer - use X11
            let window_handle = XlibWindowHandle::new(window_ptr);
            let display_handle = XlibDisplayHandle::new(
                Some(NonNull::new(display_ptr as *mut _)
                    .ok_or_else(|| BasaltError::surface("Invalid X11 display handle"))?),
                0  // screen number - 0 is the default screen
            );

            log::info!("Using X11 window system (display: {:p}, window: {:x})", display_ptr as *const (), window_ptr);
            (RawWindowHandle::Xlib(window_handle), RawDisplayHandle::Xlib(display_handle))
        } else {
            // No display handle available - cannot create surface
            return Err(BasaltError::surface(
                "No valid display handle - GLFW must provide either X11 or Wayland handles"
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
    let (_raw_window_handle, _raw_display_handle) = {
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
        use raw_window_handle::{AppKitWindowHandle, AppKitDisplayHandle, RawWindowHandle, RawDisplayHandle};

        log::info!("macOS: Starting surface creation flow");

        // Check if we're already on the main thread to avoid deadlock
        let is_main_thread = is_main_thread();
        log::info!("macOS: Is main thread: {}", is_main_thread);

        if is_main_thread {
            // Already on main thread, execute directly
            log::info!("macOS: Already on main thread, executing surface creation directly");

            // SAFELY extract NSView from NSWindow with proper null checking
            let ns_view = extract_ns_view_safely(window_ptr)?;

            log::info!("macOS: Successfully extracted NSView at {:p}", ns_view);

            let window_handle = AppKitWindowHandle::new(ns_view);
            let display_handle = AppKitDisplayHandle::new();
            let raw_window_handle = RawWindowHandle::AppKit(window_handle);
            let raw_display_handle = RawDisplayHandle::AppKit(display_handle);

            // SAFETY: Raw window/display handles are valid and have been validated
            let surface_result = unsafe {
                context.inner().instance_create_surface(
                    raw_display_handle,
                    raw_window_handle,
                    None,
                )
            };

            surface_result.map_err(|e| BasaltError::wgpu_context("macOS surface creation", format!("{:?}", e)))?
        } else {
            // On a background thread, dispatch to main
            log::info!("macOS: On background thread, dispatching to main queue");

            let result: StdArc<StdMutex<Option<std::result::Result<id::SurfaceId, String>>>> =
                StdArc::new(StdMutex::new(None));

            let result_clone = StdArc::clone(&result);
            let context_clone = context.clone();
            let window_ptr_copy = window_ptr;

            dispatch::Queue::main().exec_sync(move || {
                log::info!("macOS: Inside GCD main queue block");

                // SAFELY extract NSView from NSWindow with proper null checking
                let ns_view = match extract_ns_view_safely(window_ptr_copy) {
                    Ok(v) => v,
                    Err(e) => {
                        // Store the error message - actual error details were logged
                        *result_clone.lock().unwrap() = Some(Err(e.to_string()));
                        return;
                    }
                };

                log::info!("macOS: Successfully extracted NSView at {:p}", ns_view);

                let window_handle = AppKitWindowHandle::new(ns_view);
                let display_handle = AppKitDisplayHandle::new();
                let raw_window_handle = RawWindowHandle::AppKit(window_handle);
                let raw_display_handle = RawDisplayHandle::AppKit(display_handle);

                // SAFETY: Raw window/display handles are valid and have been validated
                let surface_result = unsafe {
                    context_clone.inner().instance_create_surface(
                        raw_display_handle,
                        raw_window_handle,
                        None,
                    )
                };

                *result_clone.lock().unwrap() = Some(surface_result.map_err(|e| e.to_string()));
            });

            let surface_id = result.lock()
                .unwrap()
                .take()
                .unwrap()
                .map_err(|e| BasaltError::wgpu_context("macOS surface creation (dispatched)", e))?;
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
    }.map_err(|e| BasaltError::wgpu_context("surface creation", format!("{:?}", e)))?;

    // Request adapter compatible with the surface
    let adapter_opts = wgpu_core::instance::RequestAdapterOptions {
        power_preference: wgt::PowerPreference::HighPerformance,
        compatible_surface: Some(surface_id),
        force_fallback_adapter: false,
    };

    let adapter_id = context
        .inner()
        .request_adapter(&adapter_opts, wgt::Backends::all(), None)
        .map_err(|e| BasaltError::device_creation(format!("Failed to find adapter: {:?}", e)))?;

    // Query adapter for available features to enable advanced capabilities
    let adapter_features = context
        .inner()
        .adapter_features(adapter_id);

    // Check if experimental features should be enabled via environment variable
    //
    // **WARNING: EXPERIMENTAL FEATURES**
    // ===================================
    // Setting BASALT_EXPERIMENTAL=1 enables wgpu experimental features that:
    // - May cause GPU crashes or undefined behavior
    // - Are not guaranteed to work across all drivers/GPUs
    // - May have security implications
    // - Are NOT recommended for production use
    //
    // Only enable for:
    // - Development/testing of new WebGPU features
    // - Benchmarking experimental capabilities
    // - Contributing to wgpu development
    //
    // wgpu 27.0: Experimental features require explicit unsafe opt-in
    let experimental_features = if std::env::var("BASALT_EXPERIMENTAL").as_deref() == Ok("1") {
        log::warn!("========================================");
        log::warn!("EXPERIMENTAL FEATURES ENABLED!");
        log::warn!("Stability is NOT guaranteed");
        log::warn!("DO NOT use in production!");
        log::warn!("========================================");
        unsafe { wgt::ExperimentalFeatures::enabled() }
    } else {
        wgt::ExperimentalFeatures::disabled()
    };

    // Build required features with advanced capabilities if available
    // Start with base features required by Bassalt
    let mut required_features = wgt::Features::DEPTH_CLIP_CONTROL | wgt::Features::IMMEDIATES;

    // Enable timestamp queries if available (for GPU profiling)
    if adapter_features.contains(wgt::Features::TIMESTAMP_QUERY) {
        log::info!("Adapter supports TIMESTAMP_QUERY - GPU profiling available");
        required_features |= wgt::Features::TIMESTAMP_QUERY;
    }
    if adapter_features.contains(wgt::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS) {
        required_features |= wgt::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
    }
    if adapter_features.contains(wgt::Features::TIMESTAMP_QUERY_INSIDE_PASSES) {
        required_features |= wgt::Features::TIMESTAMP_QUERY_INSIDE_PASSES;
    }

    // Enable RenderBundles if available (for optimized repeated draws)
    if adapter_features.contains(wgt::Features::TIMESTAMP_QUERY) {
        log::info!("Adapter supports RENDER_BUNDLE - optimized repeated draws available");
        // Note: RENDER_BUNDLE is always available in wgpu 27.0
    }

    // wgpu 28.0: Detect and enable advanced features

    // Barycentric coordinates for advanced shader effects
    if adapter_features.contains(wgt::Features::SHADER_BARYCENTRICS) {
        log::info!("Adapter supports SHADER_BARYCENTRICS - barycentric coordinates available");
        required_features |= wgt::Features::SHADER_BARYCENTRICS;
    }

    // Subgroup operations for compute shader optimization
    if adapter_features.contains(wgt::Features::SUBGROUP) {
        log::info!("Adapter supports SUBGROUP - subgroup operations available");
        required_features |= wgt::Features::SUBGROUP;
    }
    if adapter_features.contains(wgt::Features::SUBGROUP_VERTEX) {
        log::info!("Adapter supports SUBGROUP_VERTEX - vertex stage subgroup operations available");
        required_features |= wgt::Features::SUBGROUP_VERTEX;
    }
    if adapter_features.contains(wgt::Features::SUBGROUP_BARRIER) {
        log::info!("Adapter supports SUBGROUP_BARRIER - subgroup barriers available");
        required_features |= wgt::Features::SUBGROUP_BARRIER;
    }

    // Multisample array textures for advanced anti-aliasing
    if adapter_features.contains(wgt::Features::MULTISAMPLE_ARRAY) {
        log::info!("Adapter supports MULTISAMPLE_ARRAY - multisampled array textures available");
        required_features |= wgt::Features::MULTISAMPLE_ARRAY;
    }


    // Request device with required features (matching wgpu-mc)
    // wgpu 27.0 requires explicit memory_hints and experimental_features
    let device_desc = wgt::DeviceDescriptor {
        label: Some(Cow::Borrowed("Bassalt Device")),
        required_features,
        required_limits: wgt::Limits {
            // Immediates (push constants) require max_immediate_size to be set
            max_immediate_size: 128,
            max_bind_groups: 8,
            ..wgt::Limits::default()
        },
        // wgpu 27.0: Explicit memory hints for better allocation strategy
        memory_hints: wgt::MemoryHints::Performance,
        // wgpu 27.0: Experimental features controlled by BASALT_EXPERIMENTAL env var
        experimental_features,
        // wgpu 27.0: Explicit trace path (None = Off)
        trace: wgt::Trace::Off,
    };

    let (device_id, queue_id) = context
        .inner()
        .adapter_request_device(adapter_id, &device_desc, None, None)
        .map_err(|e| BasaltError::device_creation(format!("Failed to create device: {:?}", e)))?;

    // Wrap surface in BasaltSurface
    let mut bassalt_surface = BasaltSurface::from_id(context.clone(), surface_id);

    // Query surface capabilities to find the best format
    let surface_caps = context
        .inner()
        .surface_get_capabilities(surface_id, adapter_id)
        .map_err(|e| BasaltError::surface(format!("Failed to get surface capabilities: {:?}", e)))?;

    // Prefer sRGB formats for correct color rendering (Minecraft expects sRGB)
    // Try Bgra8UnormSrgb first (GPU's preferred format), then fallbacks
    // Note: MC's RGBA texture data will be converted to BGRA in map_texture_format_public
    let surface_format = surface_caps
        .formats
        .iter()
        .copied()
        .find(|f| matches!(f, wgt::TextureFormat::Bgra8UnormSrgb))
        .or_else(|| {
            surface_caps
                .formats
                .iter()
                .copied()
                .find(|f| matches!(f, wgt::TextureFormat::Bgra8Unorm))
        })
        .or_else(|| {
            surface_caps
                .formats
                .iter()
                .copied()
                .find(|f| matches!(f, wgt::TextureFormat::Rgba8Unorm | wgt::TextureFormat::Rgba8UnormSrgb))
        })
        .unwrap_or(surface_caps.formats[0]);

    log::info!("Selected surface format: {:?} (available: {:?})", surface_format, surface_caps.formats);

    // Select present mode - prefer AutoNoVsync for lower latency (like wgpu-mc)
    let present_mode = surface_caps
        .present_modes
        .iter()
        .copied()
        .find(|m| matches!(m, wgt::PresentMode::Mailbox))
        .or_else(|| {
            surface_caps
                .present_modes
                .iter()
                .copied()
                .find(|m| matches!(m, wgt::PresentMode::Immediate))
        })
        .unwrap_or(wgt::PresentMode::Fifo);

    log::info!("Selected present mode: {:?} (available: {:?})", present_mode, surface_caps.present_modes);

    // wgpu 27.0: Build view_formats with sRGB fallbacks for better compatibility
    // This allows render passes to use either the base format or its sRGB variant
    let view_formats = build_view_formats(&surface_format, &surface_caps.formats);

    // Configure the surface
    let surface_config = wgt::SurfaceConfiguration {
        usage: wgt::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: _width,
        height: _height,
        present_mode,
        desired_maximum_frame_latency: 2,
        alpha_mode: wgt::CompositeAlphaMode::Auto,
        view_formats,
    };

    log::info!("Surface configured with view_formats: {:?}", surface_config.view_formats);

    bassalt_surface.configure(device_id, surface_config)?;

    BasaltDevice::new(context, device_id, adapter_id, queue_id, Some(bassalt_surface), _width, _height, surface_format)
}
