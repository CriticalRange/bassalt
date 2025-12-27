use jni::JNIEnv;
use jni::objects::JClass;
use jni::sys::jlong;

/// Helper function to extract a reference from a raw pointer handle
///
/// # Safety
/// The handle must be a valid pointer allocated by Rust
pub unsafe fn get_ref_from_handle<'a, T>(handle: jlong) -> Option<&'a T> {
    if handle == 0 {
        None
    } else {
        (handle as *const T).as_ref()
    }
}

/// Helper function to extract a mutable reference from a raw pointer handle
///
/// # Safety
/// The handle must be a valid pointer allocated by Rust
/// Care must be taken to ensure no aliasing occurs
pub unsafe fn get_mut_from_handle<'a, T>(handle: jlong) -> Option<&'a mut T> {
    if handle == 0 {
        None
    } else {
        (handle as *mut T).as_mut()
    }
}

/// Helper function to box a value and return its handle
pub fn box_into_handle<T>(value: T) -> jlong {
    Box::into_raw(Box::new(value)) as jlong
}

/// Helper function to convert a handle back into a Box, dropping it
///
/// # Safety
/// The handle must be a valid pointer allocated by box_into_handle
pub unsafe fn unbox_from_handle<T>(handle: jlong) -> Option<Box<T>> {
    if handle == 0 {
        None
    } else {
        Some(Box::from_raw(handle as *mut T))
    }
}
