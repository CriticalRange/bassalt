//! Timestamp query support for GPU profiling
//!
//! Timestamp queries allow measuring GPU execution time for operations.
//! This is useful for profiling and optimizing rendering performance.
//!
//! Based on wgpu example: examples/features/src/timestamp_queries/mod.rs
//!
//! # Timestamp Query Types
//!
//! wgpu supports three types of timestamp queries:
//!
//! 1. **Pass-level timestamps** (`Features::TIMESTAMP_QUERY`)
//!    - Written at the beginning and end of render/compute passes
//!    - Uses `RenderPassTimestampWrites` / `ComputePassTimestampWrites`
//!
//! 2. **Encoder-level timestamps** (`Features::TIMESTAMP_QUERY_INSIDE_ENCODERS`)
//!    - Written between any commands in a command encoder
//!    - Uses `CommandEncoder::write_timestamp`
//!
//! 3. **Pass-internal timestamps** (`Features::TIMESTAMP_QUERY_INSIDE_PASSES`)
//!    - Written within a render/compute pass
//!    - Uses `RenderPass::write_timestamp` / `ComputePass::write_timestamp`
//!
//! # Usage
//!
//! ```rust,no_run
//! use bassalt_native::timestamp_queries::TimestampQuerySet;
//!
//! // Create a query set
//! let queries = TimestampQuerySet::new(&context, device_id, 8)?;
//!
//! // Record timestamps during command encoding
//! queries.write_timestamp(0)?;
//! // ... do some work ...
//! queries.write_timestamp(1)?;
//!
//! // Resolve and read the timestamps
//! let timestamps = queries.resolve_and_read(&context, device_id, 0..2)?;
//! let duration_ns = timestamps[1].wrapping_sub(timestamps[0]);
//! ```

use std::borrow::Cow;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use wgpu_core::id;
use wgpu_types as wgt;

use crate::error::{BasaltError, Result};

/// Global statistics for skipped undersized buffers
pub static SKIPPED_BUFFER_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Get the number of buffers that were skipped due to size mismatch
pub fn get_skipped_buffer_count() -> u64 {
    SKIPPED_BUFFER_COUNT.load(Ordering::Relaxed)
}

/// A set of timestamp queries for GPU profiling
///
/// Contains the query set, resolve buffer, and destination buffer.
pub struct TimestampQuerySet {
    /// The query set ID
    pub query_set_id: id::QuerySetId,
    /// Buffer for resolving timestamps
    pub resolve_buffer_id: id::BufferId,
    /// Buffer for reading resolved timestamps (MAP_READ)
    pub destination_buffer_id: id::BufferId,
    /// Number of queries in the set
    pub num_queries: u64,
    /// Index of the next unused query
    pub next_unused_query: u32,
    /// Whether the buffer is currently mapped
    is_mapped: AtomicBool,
}

impl TimestampQuerySet {
    /// Create a new timestamp query set
    ///
    /// # Arguments
    /// - `context` - The wgpu context
    /// - `device_id` - The device to create resources on
    /// - `num_queries` - Number of timestamp queries to allocate
    ///
    /// # Example
    /// ```no_run
    /// # use bassalt_native::timestamp_queries::TimestampQuerySet;
    /// # let context = unimplemented!();
    /// # let device_id = unimplemented!();
    /// let queries = TimestampQuerySet::new(&context, device_id, 8)?;
    /// ```
    pub fn new(
        context: &Arc<crate::context::BasaltContext>,
        device_id: id::DeviceId,
        num_queries: u64,
    ) -> Result<Self> {
        let global = context.inner();

        // Create the query set
        let query_set_desc = wgt::QuerySetDescriptor {
            label: Some(Cow::Borrowed("Timestamp Query Set")),
            count: num_queries as u32,
            ty: wgt::QueryType::Timestamp,
        };

        let (query_set_id, error) = global
            .device_create_query_set(device_id, &query_set_desc, None);

        if let Some(e) = error {
            return Err(BasaltError::resource_creation(
                "query set",
                format!("{:?}", e),
            ));
        }

        // Create resolve buffer (QUERY_RESOLVE usage)
        let resolve_buffer_desc = wgt::BufferDescriptor {
            label: Some(Cow::Borrowed("Query Resolve Buffer")),
            size: std::mem::size_of::<u64>() as u64 * num_queries,
            usage: wgt::BufferUsages::COPY_SRC | wgt::BufferUsages::QUERY_RESOLVE,
            mapped_at_creation: false,
        };

        let (resolve_buffer_id, error) = global.device_create_buffer(device_id, &resolve_buffer_desc, None);

        if let Some(e) = error {
            return Err(BasaltError::resource_creation(
                "resolve buffer",
                format!("{:?}", e),
            ));
        }

        // Create destination buffer (MAP_READ usage)
        let dest_buffer_desc = wgt::BufferDescriptor {
            label: Some(Cow::Borrowed("Query Destination Buffer")),
            size: std::mem::size_of::<u64>() as u64 * num_queries,
            usage: wgt::BufferUsages::COPY_DST | wgt::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        };

        let (dest_buffer_id, error) = global.device_create_buffer(device_id, &dest_buffer_desc, None);

        if let Some(e) = error {
            return Err(BasaltError::resource_creation(
                "destination buffer",
                format!("{:?}", e),
            ));
        }

        log::info!(
            "Created timestamp query set with {} queries",
            num_queries
        );

        Ok(Self {
            query_set_id,
            resolve_buffer_id,
            destination_buffer_id: dest_buffer_id,
            num_queries,
            next_unused_query: 0,
            is_mapped: AtomicBool::new(false),
        })
    }

    /// Write a timestamp at a specific query index
    ///
    /// This records a timestamp at the current point in command encoding.
    /// Requires `Features::TIMESTAMP_QUERY_INSIDE_ENCODERS`.
    ///
    /// Note: This method only tracks the query state. The actual timestamp
    /// write is done through the JNI layer which has access to the command encoder.
    ///
    /// # Arguments
    /// - `query_index` - The index in the query set to write to
    pub fn write_timestamp(
        &mut self,
        query_index: u32,
    ) -> Result<()> {
        if query_index >= self.num_queries as u32 {
            return Err(BasaltError::invalid_parameter(
                "query_index",
                format!("out of range (max: {})", self.num_queries)
            ));
        }

        // Track the query as used
        self.next_unused_query = self.next_unused_query.max(query_index + 1);
        log::trace!("Writing timestamp at query index {}", query_index);
        Ok(())
    }

    /// Resolve timestamps to the destination buffer
    ///
    /// This must be called after all timestamps have been written but before
    /// submitting the command buffer.
    ///
    /// Note: This method only validates the range. The actual resolve
    /// is done through the JNI layer which has access to the command encoder.
    ///
    /// # Arguments
    /// - `range` - The range of queries to resolve (start..end)
    pub fn resolve(&self, range: std::ops::Range<u32>) -> Result<()> {
        if range.end > self.next_unused_query {
            return Err(BasaltError::invalid_parameter(
                "range",
                format!("exceeds next unused query {}", self.next_unused_query)
            ));
        }

        log::trace!("Resolving timestamp queries {:?}", range);
        Ok(())
    }

    /// Read resolved timestamps from the destination buffer
    ///
    /// This will block until the GPU has finished writing the timestamps.
    /// Call this after submitting the command buffer.
    ///
    /// # Arguments
    /// - `context` - The wgpu context
    /// - `device_id` - The device to poll on
    /// - `range` - The range of queries to read
    ///
    /// # Returns
    /// A vector of timestamps in nanoseconds (raw GPU values)
    pub fn read(
        &self,
        context: &Arc<crate::context::BasaltContext>,
        device_id: id::DeviceId,
        range: std::ops::Range<u32>,
    ) -> Result<Vec<u64>> {
        let global = context.inner();
        let buffer_id = self.destination_buffer_id;

        // Calculate the byte range to map
        let offset = range.start as u64 * std::mem::size_of::<u64>() as u64;
        let size = Some((range.end - range.start) as u64 * std::mem::size_of::<u64>() as u64);

        // Use a channel to wait for the mapping callback
        use std::sync::mpsc;
        let (tx, rx) = mpsc::channel();

        // Create the callback for buffer_map_async
        let callback = Box::new(move |result: wgpu_core::resource::BufferAccessResult| {
            if let Err(e) = result {
                let _ = tx.send(Err(format!("Buffer mapping failed: {:?}", e)));
            } else {
                let _ = tx.send(Ok(()));
            }
        });

        // Initiate the async mapping
        let map_op = wgpu_core::resource::BufferMapOperation {
            host: wgpu_core::device::HostMap::Read,
            callback: Some(callback),
        };

        if let Err(e) = global.buffer_map_async(buffer_id, offset, size, map_op) {
            return Err(BasaltError::Generic(format!("Failed to map buffer: {:?}", e)));
        }

        // Poll the device until mapping completes
        let poll_result = loop {
            match global.device_poll(device_id, wgt::PollType::wait_indefinitely()) {
                Ok(status) if status.is_queue_empty() => break Ok(()),
                Ok(_) => continue,
                Err(e) => break Err(format!("Device poll failed: {:?}", e)),
            }
        };

        if let Err(e) = poll_result {
            return Err(BasaltError::Generic(e));
        }

        // Wait for the callback to complete
        match rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(BasaltError::Generic(e)),
            Err(e) => return Err(BasaltError::Generic(format!("Channel receive failed: {}", e))),
        }

        // Get the mapped range
        let (ptr, mapped_size) = global.buffer_get_mapped_range(buffer_id, offset, size)
            .map_err(|e| BasaltError::Generic(format!("Failed to get mapped range: {:?}", e)))?;

        let expected_size = (range.end - range.start) as u64 * std::mem::size_of::<u64>() as u64;
        if mapped_size != expected_size {
            let _ = global.buffer_unmap(buffer_id);
            return Err(BasaltError::Generic(format!(
                "Mapped size mismatch: expected {}, got {}",
                expected_size, mapped_size
            )));
        }

        // Read the timestamps
        let timestamps = unsafe {
            let slice = std::slice::from_raw_parts(ptr.as_ptr(), (range.end - range.start) as usize);
            // Convert from [u8] to [u64]
            let u64_slice = std::slice::from_raw_parts(slice.as_ptr() as *const u64, slice.len() / 8);
            u64_slice.to_vec()
        };

        // Unmap the buffer
        let _ = global.buffer_unmap(buffer_id);

        log::trace!("Read {} timestamps", timestamps.len());
        Ok(timestamps)
    }

    /// Convenience method to resolve and read timestamps
    ///
    /// This combines `resolve()` and `read()` into a single call.
    /// Note that this requires submitting the command buffer and waiting
    /// for GPU completion.
    pub fn resolve_and_read(
        &self,
        context: &Arc<crate::context::BasaltContext>,
        device_id: id::DeviceId,
        range: std::ops::Range<u32>,
    ) -> Result<Vec<u64>> {
        // This would need to be called after command buffer submission
        // For now, delegate to read()
        self.read(context, device_id, range)
    }

    /// Get the timestamp period for the device
    ///
    /// The timestamp period is the unit of time for timestamp values.
    /// Multiply raw timestamps by this value to get nanoseconds.
    ///
    /// # Returns
    /// The period in nanoseconds per timestamp tick
    pub fn get_timestamp_period(
        context: &Arc<crate::context::BasaltContext>,
        queue_id: id::QueueId,
    ) -> Result<f32> {
        let global = context.inner();

        // wgpu 27.0: Global::queue_get_timestamp_period
        let period = global.queue_get_timestamp_period(queue_id);

        log::trace!("Timestamp period: {} ns/tick", period);
        Ok(period)
    }
}

/// Helper to calculate elapsed time between two timestamps
///
/// # Arguments
/// - `start` - The start timestamp
/// - `end` - The end timestamp
/// - `period` - The timestamp period (from `get_timestamp_period`)
///
/// # Returns
/// The elapsed time in microseconds
pub fn elapsed_microseconds(start: u64, end: u64, period: f32) -> f64 {
    end.wrapping_sub(start) as f64 * period as f64 / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elapsed_time() {
        let start = 1000;
        let end = 2000;
        let period = 1.0; // 1 nanosecond per tick

        let elapsed = elapsed_microseconds(start, end, period);
        assert_eq!(elapsed, 1.0); // 1 microsecond
    }
}
