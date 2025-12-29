//! Range-based buffer allocation for efficient GPU memory packing
//!
//! This module provides a range allocator that packs multiple allocations
//! into a single large GPU buffer, reducing buffer count and improving
//! batching efficiency.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use range_alloc::RangeAllocator;
use wgpu_core::id;
use wgpu_types as wgt;

use crate::context::BasaltContext;
use crate::error::{BasaltError, Result};

/// A handle to an allocation within a managed buffer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AllocationHandle(u64);

impl AllocationHandle {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn id(&self) -> u64 {
        self.0
    }
}

/// Information about a specific allocation
#[derive(Debug, Clone)]
pub struct AllocationInfo {
    /// Offset within the buffer
    pub offset: u64,
    /// Size of the allocation
    pub size: u64,
    /// The buffer this allocation belongs to
    pub buffer_id: id::BufferId,
}

/// A managed buffer pool that uses range allocation for efficient packing
pub struct BufferPool {
    context: Arc<BasaltContext>,
    device_id: id::DeviceId,

    /// The underlying GPU buffer
    buffer_id: id::BufferId,

    /// Total size of the buffer
    total_size: u64,

    /// Buffer usage flags
    usage: wgt::BufferUsages,

    /// Range allocator for managing free/used ranges
    allocator: RwLock<RangeAllocator<u64>>,

    /// Map of allocation handles to their info
    allocations: RwLock<HashMap<u64, AllocationInfo>>,

    /// Counter for generating unique allocation handles
    next_handle_id: RwLock<u64>,

    /// Minimum alignment for allocations (usually 256 bytes for uniform buffers)
    alignment: u64,
}

impl BufferPool {
    /// Create a new buffer pool with the specified size and usage
    pub fn new(
        context: Arc<BasaltContext>,
        device_id: id::DeviceId,
        queue_id: id::QueueId,
        size: u64,
        usage: wgt::BufferUsages,
        alignment: u64,
        label: &str,
    ) -> Result<Self> {
        // Ensure size is aligned
        let aligned_size = (size + alignment - 1) & !(alignment - 1);

        // Create the underlying GPU buffer
        let desc = wgt::BufferDescriptor {
            label: Some(Cow::Borrowed(label)),
            size: aligned_size,
            usage: usage | wgt::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        };

        let (buffer_id, error) = context
            .inner()
            .device_create_buffer(device_id, &desc, None);

        if let Some(e) = error {
            return Err(BasaltError::Wgpu(format!("Failed to create buffer pool: {:?}", e)));
        }

        // Initialize range allocator
        let allocator = RangeAllocator::new(0..aligned_size);

        log::info!(
            "Created buffer pool '{}': {} bytes with {:?} usage, {} byte alignment",
            label, aligned_size, usage, alignment
        );

        Ok(Self {
            context,
            device_id,
            buffer_id,
            total_size: aligned_size,
            usage,
            allocator: RwLock::new(allocator),
            allocations: RwLock::new(HashMap::new()),
            next_handle_id: RwLock::new(0),
            alignment,
        })
    }

    /// Allocate a range within the buffer pool
    pub fn allocate(&self, size: u64) -> Result<AllocationHandle> {
        // Align the size
        let aligned_size = (size + self.alignment - 1) & !(self.alignment - 1);

        let mut allocator = self.allocator.write();

        // Try to allocate
        let range = allocator.allocate_range(aligned_size).map_err(|_| {
            BasaltError::OutOfMemory(format!(
                "Buffer pool exhausted: requested {} bytes, total {} bytes",
                aligned_size, self.total_size
            ))
        })?;

        // Generate handle
        let mut next_id = self.next_handle_id.write();
        let handle_id = *next_id;
        *next_id += 1;

        // Store allocation info
        let info = AllocationInfo {
            offset: range.start,
            size: aligned_size,
            buffer_id: self.buffer_id,
        };

        self.allocations.write().insert(handle_id, info);

        log::debug!(
            "Allocated {} bytes at offset {} (handle {})",
            aligned_size, range.start, handle_id
        );

        Ok(AllocationHandle::new(handle_id))
    }

    /// Free a previously allocated range
    pub fn free(&self, handle: AllocationHandle) -> Result<()> {
        let info = self.allocations.write().remove(&handle.id())
            .ok_or_else(|| BasaltError::InvalidParameter(
                format!("Invalid allocation handle: {}", handle.id())
            ))?;

        self.allocator.write().free_range(info.offset..info.offset + info.size);

        log::debug!(
            "Freed {} bytes at offset {} (handle {})",
            info.size, info.offset, handle.id()
        );

        Ok(())
    }

    /// Get information about an allocation
    pub fn get_info(&self, handle: AllocationHandle) -> Option<AllocationInfo> {
        self.allocations.read().get(&handle.id()).cloned()
    }

    /// Write data to an allocation
    pub fn write(&self, queue_id: id::QueueId, handle: AllocationHandle, data: &[u8]) -> Result<()> {
        let info = self.allocations.read().get(&handle.id()).cloned()
            .ok_or_else(|| BasaltError::InvalidParameter(
                format!("Invalid allocation handle: {}", handle.id())
            ))?;

        if data.len() as u64 > info.size {
            return Err(BasaltError::InvalidParameter(format!(
                "Data size {} exceeds allocation size {}",
                data.len(), info.size
            )));
        }

        self.context
            .inner()
            .queue_write_buffer(queue_id, self.buffer_id, info.offset, data)
            .map_err(|e| BasaltError::Wgpu(format!("{:?}", e)))?;

        Ok(())
    }

    /// Get the underlying buffer ID
    pub fn buffer_id(&self) -> id::BufferId {
        self.buffer_id
    }

    /// Get total size of the pool
    pub fn total_size(&self) -> u64 {
        self.total_size
    }

    /// Get the number of active allocations
    pub fn allocation_count(&self) -> usize {
        self.allocations.read().len()
    }

    /// Get the amount of free space available
    pub fn free_space(&self) -> u64 {
        let allocator = self.allocator.read();
        // Sum up all free ranges
        self.total_size - self.allocations.read().values().map(|a| a.size).sum::<u64>()
    }
}

impl Drop for BufferPool {
    fn drop(&mut self) {
        self.context.inner().buffer_drop(self.buffer_id);
        log::debug!("Dropped buffer pool with {} bytes", self.total_size);
    }
}

/// Manager for multiple buffer pools, organized by usage type
pub struct BufferPoolManager {
    context: Arc<BasaltContext>,
    device_id: id::DeviceId,
    queue_id: id::QueueId,

    /// Pool for vertex data
    vertex_pool: Option<BufferPool>,

    /// Pool for index data
    index_pool: Option<BufferPool>,

    /// Pool for uniform data
    uniform_pool: Option<BufferPool>,

    /// Pool for storage data
    storage_pool: Option<BufferPool>,
}

impl BufferPoolManager {
    /// Default vertex pool size (64 MB)
    pub const DEFAULT_VERTEX_POOL_SIZE: u64 = 64 * 1024 * 1024;

    /// Default index pool size (16 MB)
    pub const DEFAULT_INDEX_POOL_SIZE: u64 = 16 * 1024 * 1024;

    /// Default uniform pool size (4 MB)
    pub const DEFAULT_UNIFORM_POOL_SIZE: u64 = 4 * 1024 * 1024;

    /// Default storage pool size (32 MB)
    pub const DEFAULT_STORAGE_POOL_SIZE: u64 = 32 * 1024 * 1024;

    /// Create a new buffer pool manager with default pool sizes
    pub fn new(
        context: Arc<BasaltContext>,
        device_id: id::DeviceId,
        queue_id: id::QueueId,
    ) -> Result<Self> {
        Self::with_sizes(
            context,
            device_id,
            queue_id,
            Self::DEFAULT_VERTEX_POOL_SIZE,
            Self::DEFAULT_INDEX_POOL_SIZE,
            Self::DEFAULT_UNIFORM_POOL_SIZE,
            Self::DEFAULT_STORAGE_POOL_SIZE,
        )
    }

    /// Create with custom pool sizes
    pub fn with_sizes(
        context: Arc<BasaltContext>,
        device_id: id::DeviceId,
        queue_id: id::QueueId,
        vertex_size: u64,
        index_size: u64,
        uniform_size: u64,
        storage_size: u64,
    ) -> Result<Self> {
        let vertex_pool = BufferPool::new(
            context.clone(),
            device_id,
            queue_id,
            vertex_size,
            wgt::BufferUsages::VERTEX | wgt::BufferUsages::COPY_DST,
            4, // 4-byte alignment for vertices
            "Bassalt Vertex Pool",
        )?;

        let index_pool = BufferPool::new(
            context.clone(),
            device_id,
            queue_id,
            index_size,
            wgt::BufferUsages::INDEX | wgt::BufferUsages::COPY_DST,
            4, // 4-byte alignment for indices
            "Bassalt Index Pool",
        )?;

        let uniform_pool = BufferPool::new(
            context.clone(),
            device_id,
            queue_id,
            uniform_size,
            wgt::BufferUsages::UNIFORM | wgt::BufferUsages::COPY_DST,
            256, // 256-byte alignment for uniforms (WebGPU requirement)
            "Bassalt Uniform Pool",
        )?;

        let storage_pool = BufferPool::new(
            context.clone(),
            device_id,
            queue_id,
            storage_size,
            wgt::BufferUsages::STORAGE | wgt::BufferUsages::COPY_DST,
            256, // 256-byte alignment for storage
            "Bassalt Storage Pool",
        )?;

        log::info!("Created buffer pool manager with {} MB total",
            (vertex_size + index_size + uniform_size + storage_size) / (1024 * 1024));

        Ok(Self {
            context,
            device_id,
            queue_id,
            vertex_pool: Some(vertex_pool),
            index_pool: Some(index_pool),
            uniform_pool: Some(uniform_pool),
            storage_pool: Some(storage_pool),
        })
    }

    /// Get the vertex buffer pool
    pub fn vertex_pool(&self) -> Option<&BufferPool> {
        self.vertex_pool.as_ref()
    }

    /// Get the index buffer pool
    pub fn index_pool(&self) -> Option<&BufferPool> {
        self.index_pool.as_ref()
    }

    /// Get the uniform buffer pool
    pub fn uniform_pool(&self) -> Option<&BufferPool> {
        self.uniform_pool.as_ref()
    }

    /// Get the storage buffer pool
    pub fn storage_pool(&self) -> Option<&BufferPool> {
        self.storage_pool.as_ref()
    }

    /// Allocate from the appropriate pool based on usage flags
    pub fn allocate(&self, size: u64, usage: wgt::BufferUsages) -> Result<(AllocationHandle, id::BufferId, u64)> {
        let pool = if usage.contains(wgt::BufferUsages::VERTEX) {
            self.vertex_pool.as_ref()
        } else if usage.contains(wgt::BufferUsages::INDEX) {
            self.index_pool.as_ref()
        } else if usage.contains(wgt::BufferUsages::UNIFORM) {
            self.uniform_pool.as_ref()
        } else if usage.contains(wgt::BufferUsages::STORAGE) {
            self.storage_pool.as_ref()
        } else {
            None
        };

        match pool {
            Some(p) => {
                let handle = p.allocate(size)?;
                let info = p.get_info(handle).unwrap();
                Ok((handle, info.buffer_id, info.offset))
            }
            None => Err(BasaltError::InvalidParameter(format!(
                "No pool available for usage {:?}", usage
            ))),
        }
    }

    /// Write data to an allocation
    pub fn write(&self, handle: AllocationHandle, usage: wgt::BufferUsages, data: &[u8]) -> Result<()> {
        let pool = if usage.contains(wgt::BufferUsages::VERTEX) {
            self.vertex_pool.as_ref()
        } else if usage.contains(wgt::BufferUsages::INDEX) {
            self.index_pool.as_ref()
        } else if usage.contains(wgt::BufferUsages::UNIFORM) {
            self.uniform_pool.as_ref()
        } else if usage.contains(wgt::BufferUsages::STORAGE) {
            self.storage_pool.as_ref()
        } else {
            None
        };

        match pool {
            Some(p) => p.write(self.queue_id, handle, data),
            None => Err(BasaltError::InvalidParameter(format!(
                "No pool available for usage {:?}", usage
            ))),
        }
    }

    /// Free an allocation from the appropriate pool
    pub fn free(&self, handle: AllocationHandle, usage: wgt::BufferUsages) -> Result<()> {
        let pool = if usage.contains(wgt::BufferUsages::VERTEX) {
            self.vertex_pool.as_ref()
        } else if usage.contains(wgt::BufferUsages::INDEX) {
            self.index_pool.as_ref()
        } else if usage.contains(wgt::BufferUsages::UNIFORM) {
            self.uniform_pool.as_ref()
        } else if usage.contains(wgt::BufferUsages::STORAGE) {
            self.storage_pool.as_ref()
        } else {
            None
        };

        match pool {
            Some(p) => p.free(handle),
            None => Err(BasaltError::InvalidParameter(format!(
                "No pool available for usage {:?}", usage
            ))),
        }
    }
}
