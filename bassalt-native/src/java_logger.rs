use log::{Level, LevelFilter, Log, Metadata, Record};
use jni::{JNIEnv, objects::{JClass, JString}, sys::{jint, jstring}};
use std::sync::Mutex;
use once_cell::sync::Lazy;

static JAVA_VM: Lazy<Mutex<Option<jni::JavaVM>>> = Lazy::new(|| Mutex::new(None));

/// Store the JavaVM for logging use
pub fn set_java_vm(vm: jni::JavaVM) {
    if let Ok(mut java_vm_guard) = JAVA_VM.lock() {
        *java_vm_guard = Some(vm);
    }
}

/// Custom logger that forwards to Java Log4j
struct JavaLogger;

impl Log for JavaLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let level = match record.level() {
                Level::Trace => 0,
                Level::Debug => 1, 
                Level::Info => 2,
                Level::Warn => 3,
                Level::Error => 4,
            };

            let message = format!("{}", record.args());
            
            // Try to log through Java, fallback to stderr if not available
            if let Ok(java_vm_guard) = JAVA_VM.lock() {
                if let Some(ref vm) = *java_vm_guard {
                    if let Ok(mut env) = vm.attach_current_thread() {
                        let message_jstring = match env.new_string(&message) {
                            Ok(s) => s,
                            Err(_) => {
                                eprintln!("[Bassalt] Failed to create Java string for log: {}", message);
                                return;
                            }
                        };

                        // Call BassaltLogger.log(level, message)
                        let result = env.call_static_method(
                            "com/criticalrange/bassalt/backend/BassaltLogger",
                            "log",
                            "(ILjava/lang/String;)V",
                            &[
                                jni::objects::JValue::Int(level),
                                jni::objects::JValue::Object(&message_jstring),
                            ],
                        );

                        if let Err(e) = result {
                            eprintln!("[Bassalt] Failed to call Java logger: {:?}", e);
                            // Fallback to stderr
                            eprintln!("[Bassalt] [{}] {}", record.level(), message);
                        }
                        return;
                    }
                }
            }
            
            // Fallback to stderr if Java VM not available
            eprintln!("[Bassalt] [{}] {}", record.level(), message);
        }
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

/// JNI function to provide the JavaVM to the native logger
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltLogger_initNativeLogger(
    env: JNIEnv,
    _class: JClass,
) {
    // Get the JavaVM from the JNIEnv
    match env.get_java_vm() {
        Ok(vm) => set_java_vm(vm),
        Err(e) => eprintln!("[Bassalt] Failed to get JavaVM for logging: {:?}", e),
    }
    
    // Initialize the Java logger
    init_java_logging();
}
