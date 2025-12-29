# Patterns to Adopt from wgpu-mc

This document outlines performance patterns and optimizations observed in the wgpu-mc project
that could benefit bassalt-native.

## 1. Zero-Copy Array Access

**Problem**: Current `convert_byte_array()` copies data from Java to Rust.

**wgpu-mc Pattern**: Uses `get_array_elements_critical()` for zero-copy access.

```rust
// wgpu-mc approach (fast, zero-copy)
let elements = unsafe {
    env.get_array_elements_critical(&array, ReleaseMode::NoCopyBack)
}.unwrap();
let slice = unsafe {
    slice::from_raw_parts(elements.as_ptr(), elements.len())
};
```

**Recommended Implementation**:
```rust
// Add to bassalt-native/src/jni/arrays.rs
use jni::objects::{AutoElements, JByteArray, ReleaseMode};
use std::slice;

/// Zero-copy access to Java byte array for read-only operations.
/// SAFETY: The returned slice is only valid while `elements` is in scope.
pub unsafe fn get_byte_array_critical<'a>(
    env: &mut JNIEnv<'a>,
    array: &JByteArray,
) -> Result<(AutoElements<'a, 'a, 'a, jbyte>, &'a [u8]), String> {
    let elements = env
        .get_array_elements_critical(array, ReleaseMode::NoCopyBack)
        .map_err(|e| format!("Failed to get array elements: {}", e))?;
    let len = elements.len();
    let ptr = elements.as_ptr() as *const u8;
    let slice = slice::from_raw_parts(ptr, len);
    Ok((elements, slice))
}
```

**Use Cases**:
- Chunk mesh data uploads
- Texture data uploads
- Large buffer writes

---

## 2. Task Channel Pattern for Async Work

**Problem**: Current JNI calls are synchronous, blocking the Java thread.

**wgpu-mc Pattern**: Uses crossbeam channels to queue work for background execution.

```rust
// wgpu-mc approach
use crossbeam_channel::{unbounded, Sender, Receiver};

type Task = Box<dyn FnOnce() + Send + Sync>;
static TASK_CHANNELS: Lazy<(Sender<Task>, Receiver<Task>)> = Lazy::new(unbounded);

// Queue work from JNI
TASK_CHANNELS.0.send(Box::new(|| {
    // Expensive GPU operation
})).unwrap();
```

**Recommended Implementation**:
```rust
// Add to bassalt-native/src/task_queue.rs
use crossbeam_channel::{unbounded, Sender, Receiver};
use once_cell::sync::Lazy;
use std::thread;

type Task = Box<dyn FnOnce() + Send>;

pub struct TaskQueue {
    sender: Sender<Task>,
}

static TASK_QUEUE: Lazy<TaskQueue> = Lazy::new(|| {
    let (sender, receiver) = unbounded::<Task>();

    // Spawn worker thread
    thread::spawn(move || {
        while let Ok(task) = receiver.recv() {
            task();
        }
    });

    TaskQueue { sender }
});

impl TaskQueue {
    pub fn schedule<F: FnOnce() + Send + 'static>(task: F) {
        let _ = TASK_QUEUE.sender.send(Box::new(task));
    }
}
```

**Use Cases**:
- Chunk baking/meshing
- Texture loading
- Pipeline compilation

---

## 3. Command Batching

**Problem**: Each draw call goes through JNI individually.

**wgpu-mc Pattern**: Batches commands before submission.

**Recommended Implementation**:
```rust
// Add to bassalt-native/src/command_batch.rs
use smallvec::SmallVec;

pub struct CommandBatch {
    draws: SmallVec<[DrawCall; 64]>,
    buffer_writes: SmallVec<[(BufferId, u64, Vec<u8>); 16]>,
}

impl CommandBatch {
    pub fn new() -> Self {
        Self {
            draws: SmallVec::new(),
            buffer_writes: SmallVec::new(),
        }
    }

    pub fn add_draw(&mut self, call: DrawCall) {
        self.draws.push(call);
    }

    pub fn add_buffer_write(&mut self, buffer: BufferId, offset: u64, data: Vec<u8>) {
        self.buffer_writes.push((buffer, offset, data));
    }

    pub fn submit(self, device: &BasaltDevice) -> Result<()> {
        // Submit all buffer writes first
        for (buffer, offset, data) in self.buffer_writes {
            device.write_buffer(buffer, offset, &data)?;
        }

        // Then execute all draws in a single render pass
        // ...
        Ok(())
    }
}
```

---

## 4. Resource Pooling

**Problem**: Frequent buffer/texture allocation causes fragmentation.

**wgpu-mc Pattern**: Uses range allocators to pack data into large buffers.

**Already Implemented**: See `range_allocator.rs` and `atlas.rs`.

**Additional Optimization**:
```rust
// Add buffer recycling for temporary buffers
pub struct StagingBufferPool {
    available: Vec<(BufferId, u64)>, // (id, size)
    min_size: u64,
}

impl StagingBufferPool {
    pub fn acquire(&mut self, device: &BasaltDevice, size: u64) -> BufferId {
        // Find existing buffer of sufficient size
        if let Some(pos) = self.available.iter().position(|(_, s)| *s >= size) {
            return self.available.remove(pos).0;
        }
        // Create new buffer
        device.create_buffer(size.max(self.min_size), COPY_SRC | MAP_WRITE).unwrap()
    }

    pub fn release(&mut self, buffer: BufferId, size: u64) {
        self.available.push((buffer, size));
    }
}
```

---

## 5. Pipeline Caching

**Problem**: Repeated pipeline creation for same parameters.

**wgpu-mc Pattern**: Caches compiled pipelines by descriptor.

**Recommended Implementation**:
```rust
// Add to bassalt-native/src/pipeline_cache.rs
use hashbrown::HashMap;
use parking_lot::RwLock;

#[derive(Hash, Eq, PartialEq, Clone)]
pub struct PipelineKey {
    pub vertex_format: u32,
    pub blend_mode: u32,
    pub depth_test: bool,
    pub depth_write: bool,
    // ... other discriminants
}

pub struct PipelineCache {
    cache: RwLock<HashMap<PipelineKey, id::RenderPipelineId>>,
}

impl PipelineCache {
    pub fn get_or_create(
        &self,
        device: &BasaltDevice,
        key: &PipelineKey,
        create_fn: impl FnOnce() -> Result<id::RenderPipelineId>,
    ) -> Result<id::RenderPipelineId> {
        // Check cache first
        if let Some(id) = self.cache.read().get(key) {
            return Ok(*id);
        }

        // Create and cache
        let id = create_fn()?;
        self.cache.write().insert(key.clone(), id);
        Ok(id)
    }
}
```

---

## 6. Frustum Culling

**Problem**: Rendering chunks that aren't visible.

**wgpu-mc Pattern**: Uses `treeculler` crate for efficient frustum culling.

**Recommended Implementation**:
```toml
# Add to Cargo.toml
treeculler = "0.4"
```

```rust
// Add to bassalt-native/src/culling.rs
use treeculler::{BVol, Frustum, AABB};

pub struct ChunkCuller {
    frustum: Frustum,
}

impl ChunkCuller {
    pub fn update_frustum(&mut self, view_proj: &Mat4) {
        self.frustum = Frustum::from_modelview_projection(view_proj.to_cols_array());
    }

    pub fn is_visible(&self, chunk_pos: IVec3) -> bool {
        let min = chunk_pos.as_vec3() * 16.0;
        let max = min + Vec3::splat(16.0);
        let aabb = AABB::new(min.into(), max.into());
        self.frustum.test_bounding_volume(&aabb).is_visible()
    }
}
```

---

## Priority Order

1. **High**: Zero-copy array access (biggest performance win for data uploads)
2. **High**: Pipeline caching (reduces shader compilation hitches)
3. **Medium**: Command batching (reduces JNI overhead)
4. **Medium**: Frustum culling (reduces draw calls)
5. **Low**: Task queue (useful for async chunk loading)
6. **Low**: Staging buffer pool (reduces allocator pressure)

---

## References

- wgpu-mc source: `~/wgpu-mc/rust/wgpu-mc-jni/src/`
- wgpu-mc patterns analysis: Conducted 2024-12-29
- wgpu v27 API documentation: `~/wgpu/` (v27 branch)
