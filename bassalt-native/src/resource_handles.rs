//! Resource handle management for JNI
//!
//! Manages mapping between Java jlong handles and wgpu resource IDs.
//! Since wgpu-core 27 uses NonZeroU64-based RawId that can't be directly
//! cast to jlong, we maintain separate handle stores for each resource type.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use parking_lot::RwLock;
use wgpu_core::id;

/// Buffer info stored alongside ID
#[derive(Debug, Clone, Copy)]
pub struct BufferInfo {
    pub id: id::BufferId,
    pub size: u64,
}

/// Thread-safe handle store for wgpu resources
pub struct ResourceHandleStore {
    next_handle: AtomicU64,
    buffers: RwLock<HashMap<u64, BufferInfo>>,
    textures: RwLock<HashMap<u64, id::TextureId>>,
    texture_views: RwLock<HashMap<u64, id::TextureViewId>>,
    samplers: RwLock<HashMap<u64, id::SamplerId>>,
    bind_groups: RwLock<HashMap<u64, id::BindGroupId>>,
    bind_group_layouts: RwLock<HashMap<u64, id::BindGroupLayoutId>>,
    render_pipelines: RwLock<HashMap<u64, id::RenderPipelineId>>,
    command_encoders: RwLock<HashMap<u64, id::CommandEncoderId>>,
}

impl ResourceHandleStore {
    pub fn new() -> Self {
        Self {
            next_handle: AtomicU64::new(1), // Start at 1 so 0 can indicate null
            buffers: RwLock::new(HashMap::new()),
            textures: RwLock::new(HashMap::new()),
            texture_views: RwLock::new(HashMap::new()),
            samplers: RwLock::new(HashMap::new()),
            bind_groups: RwLock::new(HashMap::new()),
            bind_group_layouts: RwLock::new(HashMap::new()),
            render_pipelines: RwLock::new(HashMap::new()),
            command_encoders: RwLock::new(HashMap::new()),
        }
    }

    fn next(&self) -> u64 {
        self.next_handle.fetch_add(1, Ordering::Relaxed)
    }

    // Buffer operations
    pub fn insert_buffer(&self, buffer_id: id::BufferId, size: u64) -> u64 {
        let handle = self.next();
        let info = BufferInfo { id: buffer_id, size };
        self.buffers.write().insert(handle, info);
        handle
    }

    pub fn get_buffer(&self, handle: u64) -> Option<id::BufferId> {
        self.buffers.read().get(&handle).map(|info| info.id)
    }

    pub fn get_buffer_info(&self, handle: u64) -> Option<BufferInfo> {
        self.buffers.read().get(&handle).copied()
    }

    pub fn remove_buffer(&self, handle: u64) -> Option<id::BufferId> {
        self.buffers.write().remove(&handle).map(|info| info.id)
    }

    // Texture operations
    pub fn insert_texture(&self, texture_id: id::TextureId) -> u64 {
        let handle = self.next();
        self.textures.write().insert(handle, texture_id);
        handle
    }

    pub fn get_texture(&self, handle: u64) -> Option<id::TextureId> {
        self.textures.read().get(&handle).copied()
    }

    pub fn remove_texture(&self, handle: u64) -> Option<id::TextureId> {
        self.textures.write().remove(&handle)
    }

    // Texture view operations
    pub fn insert_texture_view(&self, view_id: id::TextureViewId) -> u64 {
        let handle = self.next();
        self.texture_views.write().insert(handle, view_id);
        handle
    }

    pub fn get_texture_view(&self, handle: u64) -> Option<id::TextureViewId> {
        self.texture_views.read().get(&handle).copied()
    }

    pub fn remove_texture_view(&self, handle: u64) -> Option<id::TextureViewId> {
        self.texture_views.write().remove(&handle)
    }

    // Sampler operations
    pub fn insert_sampler(&self, sampler_id: id::SamplerId) -> u64 {
        let handle = self.next();
        self.samplers.write().insert(handle, sampler_id);
        handle
    }

    pub fn get_sampler(&self, handle: u64) -> Option<id::SamplerId> {
        self.samplers.read().get(&handle).copied()
    }

    pub fn remove_sampler(&self, handle: u64) -> Option<id::SamplerId> {
        self.samplers.write().remove(&handle)
    }

    // Bind group operations
    pub fn insert_bind_group(&self, bind_group_id: id::BindGroupId) -> u64 {
        let handle = self.next();
        self.bind_groups.write().insert(handle, bind_group_id);
        handle
    }

    pub fn get_bind_group(&self, handle: u64) -> Option<id::BindGroupId> {
        self.bind_groups.read().get(&handle).copied()
    }

    pub fn remove_bind_group(&self, handle: u64) -> Option<id::BindGroupId> {
        self.bind_groups.write().remove(&handle)
    }

    // Bind group layout operations
    pub fn insert_bind_group_layout(&self, layout_id: id::BindGroupLayoutId) -> u64 {
        let handle = self.next();
        self.bind_group_layouts.write().insert(handle, layout_id);
        handle
    }

    pub fn get_bind_group_layout(&self, handle: u64) -> Option<id::BindGroupLayoutId> {
        self.bind_group_layouts.read().get(&handle).copied()
    }

    pub fn remove_bind_group_layout(&self, handle: u64) -> Option<id::BindGroupLayoutId> {
        self.bind_group_layouts.write().remove(&handle)
    }

    // Render pipeline operations
    pub fn insert_render_pipeline(&self, pipeline_id: id::RenderPipelineId) -> u64 {
        let handle = self.next();
        self.render_pipelines.write().insert(handle, pipeline_id);
        handle
    }

    pub fn get_render_pipeline(&self, handle: u64) -> Option<id::RenderPipelineId> {
        self.render_pipelines.read().get(&handle).copied()
    }

    pub fn remove_render_pipeline(&self, handle: u64) -> Option<id::RenderPipelineId> {
        self.render_pipelines.write().remove(&handle)
    }

    // Command encoder operations
    pub fn insert_command_encoder(&self, encoder_id: id::CommandEncoderId) -> u64 {
        let handle = self.next();
        self.command_encoders.write().insert(handle, encoder_id);
        handle
    }

    pub fn get_command_encoder(&self, handle: u64) -> Option<id::CommandEncoderId> {
        self.command_encoders.read().get(&handle).copied()
    }

    pub fn remove_command_encoder(&self, handle: u64) -> Option<id::CommandEncoderId> {
        self.command_encoders.write().remove(&handle)
    }
}

impl Default for ResourceHandleStore {
    fn default() -> Self {
        Self::new()
    }
}

// Global handle store - one per device would be cleaner but this is simpler for now
lazy_static::lazy_static! {
    pub static ref HANDLES: ResourceHandleStore = ResourceHandleStore::new();
}
