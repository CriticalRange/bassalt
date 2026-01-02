use jni::JNIEnv;
use jni::objects::JString;

/// Get a Rust String from a JString
///
/// # Errors
/// Returns an error if the JString is null or contains invalid UTF-16
pub fn get_jstring_utf8(env: &mut JNIEnv, jstr: JString) -> Result<String, String> {
    env.get_string(&jstr)
        .map(|s| s.into())
        .map_err(|e| format!("Failed to get Java string: {}", e))
}

/// Create a JString from a Rust &str
///
/// # Errors
/// Returns an error if the string contains invalid Java UTF-16
pub fn rust_string_to_jstring(env: &mut JNIEnv, s: &str) -> Result<jni::sys::jstring, String> {
    env.new_string(s)
        .map(|j| j.into_raw() as jni::sys::jstring)
        .map_err(|e| format!("Failed to create Java string: {}", e))
}
