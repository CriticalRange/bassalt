//! GPU adapter handling

use std::borrow::Cow;
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
        desc: &wgt::DeviceDescriptor<Option<Cow<'_, str>>>,
    ) -> Result<(id::DeviceId, id::QueueId)> {
        let (device_id, queue_id) = self
            .context
            .inner()
            .adapter_request_device(self.adapter_id, desc, None, None)
            .map_err(|e| crate::error::BasaltError::device_creation(format!("{:?}", e)))?;

        Ok((device_id, queue_id))
    }
}
