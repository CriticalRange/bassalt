use log::{Level, LevelFilter, Log, Metadata, Record};
use jni::{JNIEnv, objects::{JClass, JStaticMethodID, JValue, JObjectArray, JString}, sys::{jint, jlong}, signature::{Primitive, ReturnType}, JavaVM};
use std::sync::Mutex;
use once_cell::sync::{Lazy, OnceCell};
use std::sync::Arc;

/// Cached JNI method and class information for zero-copy logging
struct CachedLoggerInfo {
    /// Global reference to the BassaltLogger class
    class: jni::objects::GlobalRef,
    /// Method ID for log(int, String) - for fallback compatibility
    log_string_method_id: JStaticMethodID,
    /// Method ID for logUtf8(int, long ptr, int len) - zero-copy fast path
    log_utf8_method_id: JStaticMethodID,
}

unsafe impl Send for CachedLoggerInfo {}
unsafe impl Sync for CachedLoggerInfo {}

/// Global cached logger information
static CACHED_LOGGER: OnceCell<CachedLoggerInfo> = OnceCell::new();

static JAVA_VM: Lazy<Mutex<Option<JavaVM>>> = Lazy::new(|| Mutex::new(None));

/// Maximum number of debug messages to store
const MAX_DEBUG_MESSAGES: usize = 100;

/// A single debug message
#[derive(Clone)]
struct DebugMessage {
    level: String,
    message: String,
}

/// Thread-safe message buffer for getLastDebugMessages()
static DEBUG_MESSAGES: Lazy<Arc<Mutex<Vec<DebugMessage>>>> = Lazy::new(|| Arc::new(Mutex::new(Vec::new())));

/// Store the JavaVM for logging use
pub fn set_java_vm(vm: JavaVM) {
    if let Ok(mut java_vm_guard) = JAVA_VM.lock() {
        *java_vm_guard = Some(vm);
    }
}

/// Initialize cached JNI method and class information
///
/// This should be called once during initialization to cache method IDs
/// and class references, avoiding repeated JNI lookups on every log call.
fn init_cached_logger(env: &mut JNIEnv) -> Result<(), String> {
    // Find the BassaltLogger class
    let class = env.find_class("com/criticalrange/bassalt/backend/BassaltLogger")
        .map_err(|e| format!("Failed to find BassaltLogger class: {}", e))?;

    // Create a global reference to keep the class from being GC'd
    let global_class = env.new_global_ref(&class)
        .map_err(|e| format!("Failed to create global ref: {}", e))?;

    // Get method ID for the standard log(int, String) method (fallback)
    let log_string_method_id = env.get_static_method_id(
        &class,
        "log",
        "(ILjava/lang/String;)V"
    ).map_err(|e| format!("Failed to get log method ID: {}", e))?;

    // Get method ID for the zero-copy logUtf8(int, long, int) method
    let log_utf8_method_id = env.get_static_method_id(
        &class,
        "logUtf8",
        "(IJI)V"
    ).map_err(|e| format!("Failed to get logUtf8 method ID: {}", e))?;

    // Cache all the information globally
    CACHED_LOGGER.set(CachedLoggerInfo {
        class: global_class,
        log_string_method_id,
        log_utf8_method_id,
    }).map_err(|_| "Already initialized".to_string())?;

    Ok(())
}

/// Custom logger that forwards to Java Log4j with optimized JNI calls
struct JavaLogger;

impl Log for JavaLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let level = match record.level() {
            Level::Trace => 0,
            Level::Debug => 1,
            Level::Info => 2,
            Level::Warn => 3,
            Level::Error => 4,
        };

        // Format the message to a String (this is unfortunately necessary for log formatting)
        let message = format!("{}", record.args());

        // Store message in debug buffer for getLastDebugMessages()
        // Only store warnings and errors
        if record.level() >= Level::Warn {
            if let Ok(mut msgs) = DEBUG_MESSAGES.lock() {
                let level_str = format!("{:?}", record.level());
                msgs.push(DebugMessage {
                    level: level_str,
                    message: message.clone(),
                });
                // Keep only the most recent MAX_DEBUG_MESSAGES
                if msgs.len() > MAX_DEBUG_MESSAGES {
                    msgs.remove(0);
                }
            }
        }

        // Try to log through Java using optimized paths
        if let Ok(java_vm_guard) = JAVA_VM.lock() {
            if let Some(ref vm) = *java_vm_guard {
                // Use permanent attachment for better performance
                let env_result = vm.attach_current_thread_permanently();

                if let Ok(mut env) = env_result {
                    // Try the zero-copy fast path first if cached info is available
                    if let Some(cached) = CACHED_LOGGER.get() {
                        let message_bytes = message.as_bytes();
                        let msg_ptr = message_bytes.as_ptr() as jlong;
                        let msg_len = message_bytes.len() as jint;

                        let result = unsafe {
                            env.call_static_method_unchecked(
                                &cached.class,
                                cached.log_utf8_method_id,
                                ReturnType::Primitive(Primitive::Void),
                                &[
                                    JValue::Int(level).as_jni(),
                                    JValue::Long(msg_ptr).as_jni(),
                                    JValue::Int(msg_len).as_jni(),
                                ],
                            )
                        };

                        if result.is_ok() {
                            return; // Success with zero-copy path
                        }

                        // Fall back to string path if zero-copy fails
                        let message_jstring = match env.new_string(&message) {
                            Ok(s) => s,
                            Err(_) => {
                                eprintln!("[Bassalt] Failed to create Java string for log: {}", message);
                                return;
                            }
                        };

                        let result = unsafe {
                            env.call_static_method_unchecked(
                                &cached.class,
                                cached.log_string_method_id,
                                ReturnType::Primitive(Primitive::Void),
                                &[
                                    JValue::Int(level).as_jni(),
                                    JValue::Object(&message_jstring).as_jni(),
                                ],
                            )
                        };

                        if let Err(e) = result {
                            eprintln!("[Bassalt] Failed to call Java logger: {:?}", e);
                            eprintln!("[Bassalt] [{}] {}", record.level(), message);
                        }
                    } else {
                        // Fallback: not initialized yet, use slow path
                        let message_jstring = match env.new_string(&message) {
                            Ok(s) => s,
                            Err(_) => {
                                eprintln!("[Bassalt] Failed to create Java string for log: {}", message);
                                return;
                            }
                        };

                        let result = env.call_static_method(
                            "com/criticalrange/bassalt/backend/BassaltLogger",
                            "log",
                            "(ILjava/lang/String;)V",
                            &[
                                JValue::Int(level),
                                JValue::Object(&message_jstring),
                            ],
                        );

                        if let Err(e) = result {
                            eprintln!("[Bassalt] Failed to call Java logger: {:?}", e);
                            eprintln!("[Bassalt] [{}] {}", record.level(), message);
                        }
                    }
                    return;
                }
            }
        }

        // Final fallback to stderr if Java VM not available
        eprintln!("[Bassalt] [{}] {}", record.level(), message);
    }

    fn flush(&self) {
        // Java logging handles flushing automatically
    }
}

/// Initialize the Java logger
pub fn init_java_logging() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let log_level = if std::env::var("BASALT_DEBUG").is_ok() {
            LevelFilter::Debug
        } else {
            LevelFilter::Info
        };

        log::set_logger(&JavaLogger)
            .map(|_| log::set_max_level(log_level))
            .expect("Failed to set Java logger");
    });
}

/// JNI function to provide the JavaVM to the native logger and initialize caching
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltLogger_initNativeLogger(
    mut env: JNIEnv,
    _class: JClass,
) {
    // Get the JavaVM from the JNIEnv
    let vm = match env.get_java_vm() {
        Ok(vm) => vm,
        Err(e) => {
            eprintln!("[Bassalt] Failed to get JavaVM for logging: {:?}", e);
            return;
        }
    };

    set_java_vm(vm);

    // Initialize cached method IDs and class references
    if let Err(e) = init_cached_logger(&mut env) {
        eprintln!("[Bassalt] Failed to initialize cached logger info: {}", e);
        // Continue anyway - will use slow path fallback
    }

    // Initialize the Java logger
    init_java_logging();
}

/// JNI function to retrieve stored debug messages
/// Returns an array of strings in format "[LEVEL] message"
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_getLastDebugMessagesNative<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> JObjectArray<'local> {
    // Get the messages from the buffer
    let messages = if let Ok(msgs) = DEBUG_MESSAGES.lock() {
        msgs.clone()
    } else {
        Vec::new()
    };

    // Create a Java String array
    let string_class = match env.find_class("java/lang/String") {
        Ok(cls) => cls,
        Err(_) => return JObjectArray::default(),
    };

    let array = match env.new_object_array(messages.len() as jint, &string_class, JString::default()) {
        Ok(arr) => arr,
        Err(_) => return JObjectArray::default(),
    };

    // Fill the array with formatted messages
    for (i, msg) in messages.iter().enumerate() {
        let formatted = format!("[{}] {}", msg.level, msg.message);
        if let Ok(jstr) = env.new_string(&formatted) {
            let _ = env.set_object_array_element(&array, i as jint, jstr);
        }
    }

    array
}
