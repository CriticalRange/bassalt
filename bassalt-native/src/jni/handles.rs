use std::sync::Arc;
use parking_lot::Mutex;
use std::collections::HashMap;

/// Store for tracking Rust objects owned by Java references
///
/// This provides a safer alternative to raw pointers by maintaining
/// ownership in Rust and returning opaque handles to Java.
pub struct HandleStore<K = u64, V: Sized> {
    next_id: std::sync::atomic::AtomicU64,
    data: Mutex<HashMap<K, Box<V>>>,
}

impl<K: Copy + Clone + std::hash::Hash + Eq, V: Sized> HandleStore<K, V> {
    pub fn new() -> Self {
        Self {
            next_id: std::sync::atomic::AtomicU64::new(1),
            data: Mutex::new(HashMap::new()),
        }
    }

    /// Allocate a new handle for a value
    pub fn allocate(&self, value: V) -> K {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut data = self.data.lock();
        data.insert(id, Box::new(value));
        // For u64 handles, return the id directly
        // For other types, you'd need to convert
        unsafe { std::mem::transmute_copy(&id) }
    }

    /// Get a reference to a value by handle
    pub fn get(&self, handle: K) -> Option<&V> {
        let id = unsafe { std::mem::transmute_copy::<K, u64>(&handle) };
        let data = self.data.lock();
        data.get(&id).map(|b| b.as_ref())
    }

    /// Get a mutable reference to a value by handle
    pub fn get_mut(&self, handle: K) -> Option<&mut V> {
        let id = unsafe { std::mem::transmute_copy::<K, u64>(&handle) };
        let mut data = self.data.lock();
        data.get_mut(&id).map(|b| b.as_mut())
    }

    /// Remove and return a value by handle
    pub fn remove(&self, handle: K) -> Option<V> {
        let id = unsafe { std::mem::transmute_copy::<K, u64>(&handle) };
        let mut data = self.data.lock();
        data.remove(&id).map(|b| *b)
    }

    /// Remove a value by handle and drop it
    pub fn drop_handle(&self, handle: K) -> bool {
        let id = unsafe { std::mem::transmute_copy::<K, u64>(&handle) };
        let mut data = self.data.lock();
        data.remove(&id).is_some()
    }
}

impl<K: Copy + Clone + std::hash::Hash + Eq, V: Sized> Default for HandleStore<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
