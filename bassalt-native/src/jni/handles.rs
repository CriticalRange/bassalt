//! Handle management for JNI - simplified version

use parking_lot::Mutex;
use std::collections::HashMap;

/// Store for tracking Rust objects owned by Java references
///
/// This provides a safer alternative to raw pointers by maintaining
/// ownership in Rust and returning opaque handles to Java.
pub struct HandleStore<V: Sized> {
    next_id: std::sync::atomic::AtomicU64,
    data: Mutex<HashMap<u64, Box<V>>>,
}

impl<V: Sized> HandleStore<V> {
    pub fn new() -> Self {
        Self {
            next_id: std::sync::atomic::AtomicU64::new(1),
            data: Mutex::new(HashMap::new()),
        }
    }

    /// Allocate a new handle for a value
    pub fn allocate(&self, value: V) -> u64 {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut data = self.data.lock();
        data.insert(id, Box::new(value));
        id
    }

    /// Get a reference to a value by handle
    pub fn get(&self, _handle: u64) -> Option<&V> {
        let _data = self.data.lock();
        // Safety: This is actually incorrect - we can't return a reference while holding the lock
        // For now, this is a placeholder. Real implementation would use different patterns.
        None
    }

    /// Remove and return a value by handle
    pub fn remove(&self, handle: u64) -> Option<V> {
        let mut data = self.data.lock();
        data.remove(&handle).map(|b| *b)
    }

    /// Remove a value by handle and drop it
    pub fn drop_handle(&self, handle: u64) -> bool {
        let mut data = self.data.lock();
        data.remove(&handle).is_some()
    }
}

impl<V: Sized> Default for HandleStore<V> {
    fn default() -> Self {
        Self::new()
    }
}
