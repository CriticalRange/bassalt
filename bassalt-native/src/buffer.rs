//! Buffer management

use wgpu_types as wgt;

/// Buffer descriptor for creating buffers
#[derive(Debug, Clone)]
pub struct BufferDescriptor {
    pub label: Option<String>,
    pub size: u64,
    pub usage: wgt::BufferUsages,
    pub mapped_at_creation: bool,
}

impl Default for BufferDescriptor {
    fn default() -> Self {
        Self {
            label: None,
            size: 0,
            usage: wgt::BufferUsages::empty(),
            mapped_at_creation: false,
        }
    }
}
