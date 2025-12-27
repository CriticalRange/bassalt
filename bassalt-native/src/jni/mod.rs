pub mod env;
pub mod strings;
pub mod handles;

use jni::JNIEnv;
use log::LevelFilter;

/// Initialize logging for the native library
pub fn init_logging() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let log_level = if std::env::var("BASALT_DEBUG").is_ok() {
            LevelFilter::Debug
        } else {
            LevelFilter::Info
        };

        env_logger::Builder::new()
            .filter_level(log_level)
            .init();
    });
}

/// Trait for converting Rust errors to Java exceptions
pub trait ToJavaException {
    fn throw_in(&self, env: &mut JNIEnv, class_name: &str);
}

impl ToJavaException for String {
    fn throw_in(&self, env: &mut JNIEnv, class_name: &str) {
        let _ = env.throw_new(class_name, self);
    }
}

impl ToJavaException for &str {
    fn throw_in(&self, env: &mut JNIEnv, class_name: &str) {
        let _ = env.throw_new(class_name, self);
    }
}

impl<T: ToJavaException> ToJavaException for Result<T, String> {
    fn throw_in(&self, env: &mut JNIEnv, class_name: &str) {
        match self {
            Ok(_) => {}
            Err(e) => e.throw_in(env, class_name),
        }
    }
}

impl<T: ToJavaException> ToJavaException for Result<T, &str> {
    fn throw_in(&self, env: &mut JNIEnv, class_name: &str) {
        match self {
            Ok(_) => {}
            Err(e) => e.throw_in(env, class_name),
        }
    }
}
