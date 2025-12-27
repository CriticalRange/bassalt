//! GPU adapter handling

use std::sync::Arc;
use wgpu_core::id;
use wgpu_types as wgt;
use crate::context::BasaltContext;
use crate::error::Result;

/// Wrapper for a GPU adapter
pub struct BasaltAdapter {
    context: Arc<BasaltContext>,
    adapter_id: id::AdapterId,
    info: wgt::AdapterInfo,
}

impl BasaltAdapter {
    /// Create a new adapter wrapper
    pub fn new(context: Arc<BasaltContext>, adapter_id: id::AdapterId, info: wgt::AdapterInfo) -> Self {
        Self {
            context,
            adapter_id,
            info,
        }
    }

    /// Get the adapter ID
    pub fn id(&self) -> id::AdapterId {
        self.adapter_id
    }

    /// Get the adapter info
    pub fn info(&self) -> &wgt::AdapterInfo {
        &self.info
    }

    /// Request a device from this adapter
    pub fn request_device(
        &self,
        desc: &wgt::DeviceDescriptor,
    ) -> Result<(id::DeviceId, id::QueueId)> {
        let (device_id, error) = self
            .context
            .inner()
            .request_device(self.adapter_id, desc, None);

        let device_id = device_id.ok_or_else(|| {
            error.map_or_else(
                || crate::error::BasaltError::Device("Unknown device error".into()),
                |e| crate::error::BasaltError::Device(format!("{:?}", e)),
            )
        })?;

        let queue_id = self
            .context
            .inner()
            .device_get_queue(device_id);

        Ok((device_id, queue_id))
    }
}
