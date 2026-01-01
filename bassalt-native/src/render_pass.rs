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
    /// Set push constants for per-draw data
    /// This allows passing small amounts of per-draw data without rebinding
    SetPushConstants {
        /// Shader stages that can access this data
        stages: wgt::ShaderStages,
        /// Byte offset within the push constant range
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
}

impl RenderPassState {
    /// Create a new render pass with command recording
    pub fn new(
        context: Arc<BasaltContext>,
        device_id: id::DeviceId,
        queue_id: id::QueueId,
        color_view: Option<id::TextureViewId>,
        depth_view: Option<id::TextureViewId>,
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
            return Err(BasaltError::Device(format!("Failed to create command encoder: {:?}", e)));
        }

        // Convert clear color from u32 ARGB (Minecraft format) to wgt::Color
        let a = ((clear_color >> 24) & 0xFF) as f64 / 255.0;
        let r = ((clear_color >> 16) & 0xFF) as f64 / 255.0;
        let g = ((clear_color >> 8) & 0xFF) as f64 / 255.0;
        let b = (clear_color & 0xFF) as f64 / 255.0;

        Ok(Self {
            context,
            device_id,
            queue_id,
            command_encoder_id,
            color_view,
            depth_view,
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
        })
    }

    /// Get the command encoder ID
    pub fn encoder_id(&self) -> id::CommandEncoderId {
        self.command_encoder_id
    }

    /// Check if the render pass is active
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Record a set pipeline command
    pub fn record_set_pipeline(&mut self, pipeline_id: id::RenderPipelineId) {
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
        self.commands.push(RenderCommand::SetViewport {
            x,
            y,
            width,
            height,
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
    /// Push constants allow passing small amounts of per-draw data directly to shaders
    /// without the overhead of creating and binding uniform buffers.
    ///
    /// # Arguments
    /// * `stages` - Which shader stages can access this data
    /// * `offset` - Byte offset within the push constant range (must be 4-byte aligned)
    /// * `data` - The data to write (must be 4-byte aligned)
    ///
    /// # Example usage in shaders (WGSL):
    /// ```wgsl
    /// var<push_constant> model_matrix: mat4x4<f32>;
    /// ```
    pub fn record_set_push_constants(&mut self, stages: wgt::ShaderStages, offset: u32, data: Vec<u8>) {
        self.commands.push(RenderCommand::SetPushConstants { stages, offset, data });
    }

    /// Record push constants for vertex and fragment stages (convenience method)
    pub fn record_set_push_constants_all(&mut self, offset: u32, data: &[u8]) {
        self.record_set_push_constants(
            wgt::ShaderStages::VERTEX | wgt::ShaderStages::FRAGMENT,
            offset,
            data.to_vec(),
        );
    }

    /// End the render pass and submit to the queue
    ///
    /// Executes all recorded commands using wgpu-core 27's command_encoder_run_render_pass.
    pub fn finish_and_submit(&mut self, context: &BasaltContext, queue_id: id::QueueId) -> Result<()> {
        if !self.is_active {
            log::warn!("Render pass is not active, skipping submit");
            return Ok(());
        }

        log::info!("Finishing render pass with {} commands, color_view={:?}", 
            self.commands.len(), self.color_view);

        let global = context.inner();

        // Build render pass descriptor with color and depth attachments
        // Use Clear or Load based on should_clear flags
        let mut color_attachments = Vec::new();
        if let Some(view) = self.color_view {
            let load_op = if self.should_clear_color {
                log::info!("Color attachment: CLEAR with {:?}", self.clear_color);
                wgpu_core::command::LoadOp::Clear(self.clear_color)
            } else {
                log::info!("Color attachment: LOAD (preserving previous content)");
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
        let depth_stencil_attachment = self.depth_view.map(|view| {
            let depth_load_op = if self.should_clear_depth {
                log::info!("Depth attachment: CLEAR with {}", self.clear_depth);
                wgpu_core::command::LoadOp::Clear(Some(self.clear_depth))
            } else {
                log::info!("Depth attachment: LOAD (preserving previous content)");
                wgpu_core::command::LoadOp::Load
            };
            wgpu_core::command::RenderPassDepthStencilAttachment {
                view,
                depth: wgpu_core::command::PassChannel {
                    load_op: Some(depth_load_op),
                    store_op: Some(wgpu_core::command::StoreOp::Store),
                    read_only: false,
                },
                stencil: wgpu_core::command::PassChannel {
                    load_op: Some(wgpu_core::command::LoadOp::Clear(Some(self.clear_stencil))),
                    store_op: Some(wgpu_core::command::StoreOp::Store),
                    read_only: false,
                },
            }
        });

        let desc = wgpu_core::command::RenderPassDescriptor {
            label: Some(Cow::Borrowed("Basalt Render Pass")),
            color_attachments: Cow::Borrowed(&color_attachments),
            depth_stencil_attachment: depth_stencil_attachment.as_ref(),
            timestamp_writes: None,
            occlusion_query_set: None,
        };

        // Take ownership of commands vec to execute them
        let commands = std::mem::take(&mut self.commands);
        
        if color_attachments.is_empty() {
            log::error!("No color attachments - render pass has nothing to render to!");
            return Err(BasaltError::Device("No color attachment".into()));
        }

        log::info!("Beginning render pass with {} color attachments", color_attachments.len());

        // Begin render pass
        let (mut render_pass, error) = global.command_encoder_begin_render_pass(
            self.command_encoder_id,
            &desc,
        );

        if let Some(e) = error {
            return Err(BasaltError::Device(format!(
                "Failed to begin render pass: {:?}", e
            )));
        }

        // Execute all recorded commands
        for cmd in commands.iter() {
            match cmd {
                RenderCommand::SetPipeline { pipeline_id } => {
                    if let Err(e) = global.render_pass_set_pipeline(&mut render_pass, *pipeline_id) {
                        log::error!("Failed to set pipeline: {:?}", e);
                    }
                }
                RenderCommand::SetVertexBuffer { slot, buffer_id, offset, size } => {
                    if let Err(e) = global.render_pass_set_vertex_buffer(&mut render_pass, *slot, *buffer_id, *offset, *size) {
                        log::error!("Failed to set vertex buffer: {:?}", e);
                    }
                }
                RenderCommand::SetIndexBuffer { buffer_id, index_format, offset, size } => {
                    if let Err(e) = global.render_pass_set_index_buffer(&mut render_pass, *buffer_id, *index_format, *offset, *size) {
                        log::error!("Failed to set index buffer: {:?}", e);
                    }
                }
                RenderCommand::SetBindGroup { index, bind_group_id, offsets } => {
                    if let Err(e) = global.render_pass_set_bind_group(&mut render_pass, *index, *bind_group_id, offsets) {
                        log::error!("Failed to set bind group: {:?}", e);
                    }
                }
                RenderCommand::DrawIndexed {
                    index_count,
                    instance_count,
                    first_index,
                    base_vertex,
                    first_instance,
                } => {
                    log::info!("DrawIndexed: indices={}, instances={}, first_idx={}, base_vtx={}", 
                        index_count, instance_count, first_index, base_vertex);
                    if let Err(e) = global.render_pass_draw_indexed(
                        &mut render_pass,
                        *index_count,
                        *instance_count,
                        *first_index,
                        *base_vertex,
                        *first_instance,
                    ) {
                        log::error!("Failed to draw indexed: {:?}", e);
                    }
                }
                RenderCommand::Draw {
                    vertex_count,
                    instance_count,
                    first_vertex,
                    first_instance,
                } => {
                    log::debug!("Draw: vertices={}, instances={}, first_vtx={}", 
                        vertex_count, instance_count, first_vertex);
                    if let Err(e) = global.render_pass_draw(
                        &mut render_pass,
                        *vertex_count,
                        *instance_count,
                        *first_vertex,
                        *first_instance,
                    ) {
                        log::error!("Failed to draw: {:?}", e);
                    }
                }
                RenderCommand::SetViewport { x, y, width, height, min_depth, max_depth } => {
                    if let Err(e) = global.render_pass_set_viewport(&mut render_pass, *x, *y, *width, *height, *min_depth, *max_depth) {
                        log::error!("Failed to set viewport: {:?}", e);
                    }
                }
                RenderCommand::SetScissorRect { x, y, width, height } => {
                    if let Err(e) = global.render_pass_set_scissor_rect(&mut render_pass, *x, *y, *width, *height) {
                        log::error!("Failed to set scissor rect: {:?}", e);
                    }
                }
                RenderCommand::PushDebugGroup { label } => {
                    // Use white color (0xFFFFFFFF) for debug groups
                    let _ = global.render_pass_push_debug_group(&mut render_pass, label, 0xFFFFFFFF);
                }
                RenderCommand::PopDebugGroup => {
                    let _ = global.render_pass_pop_debug_group(&mut render_pass);
                }
                RenderCommand::InsertDebugMarker { label } => {
                    // Use white color (0xFFFFFFFF) for debug markers
                    let _ = global.render_pass_insert_debug_marker(&mut render_pass, label, 0xFFFFFFFF);
                }
                RenderCommand::SetPushConstants { stages, offset, data } => {
                    if let Err(e) = global.render_pass_set_push_constants(&mut render_pass, *stages, *offset, data) {
                        log::error!("Failed to set push constants: {:?}", e);
                    }
                }
            }
        }

        // End the render pass
        if let Err(e) = global.render_pass_end(&mut render_pass) {
            return Err(BasaltError::Device(format!(
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
            return Err(BasaltError::Device(format!(
                "Failed to finish command encoder: {:?}", e
            )));
        }

        // Submit to queue
        let result = global.queue_submit(queue_id, &[command_buffer_id]);

        if let Err(e) = result {
            return Err(BasaltError::Device(format!(
                "Failed to submit command buffer: {:?}", e
            )));
        }

        self.is_active = false;
        log::debug!("Render pass executed with {} commands and submitted to queue", commands.len());
        Ok(())
    }
    
    /// Mark the render pass as inactive without submitting
    pub fn cancel(&mut self) {
        self.is_active = false;
    }
}
