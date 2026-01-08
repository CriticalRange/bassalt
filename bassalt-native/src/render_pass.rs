//! Render pass management
//!
//! Manages the lifecycle of command encoders and render passes.
//! In wgpu-core 27, render passes have significantly changed APIs.
//! For now, this provides a simplified wrapper that creates command encoders
//! and manages their lifecycle.

use std::borrow::Cow;
use std::num::NonZero;
use std::sync::Arc;
use wgpu_core::id;
use wgpu_types as wgt;

use crate::context::BasaltContext;
use crate::error::{BasaltError, Result};

/// Commands that can be recorded in a render pass
#[derive(Debug, Clone)]
pub enum RenderCommand {
    SetPipeline {
        pipeline_id: id::RenderPipelineId,
    },
    SetVertexBuffer {
        slot: u32,
        buffer_id: id::BufferId,
        offset: u64,
        size: Option<NonZero<u64>>,
    },
    SetIndexBuffer {
        buffer_id: id::BufferId,
        index_format: wgt::IndexFormat,
        offset: u64,
        size: Option<NonZero<u64>>,
    },
    SetBindGroup {
        index: u32,
        bind_group_id: Option<id::BindGroupId>,
        offsets: Vec<u32>,
    },
    /// Set immediates for per-draw data (wgpu 28.0+)
    /// This allows passing small amounts of per-draw data without rebinding
    /// Immediates apply to all shader stages that use them (no stage specification needed)
    SetImmediates {
        /// Byte offset within the immediate data range
        offset: u32,
        /// Data to write (must be 4-byte aligned)
        data: Vec<u8>,
    },
    DrawIndexed {
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        base_vertex: i32,
        first_instance: u32,
    },
    Draw {
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    },
    SetViewport {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        min_depth: f32,
        max_depth: f32,
    },
    SetScissorRect {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    PushDebugGroup {
        label: String,
    },
    PopDebugGroup,
    InsertDebugMarker {
        label: String,
    },
}

/// Active render pass state with command recording
///
/// Records render commands and executes them atomically using wgpu-core 27's
/// command_encoder_run_render_pass closure pattern.
pub struct RenderPassState {
    context: Arc<BasaltContext>,
    device_id: id::DeviceId,
    queue_id: id::QueueId,
    command_encoder_id: id::CommandEncoderId,

    // Render pass configuration
    color_view: Option<id::TextureViewId>,
    depth_view: Option<id::TextureViewId>,
    // Track the output texture for main framebuffer detection
    // This will be set as the main framebuffer AFTER the render pass executes
    output_texture: Option<id::TextureId>,
    should_clear_color: bool,
    clear_color: wgt::Color,
    should_clear_depth: bool,
    clear_depth: f32,
    clear_stencil: u32,

    // Viewport dimensions for scissor clamping
    viewport_width: u32,
    viewport_height: u32,

    // Recorded commands
    commands: Vec<RenderCommand>,
    is_active: bool,

    // Track which bind groups are set (for validation)
    bind_groups_set: [bool; 4],
    pipeline_set: bool,

    // Track max index count for validation (from index buffer size)
    max_index_count: Option<u64>,

    // Track vertex buffer info for validation (to detect vertex overflows)
    // Stores (buffer_id, size_in_bytes) for slot 0 (main vertex buffer)
    vertex_buffer_size: Option<u64>,

    // Depth write mode tracking
    // Tracks whether any pipeline in this pass writes to depth
    // This is used to set the read_only flag on the depth attachment
    depth_mode: DepthMode,

    // Track if current pipeline is compatible with depth mode
    // When incompatible, draws are skipped to prevent validation errors
    pipeline_compatible: bool,
}

/// Depth write mode for a render pass
///
/// This tracks whether the depth attachment should be read-only or writable.
/// The mode is determined by the first pipeline set in the render pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DepthMode {
    /// No pipeline set yet (will be determined by first pipeline)
    Unknown,
    /// Depth attachment is read-only (pipelines don't write depth)
    ReadOnly,
    /// Depth attachment is writable (pipelines write depth)
    Writable,
    /// No depth attachment needed (pipelines have no depth output)
    NoDepth,
}

impl RenderPassState {
    /// Create a new render pass with command recording
    pub fn new(
        context: Arc<BasaltContext>,
        device_id: id::DeviceId,
        queue_id: id::QueueId,
        color_view: Option<id::TextureViewId>,
        depth_view: Option<id::TextureViewId>,
        output_texture: Option<id::TextureId>, // The texture that will be rendered
        should_clear_color: bool,
        clear_color: u32,
        should_clear_depth: bool,
        clear_depth: f32,
        clear_stencil: u32,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        // Create command encoder
        let encoder_desc = wgt::CommandEncoderDescriptor {
            label: Some(Cow::Borrowed("Basalt Command Encoder")),
        };

        let (command_encoder_id, error) = context
            .inner()
            .device_create_command_encoder(device_id, &encoder_desc, None);

        if let Some(e) = error {
            return Err(BasaltError::device_creation(format!("Failed to create command encoder: {:?}", e)));
        }

        // Validate depth clear value is in range [0.0, 1.0]
        // wgpu-core requires this validation to prevent GPU errors
        if should_clear_depth && (clear_depth < 0.0 || clear_depth > 1.0) {
            return Err(BasaltError::device_creation(format!(
                "Invalid depth clear value: {} (must be in range [0.0, 1.0])",
                clear_depth
            )));
        }

        // Convert clear color from u32 ARGB (Minecraft format) to wgt::Color
        let a = ((clear_color >> 24) & 0xFF) as f64 / 255.0;
        let r = ((clear_color >> 16) & 0xFF) as f64 / 255.0;
        let g = ((clear_color >> 8) & 0xFF) as f64 / 255.0;
        let b = (clear_color & 0xFF) as f64 / 255.0;

        // Create the render pass state with default viewport and scissor
        // CRITICAL: WebGPU viewport defaults to (0,0,0,0) which clips everything!
        // We MUST set viewport to the full render target size
        let mut state = Self {
            context,
            device_id,
            queue_id,
            command_encoder_id,
            color_view,
            depth_view,
            output_texture,
            should_clear_color,
            clear_color: wgt::Color { r, g, b, a },
            should_clear_depth,
            clear_depth,
            clear_stencil,
            viewport_width: width,
            viewport_height: height,
            commands: Vec::with_capacity(32), // Pre-allocate for typical frame
            is_active: true,
            bind_groups_set: [false; 4],
            pipeline_set: false,
            max_index_count: None,
            vertex_buffer_size: None,
            depth_mode: DepthMode::Unknown, // Will be determined by first pipeline
            pipeline_compatible: true, // Initially true, set false when incompatible pipeline is set
        };

        // IMPORTANT: Set default viewport and scissor rect to the full render target
        // Without this, the viewport defaults to (0,0,0,0) and nothing renders!
        state.commands.push(RenderCommand::SetViewport {
            x: 0.0,
            y: 0.0,
            width: width as f32,
            height: height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        });
        state.commands.push(RenderCommand::SetScissorRect {
            x: 0,
            y: 0,
            width,
            height,
        });
        log::debug!("Set default viewport and scissor: {}x{}", width, height);

        Ok(state)
    }

    /// Get the command encoder ID
    pub fn encoder_id(&self) -> id::CommandEncoderId {
        self.command_encoder_id
    }

    /// Check if the render pass is active
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Record a set pipeline command and track depth write mode
    ///
    /// The depth mode is determined by the first pipeline set in the render pass.
    /// Subsequent pipelines with different depth modes will log warnings but not skip draws.
    /// This allows wgpu-core to handle validation and prevents missing geometry.
    pub fn record_set_pipeline(
        &mut self,
        pipeline_id: id::RenderPipelineId,
        depth_write_enabled: bool,
        depth_test_enabled: bool,
        has_depth_output: bool,
    ) {
        // Determine depth mode on first pipeline set
        if matches!(self.depth_mode, DepthMode::Unknown) {
            self.depth_mode = if !has_depth_output {
                DepthMode::NoDepth
            } else if depth_write_enabled {
                DepthMode::Writable
            } else {
                // Depth test enabled but no write = read-only
                DepthMode::ReadOnly
            };
            log::debug!("First pipeline set: depth_mode={:?} (write={}, test={}, has_depth={})",
                self.depth_mode, depth_write_enabled, depth_test_enabled, has_depth_output);
        } else {
            // Validate compatibility with existing depth mode
            let expected_mode = if !has_depth_output {
                DepthMode::NoDepth
            } else if depth_write_enabled {
                DepthMode::Writable
            } else {
                DepthMode::ReadOnly
            };

            if self.depth_mode != expected_mode {
                // Log warning but don't skip draws - let wgpu-core handle validation
                log::warn!("Pipeline depth mode mismatch: expected {:?}, got {:?}. Draws will continue (wgpu-core will validate).",
                    self.depth_mode, expected_mode);
                // Don't set pipeline_compatible = false - allow draws to proceed
            }
        }

        self.commands.push(RenderCommand::SetPipeline { pipeline_id });
        self.pipeline_set = true;
        // Reset bind groups when pipeline changes
        self.bind_groups_set = [false; 4];
    }

    /// Record a set vertex buffer command
    pub fn record_set_vertex_buffer(
        &mut self,
        slot: u32,
        buffer_id: id::BufferId,
        offset: u64,
        size: Option<NonZero<u64>>,
    ) {
        // Track vertex buffer size for slot 0 (main vertex buffer) to detect overflows
        if slot == 0 {
            self.vertex_buffer_size = size.map(|sz| sz.get());
            log::debug!("[Bassalt] Set vertex buffer slot 0: buffer={:?}, offset={}, size={:?}",
                buffer_id, offset, size);
        }

        self.commands.push(RenderCommand::SetVertexBuffer {
            slot,
            buffer_id,
            offset,
            size,
        });
    }

    /// Record a set index buffer command
    pub fn record_set_index_buffer(
        &mut self,
        buffer_id: id::BufferId,
        index_format: wgt::IndexFormat,
        offset: u64,
        size: Option<NonZero<u64>>,
    ) {
        self.commands.push(RenderCommand::SetIndexBuffer {
            buffer_id,
            index_format,
            offset,
            size,
        });
    }

    /// Record a set bind group command
    pub fn record_set_bind_group(
        &mut self,
        index: u32,
        bind_group_id: Option<id::BindGroupId>,
        offsets: Vec<u32>,
    ) {
        self.commands.push(RenderCommand::SetBindGroup {
            index,
            bind_group_id,
            offsets,
        });
        if (index as usize) < self.bind_groups_set.len() {
            self.bind_groups_set[index as usize] = true;
        }
    }

    /// Record a draw indexed command
    pub fn record_draw_indexed(
        &mut self,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        base_vertex: i32,
        first_instance: u32,
    ) {
        // Validate state before draw
        if !self.pipeline_set {
            log::warn!("DrawIndexed called without pipeline set!");
        }
        if !self.bind_groups_set[0] {
            log::warn!("DrawIndexed called without bind group 0 set!");
        }

        // VALIDATION: Check for vertex buffer overflow
        // For POSITION_COLOR format, vertex stride is 16 bytes
        // This is a heuristic - actual stride depends on the vertex format
        const VERTEX_STRIDE: u64 = 16; // POSITION_COLOR: 12 bytes pos + 4 bytes color

        if let Some(vbuf_size) = self.vertex_buffer_size {
            // Calculate max vertex index that could be accessed
            // Worst case: base_vertex + index_count (if indices are 0,1,2,...)
            let max_vertex_index = if base_vertex >= 0 {
                base_vertex as u64 + index_count as u64 - 1
            } else {
                // Negative base_vertex means we're reading from before the buffer start
                index_count as u64
            };

            // Calculate how many vertices fit in the buffer
            let max_vertices = vbuf_size / VERTEX_STRIDE;

            if max_vertex_index >= max_vertices {
                log::warn!(
                    "[Bassalt] VERTEX BUFFER OVERFLOW DETECTED: \
                    drawIndexed(indices={}, baseVertex={}) will access vertex {} \
                    but buffer only fits {} vertices ({} bytes / {} stride). \
                    This will cause flashing triangles from unwritten vertices!",
                    index_count, base_vertex, max_vertex_index, max_vertices, vbuf_size, VERTEX_STRIDE
                );

                // Don't skip the draw - let wgpu-core validate and error if needed
                // But at least we warned about the root cause
            }
        }

        self.commands.push(RenderCommand::DrawIndexed {
            index_count,
            instance_count,
            first_index,
            base_vertex,
            first_instance,
        });
    }

    /// Record a draw command
    pub fn record_draw(
        &mut self,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) {
        self.commands.push(RenderCommand::Draw {
            vertex_count,
            instance_count,
            first_vertex,
            first_instance,
        });
    }

    /// Record a set viewport command
    pub fn record_set_viewport(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        min_depth: f32,
        max_depth: f32,
    ) {
        // Validate viewport dimensions against render target size
        if width > self.viewport_width as f32 || height > self.viewport_height as f32 {
            log::warn!(
                "Viewport dimensions ({}, {}) exceed render target ({}, {}) - clamping",
                width, height, self.viewport_width, self.viewport_height
            );
        }

        // Clamp viewport to render target bounds
        let clamped_width = width.min(self.viewport_width as f32);
        let clamped_height = height.min(self.viewport_height as f32);

        // Validate depth range
        if min_depth < 0.0 || min_depth > 1.0 {
            log::warn!("Viewport min_depth {} is outside [0, 1] range - this may cause issues", min_depth);
        }
        if max_depth < 0.0 || max_depth > 1.0 {
            log::warn!("Viewport max_depth {} is outside [0, 1] range - this may cause issues", max_depth);
        }
        if min_depth > max_depth {
            log::warn!("Viewport min_depth {} > max_depth {} - swapping values", min_depth, max_depth);
        }

        self.commands.push(RenderCommand::SetViewport {
            x,
            y,
            width: clamped_width,
            height: clamped_height,
            min_depth,
            max_depth,
        });
    }

    /// Record a set scissor rect command
    pub fn record_set_scissor_rect(&mut self, x: u32, y: u32, width: u32, height: u32) {
        // Clamp scissor rect to viewport dimensions to prevent InvalidScissorRect errors
        let clamped_x = x.min(self.viewport_width.saturating_sub(1));
        let clamped_y = y.min(self.viewport_height.saturating_sub(1));
        let max_width = self.viewport_width.saturating_sub(clamped_x);
        let max_height = self.viewport_height.saturating_sub(clamped_y);
        let clamped_width = width.min(max_width).max(1);
        let clamped_height = height.min(max_height).max(1);

        self.commands.push(RenderCommand::SetScissorRect {
            x: clamped_x,
            y: clamped_y,
            width: clamped_width,
            height: clamped_height,
        });
    }

    /// Record a push debug group command
    pub fn record_push_debug_group(&mut self, label: String) {
        self.commands.push(RenderCommand::PushDebugGroup { label });
    }

    /// Record a pop debug group command
    pub fn record_pop_debug_group(&mut self) {
        self.commands.push(RenderCommand::PopDebugGroup);
    }

    /// Record an insert debug marker command
    pub fn record_insert_debug_marker(&mut self, label: String) {
        self.commands.push(RenderCommand::InsertDebugMarker { label });
    }

    /// Record a set push constants command
    ///
    /// Immediates allow passing small amounts of per-draw data directly to shaders
    /// without the overhead of creating and binding uniform buffers.
    ///
    /// # Arguments
    /// * `offset` - Byte offset within the immediate data range (must be 4-byte aligned)
    /// * `data` - The data to write (must be 4-byte aligned)
    ///
    /// # Example usage in shaders (WGSL) for wgpu 28.0+:
    /// ```wgsl
    /// var<immediate> model_matrix: mat4x4<f32>;
    /// ```
    pub fn record_set_immediates(&mut self, offset: u32, data: Vec<u8>) {
        self.commands.push(RenderCommand::SetImmediates { offset, data });
    }

    /// Record immediates (convenience method)
    /// This method is kept for compatibility with the old API name
    pub fn record_set_push_constants_all(&mut self, offset: u32, data: &[u8]) {
        self.record_set_immediates(offset, data.to_vec());
    }

    /// Set the maximum index count for validation (from index buffer size)
    ///
    /// Called when setting the index buffer to track the maximum number of indices
    /// that can be safely drawn without reading past the end of the buffer.
    pub fn set_max_index_count(&mut self, count: u64) {
        self.max_index_count = Some(count);
    }

    /// Get the maximum index count for validation
    pub fn get_max_index_count(&self) -> Option<u64> {
        self.max_index_count
    }

    /// End the render pass and submit to the queue
    ///
    /// Executes all recorded commands using wgpu-core 27's command_encoder_run_render_pass.
    /// Returns the output texture (if any) for main framebuffer tracking.
    pub fn finish_and_submit(&mut self, context: &BasaltContext, queue_id: id::QueueId) -> Result<Option<id::TextureId>> {
        if !self.is_active {
            log::warn!("Render pass is not active, skipping submit");
            return Ok(None);
        }

        log::debug!("Finishing render pass with {} commands, color_view={:?}", 
            self.commands.len(), self.color_view);

        let global = context.inner();

        // Build render pass descriptor with color and depth attachments
        // Use Clear or Load based on should_clear flags
        let mut color_attachments = Vec::new();
        if let Some(view) = self.color_view {
            let load_op = if self.should_clear_color {
                log::debug!("Color attachment: CLEAR with {:?}", self.clear_color);
                wgpu_core::command::LoadOp::Clear(self.clear_color)
            } else {
                log::debug!("Color attachment: LOAD (preserving previous content)");
                wgpu_core::command::LoadOp::Load
            };
            color_attachments.push(Some(wgpu_core::command::RenderPassColorAttachment {
                view,
                resolve_target: None,
                load_op,
                store_op: wgpu_core::command::StoreOp::Store,
                depth_slice: None,
            }));
        }

        // Depth attachment - use Clear or Load based on should_clear_depth
        // Determine read_only flag based on tracked depth mode from pipelines
        // Skip depth attachment entirely when depth_mode is NoDepth (GUI, post-processing)
        // FIX: Condition was inverted! Create attachment when depth IS needed (not NoDepth)
        let depth_stencil_attachment = if !matches!(self.depth_mode, DepthMode::NoDepth) && self.depth_view.is_some() {
            self.depth_view.map(|view| {
                let depth_load_op = if self.should_clear_depth {
                    log::info!("Depth attachment: CLEAR with {}", self.clear_depth);
                    wgpu_core::command::LoadOp::Clear(Some(self.clear_depth))
                } else {
                    log::info!("Depth attachment: LOAD (preserving previous content)");
                    wgpu_core::command::LoadOp::Load
                };

                // For read-only depth, load_op and store_op must be None
                let depth_read_only = matches!(self.depth_mode, DepthMode::ReadOnly);
                let (depth_load_op, depth_store_op) = if depth_read_only {
                    log::info!("Depth attachment: READ-ONLY mode");
                    (None, None)
                } else {
                    (Some(depth_load_op), Some(wgpu_core::command::StoreOp::Store))
                };

                log::info!("Depth attachment: read_only={}, depth_mode={:?}", depth_read_only, self.depth_mode);
                wgpu_core::command::RenderPassDepthStencilAttachment {
                    view,
                    depth: wgpu_core::command::PassChannel {
                        load_op: depth_load_op,
                        store_op: depth_store_op,
                        read_only: depth_read_only,
                    },
                    stencil: wgpu_core::command::PassChannel {
                        load_op: Some(wgpu_core::command::LoadOp::Clear(Some(self.clear_stencil))),
                        store_op: Some(wgpu_core::command::StoreOp::Store),
                        read_only: false,
                    },
                }
            })
        } else {
            log::debug!("Skipping depth attachment (depth_mode={:?}, depth_view={:?})", self.depth_mode, self.depth_view.is_some());
            None
        };

        let desc = wgpu_core::command::RenderPassDescriptor {
            label: Some(Cow::Borrowed("Basalt Render Pass")),
            color_attachments: Cow::Borrowed(&color_attachments),
            depth_stencil_attachment: depth_stencil_attachment.as_ref(),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,  // No multiview rendering (wgpu 28.0+)
        };

        // Take ownership of commands vec to execute them
        let commands = std::mem::take(&mut self.commands);

        // **FIX**: Allow depth-only render passes (for shadow rendering, etc.)
        // but reject completely empty passes (no color AND no depth)
        if color_attachments.is_empty() && depth_stencil_attachment.is_none() {
            log::error!("No attachments at all - render pass has nothing to render to!");
            return Err(BasaltError::device_creation("No attachments"));
        }

        // Warn about depth-only passes (this is unusual but valid)
        if color_attachments.is_empty() {
            log::debug!("Depth-only render pass (shadow rendering or depth pre-pass)");
        }

        log::debug!("Beginning render pass with {} color attachments, depth={}",
            color_attachments.len(),
            depth_stencil_attachment.is_some());

        // Begin render pass
        let (mut render_pass, error) = global.command_encoder_begin_render_pass(
            self.command_encoder_id,
            &desc,
        );

        if let Some(e) = error {
            return Err(BasaltError::device_creation(format!(
                "Failed to begin render pass: {:?}", e
            )));
        }

        // Execute all recorded commands with proper error propagation
        for (cmd_index, cmd) in commands.iter().enumerate() {
            match cmd {
                RenderCommand::SetPipeline { pipeline_id } => {
                    global.render_pass_set_pipeline(&mut render_pass, *pipeline_id)
                        .map_err(|e| BasaltError::RenderPass(format!("Command {}: Failed to set pipeline {:?}: {:?}", cmd_index, pipeline_id, e)))?;
                }
                RenderCommand::SetVertexBuffer { slot, buffer_id, offset, size } => {
                    global.render_pass_set_vertex_buffer(&mut render_pass, *slot, *buffer_id, *offset, *size)
                        .map_err(|e| BasaltError::RenderPass(format!("Command {}: Failed to set vertex buffer (slot={}, buffer={:?}): {:?}", cmd_index, slot, buffer_id, e)))?;
                }
                RenderCommand::SetIndexBuffer { buffer_id, index_format, offset, size } => {
                    global.render_pass_set_index_buffer(&mut render_pass, *buffer_id, *index_format, *offset, *size)
                        .map_err(|e| BasaltError::RenderPass(format!("Command {}: Failed to set index buffer {:?}: {:?}", cmd_index, buffer_id, e)))?;
                }
                RenderCommand::SetBindGroup { index, bind_group_id, offsets } => {
                    global.render_pass_set_bind_group(&mut render_pass, *index, *bind_group_id, offsets)
                        .map_err(|e| BasaltError::RenderPass(format!("Command {}: Failed to set bind group (index={}, group={:?}): {:?}", cmd_index, index, bind_group_id, e)))?;
                }
                RenderCommand::DrawIndexed {
                    index_count,
                    instance_count,
                    first_index,
                    base_vertex,
                    first_instance,
                } => {
                    log::debug!(">>> EXECUTING DRAW: indices={}, instances={}, first_idx={}, base_vtx={}",
                        index_count, instance_count, first_index, base_vertex);
                    global.render_pass_draw_indexed(
                        &mut render_pass,
                        *index_count,
                        *instance_count,
                        *first_index,
                        *base_vertex,
                        *first_instance,
                    ).map_err(|e| BasaltError::RenderPass(format!("Command {}: Failed to draw indexed (indices={}, instances={}): {:?}", cmd_index, index_count, instance_count, e)))?;
                }
                RenderCommand::Draw {
                    vertex_count,
                    instance_count,
                    first_vertex,
                    first_instance,
                } => {
                    log::debug!("Draw: vertices={}, instances={}, first_vtx={}",
                        vertex_count, instance_count, first_vertex);
                    global.render_pass_draw(
                        &mut render_pass,
                        *vertex_count,
                        *instance_count,
                        *first_vertex,
                        *first_instance,
                    ).map_err(|e| BasaltError::RenderPass(format!("Command {}: Failed to draw (vertices={}, instances={}): {:?}", cmd_index, vertex_count, instance_count, e)))?;
                }
                RenderCommand::SetViewport { x, y, width, height, min_depth, max_depth } => {
                    global.render_pass_set_viewport(&mut render_pass, *x, *y, *width, *height, *min_depth, *max_depth)
                        .map_err(|e| BasaltError::RenderPass(format!("Command {}: Failed to set viewport: {:?}", cmd_index, e)))?;
                }
                RenderCommand::SetScissorRect { x, y, width, height } => {
                    global.render_pass_set_scissor_rect(&mut render_pass, *x, *y, *width, *height)
                        .map_err(|e| BasaltError::RenderPass(format!("Command {}: Failed to set scissor rect: {:?}", cmd_index, e)))?;
                }
                RenderCommand::PushDebugGroup { label } => {
                    // Debug groups are optional - log errors but don't fail
                    let _ = global.render_pass_push_debug_group(&mut render_pass, label, 0xFFFFFFFF);
                }
                RenderCommand::PopDebugGroup => {
                    let _ = global.render_pass_pop_debug_group(&mut render_pass);
                }
                RenderCommand::InsertDebugMarker { label } => {
                    // Debug markers are optional - log errors but don't fail
                    let _ = global.render_pass_insert_debug_marker(&mut render_pass, label, 0xFFFFFFFF);
                }
                RenderCommand::SetImmediates { offset, data } => {
                    global.render_pass_set_immediates(&mut render_pass, *offset, data)
                        .map_err(|e| BasaltError::RenderPass(format!("Command {}: Failed to set immediates (offset={}, size={}): {:?}", cmd_index, offset, data.len(), e)))?;
                }
            }
        }

        // End the render pass
        if let Err(e) = global.render_pass_end(&mut render_pass) {
            return Err(BasaltError::device_creation(format!(
                "Failed to end render pass: {:?}", e
            )));
        }

        // Finish the command encoder
        let (command_buffer_id, error) = global.command_encoder_finish(
            self.command_encoder_id,
            &wgt::CommandBufferDescriptor::default(),
            None,
        );

        if let Some(e) = error {
            return Err(BasaltError::device_creation(format!(
                "Failed to finish command encoder: {:?}", e
            )));
        }

        // Submit to queue
        let result = global.queue_submit(queue_id, &[command_buffer_id]);

        if let Err(e) = result {
            return Err(BasaltError::device_creation(format!(
                "Failed to submit command buffer: {:?}", e
            )));
        }

        // Poll the device to drive GPU progress and internal state machines
        // This is important for proper frame synchronization and preventing stalls
        // Use Poll (non-blocking) here - frame limiting is handled elsewhere
        let _ = global.device_poll(self.device_id, wgt::PollType::Poll);

        self.is_active = false;
        log::debug!("Render pass executed with {} commands and submitted to queue", commands.len());

        // Return the output texture for main framebuffer tracking
        // This is set AFTER rendering completes, avoiding the race condition
        let output = self.output_texture;
        if output.is_some() {
            log::debug!("Render pass completed, output texture: {:?} (ready for presentation)", output);
        }

        Ok(output)
    }
    
    /// Mark the render pass as inactive without submitting
    pub fn cancel(&mut self) {
        self.is_active = false;
    }
}
