#![allow(clippy::missing_safety_doc)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

mod jni;
mod java_logger;
mod context;
mod device;
mod surface;
mod buffer;
mod texture;
mod texture_and_view;
mod sampler;
mod pipeline;
mod shader;
mod shader_processor;
mod shader_validator;
mod command;
mod error;
mod resource_handles;
mod render_pass;
mod bind_group;
mod bind_group_layouts;
mod pipeline_registry;
mod render_bundle;
mod timestamp_queries;
mod msaa;

use std::borrow::Cow;
use std::sync::Arc;
use ::jni::JNIEnv;
use ::jni::objects::{JByteArray, JClass, JString, JObject};
use ::jni::sys::{jlong, jint, jboolean, jstring, jfloat, jlongArray};
use once_cell::sync::OnceCell;
use log::info;
use wgpu_types as wgt;

use crate::context::BasaltContext;
use crate::device::BasaltDevice;
use crate::error::BasaltError;
use crate::resource_handles::HANDLES;

/// Global context singleton
static GLOBAL_CONTEXT: OnceCell<Arc<BasaltContext>> = OnceCell::new();

/// Initialize the Basalt renderer
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltBackend_init(
    _env: JNIEnv,
    _class: JClass,
) -> jlong {
    java_logger::init_java_logging();

    let context = Arc::new(BasaltContext::new());
    match GLOBAL_CONTEXT.set(context.clone()) {
        Ok(_) => {
            info!("Basalt renderer initialized");
            Arc::into_raw(context) as jlong
        }
        Err(_) => {
            // Already initialized, return existing
            let ctx = GLOBAL_CONTEXT.get().unwrap();
            Arc::into_raw(ctx.clone()) as jlong
        }
    }
}

/// Get the global context
pub fn get_global_context() -> Option<Arc<BasaltContext>> {
    GLOBAL_CONTEXT.get().cloned()
}

/// Map JNI blend factor index to WebGPU blend factor
/// Matches BassaltBackend.BLEND_FACTOR_* constants
fn map_blend_factor_from_jni(factor: jint) -> Option<wgt::BlendFactor> {
    Some(match factor as u32 {
        0 => wgt::BlendFactor::Zero,
        1 => wgt::BlendFactor::One,
        2 => wgt::BlendFactor::Src,
        3 => wgt::BlendFactor::OneMinusSrc,
        4 => wgt::BlendFactor::Dst,
        5 => wgt::BlendFactor::OneMinusDst,
        6 => wgt::BlendFactor::SrcAlpha,
        7 => wgt::BlendFactor::OneMinusSrcAlpha,
        8 => wgt::BlendFactor::DstAlpha,
        9 => wgt::BlendFactor::OneMinusDstAlpha,
        _ => return None, // Unknown factor, return None
    })
}

/// Create a device from GLFW window handle
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltBackend_createDevice(
    mut env: JNIEnv,
    _class: JClass,
    context_ptr: jlong,
    window_ptr: jlong,
    display_ptr: jlong,
    width: jint,
    height: jint,
) -> jlong {
    let context = unsafe {
        if context_ptr == 0 {
            let _ = env.throw_new("java/lang/IllegalArgumentException", "Null context pointer");
            return 0;
        }
        Arc::from_raw(context_ptr as *const BasaltContext)
    };

    // We'll re-clone the Arc to keep it alive
    let context_clone = context.clone();
    std::mem::forget(context); // Don't drop, we still own the reference

    match device::create_device_from_window(
        context_clone,
        window_ptr as u64,
        display_ptr as u64,
        width as u32,
        height as u32
    ) {
        Ok(device) => {
            info!("Device created successfully");
            Box::into_raw(Box::new(device)) as jlong
        }
        Err(e) => {
            let msg = format!("Failed to create device: {}", e);
            let _ = env.throw_new("java/lang/RuntimeException", &msg);
            0
        }
    }
}

/// Get adapter information
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltBackend_getAdapterInfo(
    mut env: JNIEnv,
    _class: JClass,
    context_ptr: jlong,
) -> jstring {
    let context = unsafe {
        if context_ptr == 0 {
            let _ = env.throw_new("java/lang/IllegalArgumentException", "Null context pointer");
            return std::ptr::null_mut();
        }
        Arc::from_raw(context_ptr as *const BasaltContext)
    };

    let info = context.get_adapter_info();
    std::mem::forget(context); // Keep alive

    match env.new_string(&info) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Release a device
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_release(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
) {
    if device_ptr != 0 {
        unsafe {
            let _device = Box::from_raw(device_ptr as *mut BasaltDevice);
            // Device is dropped here
        };
    }
}

/// Present the current frame
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_presentFrame(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
) {
    if device_ptr != 0 {
        unsafe {
            let device = &*(device_ptr as *const BasaltDevice);
            if let Err(e) = device.present_frame() {
                log::error!("Failed to present frame: {}", e);
            }
        }
    }
}

/// Set vsync mode
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_setVsync(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    enabled: jboolean,
) {
    if device_ptr != 0 {
        unsafe {
            let device = &*(device_ptr as *const BasaltDevice);
            if let Err(e) = device.set_vsync(enabled != 0) {
                log::error!("Failed to set vsync: {}", e);
            }
        }
    }
}

/// Get implementation information
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_getImplementationInfo(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
) -> jstring {
    if device_ptr == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null device pointer");
        return std::ptr::null_mut();
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };
    let info = device.get_implementation_info();

    match env.new_string(&info) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Get vendor name
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_getVendor(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
) -> jstring {
    if device_ptr == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null device pointer");
        return std::ptr::null_mut();
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };
    let vendor = device.get_vendor();

    match env.new_string(&vendor) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Get renderer name
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_getRenderer(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
) -> jstring {
    if device_ptr == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null device pointer");
        return std::ptr::null_mut();
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };
    let renderer = device.get_renderer();

    match env.new_string(&renderer) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Get driver version
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_getVersion(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
) -> jstring {
    if device_ptr == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null device pointer");
        return std::ptr::null_mut();
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };
    let version = device.get_version();

    match env.new_string(&version) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Get max texture size
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_getMaxTextureSize(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
) -> jint {
    if device_ptr == 0 {
        return 0;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };
    device.get_limits().max_texture_dimension_2d as jint
}

/// Get max supported anisotropy
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_getMaxSupportedAnisotropy0(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
) -> jint {
    if device_ptr == 0 {
        return 1;
    }

    // wgpu doesn't expose max anisotropy in device limits
    // Most modern GPUs support 16x anisotropy
    // This is a safe default value
    16
}

/// Get enabled extensions/features as a comma-separated string
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_getEnabledFeatures0(
    env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
) -> jstring {
    if device_ptr == 0 {
        return std::ptr::null_mut();
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Query device features from wgpu-core
    let features = device.get_context().inner().device_features(device.get_device_id());

    // Convert features to a comma-separated string
    let mut feature_list = Vec::new();
    if features.contains(wgt::Features::DEPTH_CLIP_CONTROL) {
        feature_list.push("DEPTH_CLIP_CONTROL");
    }
    if features.contains(wgt::Features::TEXTURE_COMPRESSION_BC) {
        feature_list.push("TEXTURE_COMPRESSION_BC");
    }
    if features.contains(wgt::Features::TEXTURE_COMPRESSION_ETC2) {
        feature_list.push("TEXTURE_COMPRESSION_ETC2");
    }
    if features.contains(wgt::Features::TEXTURE_COMPRESSION_ASTC) {
        feature_list.push("TEXTURE_COMPRESSION_ASTC");
    }
    if features.contains(wgt::Features::TIMESTAMP_QUERY) {
        feature_list.push("TIMESTAMP_QUERY");
    }
    if features.contains(wgt::Features::PIPELINE_STATISTICS_QUERY) {
        feature_list.push("PIPELINE_STATISTICS_QUERY");
    }

    let features_str = feature_list.join(", ");

    match env.new_string(&features_str) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Get uniform offset alignment
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_getUniformOffsetAlignment(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
) -> jint {
    if device_ptr == 0 {
        return 0;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };
    device.get_limits().min_uniform_buffer_offset_alignment as jint
}

/// Check if Z range is 0-1 (WebGPU standard) or -1 to 1 (OpenGL)
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_isZZeroToOne(
    _env: JNIEnv,
    _class: JClass,
    _device_ptr: jlong,
) -> jboolean {
    // WebGPU always uses 0-1 Z range
    1
}

/// Close/release the device
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_close(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
) {
    if device_ptr != 0 {
        unsafe {
            let _device = Box::from_raw(device_ptr as *mut BasaltDevice);
            // Device is dropped here
        };
    }
}

/// Translate GLSL shader to WGSL
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_shader_WgslCompiler_translateGlslToWgsl(
    mut env: JNIEnv,
    _class: JClass,
    glsl_source: JString,
    stage: jint,
) -> jstring {
    let glsl_str: String = match env.get_string(&glsl_source) {
        Ok(s) => s.into(),
        Err(e) => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", &format!("Invalid string: {}", e));
            return std::ptr::null_mut();
        }
    };

    let stage = match stage {
        0 => naga::ShaderStage::Vertex,
        1 => naga::ShaderStage::Fragment,
        2 => naga::ShaderStage::Compute,
        _ => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", "Invalid shader stage");
            return std::ptr::null_mut();
        }
    };

    match shader::glsl_to_wgsl(&glsl_str, stage) {
        Ok(wgsl) => match env.new_string(&wgsl) {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        Err(e) => {
            let msg = format!("Shader translation failed: {}", e);
            let _ = env.throw_new("java/lang/RuntimeException", &msg);
            std::ptr::null_mut()
        }
    }
}

// ============================================================================
// BUFFER OPERATIONS
// ============================================================================

/// Create an empty buffer
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_createBufferEmpty(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    size: jlong,
    usage: jint,
) -> jlong {
    if device_ptr == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null device pointer");
        return 0;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    match device.create_buffer(size as u64, usage as u32) {
        Ok(buffer_id) => {
            // Store the buffer ID and size, return a handle
            let handle = HANDLES.insert_buffer(buffer_id, size as u64);
            log::info!("[BassaltNative] Created buffer: handle={}, wgpu_id={:?}, size={}", handle, buffer_id, size);
            handle as jlong
        }
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to create buffer: {}", e));
            0
        }
    }
}

/// Create a buffer with initial data
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_createBufferData(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    data: JByteArray,
    usage: jint,
) -> jlong {
    if device_ptr == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null device pointer");
        return 0;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Convert Java byte array to Rust Vec
    let data_vec: Vec<u8> = match env.convert_byte_array(&data) {
        Ok(arr) => arr,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to get byte array: {}", e));
            return 0;
        }
    };

    let size = data_vec.len() as u64;

    match device.create_buffer(size, usage as u32) {
        Ok(buffer_id) => {
            // Write initial data
            if let Err(e) = device.write_buffer(buffer_id, 0, &data_vec) {
                let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to write initial buffer data: {}", e));
                return 0;
            }

            // Store the buffer ID and size, return a handle
            let handle = HANDLES.insert_buffer(buffer_id, size);
            log::debug!("Created buffer with handle {} (size={}, with data)", handle, size);
            handle as jlong
        }
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to create buffer: {}", e));
            0
        }
    }
}

/// Write data to a buffer
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_writeBuffer0(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    buffer_handle: jlong,
    data_ptr: JByteArray,
    offset: jlong,
) {
    if device_ptr == 0 || buffer_handle == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null pointer");
        return;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Look up buffer ID from handle
    let buffer_id = match HANDLES.get_buffer(buffer_handle as u64) {
        Some(id) => id,
        None => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", "Invalid buffer handle");
            return;
        }
    };

    // Convert Java byte array to Rust Vec
    let data: Vec<u8> = match env.convert_byte_array(&data_ptr) {
        Ok(arr) => arr,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to get byte array: {}", e));
            return;
        }
    };

    if let Err(e) = device.write_buffer(buffer_id, offset as u64, &data) {
        let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to write buffer: {}", e));
    }
}

/// Destroy a buffer
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_destroyBuffer(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    buffer_handle: jlong,
) {
    if device_ptr == 0 || buffer_handle == 0 {
        return;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Look up and remove buffer ID from handle store
    if let Some(buffer_id) = HANDLES.remove_buffer(buffer_handle as u64) {
        device.destroy_buffer(buffer_id);
        log::debug!("Destroyed buffer with handle {}", buffer_handle);
    }
}

// ============================================================================
// TEXTURE OPERATIONS
// ============================================================================

/// Create a texture
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_createTexture(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    width: jint,
    height: jint,
    depth: jint,
    mip_levels: jint,
    format: jint,
    usage: jint,
) -> jlong {
    if device_ptr == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null device pointer");
        return 0;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Map format first so we can store it
    let texture_format = match device.map_texture_format_public(format as u32) {
        Ok(f) => f,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &format!("Invalid texture format: {}", e));
            return 0;
        }
    };
    
    match device.create_texture(
        width as u32,
        height as u32,
        depth as u32,
        mip_levels as u32,
        format as u32,
        usage as u32,
    ) {
        Ok(texture_id) => {
            // Store texture with array layer info and format for debugging
            let handle = HANDLES.insert_texture(
                texture_id,
                depth as u32,
                wgt::TextureDimension::D2,
                texture_format,
            );
            log::info!("Created texture with handle {} ({}x{}x{}) format={:?}", handle, width, height, depth, texture_format);
            handle as jlong
        }
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to create texture: {}", e));
            0
        }
    }
}

/// Destroy a texture
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_destroyTexture(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    texture_handle: jlong,
) {
    if device_ptr == 0 || texture_handle == 0 {
        return;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    if let Some(texture_id) = HANDLES.remove_texture(texture_handle as u64) {
        device.destroy_texture(texture_id);
        log::debug!("Destroyed texture with handle {}", texture_handle);
    }
}

/// Create a texture view
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_createTextureView(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    texture_handle: jlong,
) -> jlong {
    if device_ptr == 0 || texture_handle == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null pointer");
        return 0;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Look up texture info from handle (including array layers)
    let texture_info = match HANDLES.get_texture_info(texture_handle as u64) {
        Some(info) => info,
        None => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", "Invalid texture handle");
            return 0;
        }
    };

    match device.create_texture_view(texture_info.id, texture_info.array_layers) {
        Ok((view_id, dimension)) => {
            let handle = HANDLES.insert_texture_view(view_id, dimension, texture_info.id);
            // Register the view-to-texture mapping in context for reliable lookups
            device.context().register_texture_view(view_id, texture_info.id);
            log::debug!("Created texture view with handle {} (dimension={:?}, layers={}) for texture {}",
                       handle, dimension, texture_info.array_layers, texture_handle);
            handle as jlong
        }
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to create texture view: {}", e));
            0
        }
    }
}

// ============================================================================
// SAMPLER OPERATIONS
// ============================================================================

/// Create a sampler
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_createSampler(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    address_mode_u: jint,
    address_mode_v: jint,
    address_mode_w: jint,
    min_filter: jint,
    mag_filter: jint,
    mipmap_filter: jint,
    lod_min_clamp: jfloat,
    lod_max_clamp: jfloat,
    max_anisotropy: jint,
) -> jlong {
    if device_ptr == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null device pointer");
        return 0;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    match device.create_sampler(
        address_mode_u as u32,
        address_mode_v as u32,
        address_mode_w as u32,
        min_filter as u32,
        mag_filter as u32,
        mipmap_filter as u32,
        lod_min_clamp,
        lod_max_clamp,
        max_anisotropy as u32,
    ) {
        Ok(sampler_id) => {
            let handle = HANDLES.insert_sampler(sampler_id);
            log::debug!("Created sampler with handle {}", handle);
            handle as jlong
        }
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to create sampler: {}", e));
            0
        }
    }
}

/// Create vertex buffer layout based on format index
fn create_vertex_buffer_layout(format_index: usize) -> Cow<'static, [wgpu_core::pipeline::VertexBufferLayout<'static>]> {
    use std::borrow::Cow;

    match format_index {
        // 255 = EMPTY (no vertex input - shader uses @builtin(vertex_index))
        // Used by shaders like rendertype_clouds that generate geometry procedurally
        255 => Cow::Borrowed(&[]),
        // 0 = POSITION (3 floats)
        0 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
            array_stride: 12, // 3 floats * 4 bytes
            step_mode: wgt::VertexStepMode::Vertex,
            attributes: Cow::Owned(vec![
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
            ]),
        }]),
        // 1 = POSITION_COLOR (3 floats + 4 unsigned bytes)
        // GUI uses UBYTE colors, not float! Total stride = 16 bytes
        1 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
            array_stride: 16, // 12 + 4 = 16 bytes
            step_mode: wgt::VertexStepMode::Vertex,
            attributes: Cow::Owned(vec![
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Unorm8x4,  // UBYTE colors!
                    offset: 12,
                    shader_location: 1,
                },
            ]),
        }]),
        // 2 = POSITION_TEX (3 floats + 2 floats)
        2 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
            array_stride: 20, // 12 + 8 = 20 bytes
            step_mode: wgt::VertexStepMode::Vertex,
            attributes: Cow::Owned(vec![
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x2,
                    offset: 12,
                    shader_location: 1,
                },
            ]),
        }]),
        // 3 = POSITION_TEX_COLOR (3 floats + 2 floats + 4 unsigned bytes)
        // Color is UBYTE (unsigned bytes), not float! Total stride = 24 bytes
        3 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
            array_stride: 24, // 12 + 8 + 4 = 24 bytes
            step_mode: wgt::VertexStepMode::Vertex,
            attributes: Cow::Owned(vec![
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x2,
                    offset: 12,
                    shader_location: 1,
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Unorm8x4,  // UBYTE colors!
                    offset: 20,
                    shader_location: 2,
                },
            ]),
        }]),
        // 4 = POSITION_TEX_COLOR_NORMAL (3 floats + 2 floats + 4 unsigned bytes + 3 floats)
        // Color is UBYTE (unsigned bytes), not float! Total stride = 36 bytes
        4 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
            array_stride: 36, // 12 + 8 + 4 + 12 = 36 bytes
            step_mode: wgt::VertexStepMode::Vertex,
            attributes: Cow::Owned(vec![
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x2,
                    offset: 12,
                    shader_location: 1,
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Unorm8x4,  // UBYTE colors!
                    offset: 20,
                    shader_location: 2,
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x3,
                    offset: 24,
                    shader_location: 3,
                },
            ]),
        }]),
        // 5 = POSITION_TEX_COLOR (Position + UV0 + Color)
        // Memory layout: Position[12] + UV0[8] + Color[4] = 24 bytes
        // Color is UBYTE (unsigned bytes), not float!
        5 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
            array_stride: 24, // 12 + 8 + 4 = 24 bytes
            step_mode: wgt::VertexStepMode::Vertex,
            attributes: Cow::Owned(vec![
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0, // position
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x2,
                    offset: 12,
                    shader_location: 1, // uv
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Unorm8x4,  // UBYTE colors!
                    offset: 20,
                    shader_location: 2, // color
                },
            ]),
        }]),
        // 6 = POSITION_COLOR_TEX_TEX_TEX_NORMAL (position, color, uv0, uv1, uv2, normal)
        // Color is UBYTE (unsigned bytes), not float! Total stride = 52 bytes
        6 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
            array_stride: 52, // 12 + 4 + 8 + 8 + 8 + 12 = 52 bytes
            step_mode: wgt::VertexStepMode::Vertex,
            attributes: Cow::Owned(vec![
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0, // position
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Unorm8x4,  // UBYTE colors!
                    offset: 12,
                    shader_location: 1, // color
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x2,
                    offset: 16,
                    shader_location: 2, // uv0
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x2,
                    offset: 24,
                    shader_location: 3, // uv1
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x2,
                    offset: 32,
                    shader_location: 4, // uv2
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x3,
                    offset: 40,
                    shader_location: 5, // normal
                },
            ]),
        }]),
        // 7 = POSITION_COLOR_TEX_TEX_NORMAL (position, color, uv0, uv2, normal - skips uv1)
        // Color is UBYTE (unsigned bytes), not float! Total stride = 44 bytes
        7 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
            array_stride: 44, // 12 + 4 + 8 + 8 + 12 = 44 bytes
            step_mode: wgt::VertexStepMode::Vertex,
            attributes: Cow::Owned(vec![
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0, // position
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Unorm8x4,  // UBYTE colors!
                    offset: 12,
                    shader_location: 1, // color
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x2,
                    offset: 16,
                    shader_location: 2, // uv0
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x2,
                    offset: 24,
                    shader_location: 3, // uv2
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x3,
                    offset: 32,
                    shader_location: 4, // normal
                },
            ]),
        }]),
        // 8 = POSITION_COLOR_TEX_TEX (position, color, uv0, uv2 - no normal)
        // Color is UBYTE (unsigned bytes), not float! Total stride = 32 bytes
        8 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
            array_stride: 32, // 12 + 4 + 8 + 8 = 32 bytes
            step_mode: wgt::VertexStepMode::Vertex,
            attributes: Cow::Owned(vec![
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0, // position
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Unorm8x4,  // UBYTE colors!
                    offset: 12,
                    shader_location: 1, // color
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x2,
                    offset: 16,
                    shader_location: 2, // uv0
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x2,
                    offset: 24,
                    shader_location: 3, // uv2
                },
            ]),
        }]),
        // Default to POSITION_TEX_COLOR for unknown formats
        _ => {
            log::warn!("Unknown vertex format index: {}, defaulting to POSITION_TEX_COLOR", format_index);
            Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
                array_stride: 36,
                step_mode: wgt::VertexStepMode::Vertex,
                attributes: Cow::Owned(vec![
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x2,
                        offset: 12,
                        shader_location: 1,
                    },
                    wgt::VertexAttribute {
                        format: wgt::VertexFormat::Float32x4,
                        offset: 20,
                        shader_location: 2,
                    },
                ]),
            }])
        }
    }
}

/// Detect if a fragment shader writes to the depth buffer by checking for FragDepth output.
/// This is used to determine if a pipeline needs depth_stencil state.
fn shader_writes_depth(fragment_module: &naga::Module) -> bool {
    for entry_point in &fragment_module.entry_points {
        if entry_point.stage != naga::ShaderStage::Fragment {
            continue;
        }
        
        // Check if the entry point has early_depth_test set
        if entry_point.early_depth_test.is_some() {
            return true;
        }
        
        // Check function result for FragDepth builtin
        if let Some(ref result) = entry_point.function.result {
            // Direct binding check
            if let Some(naga::Binding::BuiltIn(naga::BuiltIn::FragDepth)) = &result.binding {
                return true;
            }
            
            // Check if result is a struct with FragDepth member
            let ty = &fragment_module.types[result.ty];
            if let naga::TypeInner::Struct { members, .. } = &ty.inner {
                for member in members {
                    if let Some(naga::Binding::BuiltIn(naga::BuiltIn::FragDepth)) = &member.binding {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Helper function to create a bind group layout from shader reflection
/// Simplified to single bind group (group 0) only
/// Returns (BindGroupLayoutId, PipelineLayoutId, binding_layouts)
fn create_layout_from_shaders(
    context: &Arc<BasaltContext>,
    device_id: wgpu_core::id::DeviceId,
    vertex_module: &naga::Module,
    fragment_module: &naga::Module,
) -> Result<(wgpu_core::id::BindGroupLayoutId, wgpu_core::id::PipelineLayoutId, Vec<resource_handles::BindingLayoutEntry>), BasaltError> {
    use std::collections::BTreeMap;
    use std::borrow::Cow;
    use std::num::NonZeroU64;
    use wgpu_core::binding_model;
    use resource_handles::{BindingLayoutEntry, BindingLayoutType};
    use naga::proc::{Layouter, GlobalCtx};

    // Create layouters for both modules to calculate type sizes
    let mut vertex_layouter = Layouter::default();
    let mut fragment_layouter = Layouter::default();

    // Update layouters with module types
    let vertex_gctx = GlobalCtx {
        types: &vertex_module.types,
        constants: &vertex_module.constants,
        overrides: &vertex_module.overrides,
        global_expressions: &vertex_module.global_expressions,
    };
    let fragment_gctx = GlobalCtx {
        types: &fragment_module.types,
        constants: &fragment_module.constants,
        overrides: &fragment_module.overrides,
        global_expressions: &fragment_module.global_expressions,
    };

    if let Err(e) = vertex_layouter.update(vertex_gctx) {
        log::warn!("Failed to calculate vertex shader layouts: {:?}", e);
    }
    if let Err(e) = fragment_layouter.update(fragment_gctx) {
        log::warn!("Failed to calculate fragment shader layouts: {:?}", e);
    }

    // Collect all bindings from group 0 only (single bind group approach)
    // Key: binding number, Value: (wgpu entry, our layout type, min_binding_size, variable name)
    let mut bindings: BTreeMap<u32, (wgt::BindGroupLayoutEntry, BindingLayoutType, Option<u64>, Option<String>)> = BTreeMap::new();

    // Helper to extract bindings from a module (group 0 only)
    let mut extract_bindings = |module: &naga::Module, layouter: &Layouter, stage: wgt::ShaderStages| {
        log::info!("extract_bindings: processing {:?} shader, {} global variables", stage, module.global_variables.len());
        for (_handle, global_var) in module.global_variables.iter() {
            if let Some(binding) = &global_var.binding {
                // Only process group 0 bindings (single bind group approach)
                if binding.group != 0 {
                    log::debug!("  Skipping binding at group {}, binding {} (not group 0)", binding.group, binding.binding);
                    continue;
                }
                log::info!("  Found binding {} at group {:?}, name: {:?}, space: {:?}",
                    binding.binding, binding.group, global_var.name, global_var.space);
                {
                    let ty = &module.types[global_var.ty];

                    // Get the variable name from the shader
                    let var_name = global_var.name.clone();

                    let (binding_type, layout_type, min_size, binding_var_name) = match global_var.space {
                        naga::AddressSpace::Uniform => {
                            // Calculate the actual size of the uniform buffer struct
                            let type_layout = layouter[global_var.ty];
                            let struct_size = type_layout.to_stride() as u64; // Use stride for proper alignment

                            log::debug!("Uniform buffer at binding {}: size = {} bytes, alignment = {}",
                                       binding.binding, struct_size, type_layout.alignment);

                            let min_binding_size = NonZeroU64::new(struct_size);

                            (wgt::BindingType::Buffer {
                                ty: wgt::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size,
                            }, BindingLayoutType::UniformBuffer, Some(struct_size), var_name.clone())
                        }
                        naga::AddressSpace::Storage { access: _ } => {
                            // Storage buffer (like wgpu-mc uses for uniforms/projection)
                            let type_layout = layouter[global_var.ty];
                            let struct_size = type_layout.to_stride() as u64;

                            log::debug!("Storage buffer at binding {}: size = {} bytes",
                                       binding.binding, struct_size);

                            let min_binding_size = NonZeroU64::new(struct_size);

                            (wgt::BindingType::Buffer {
                                ty: wgt::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size,
                            }, BindingLayoutType::StorageBuffer, Some(struct_size), var_name.clone())
                        }
                        naga::AddressSpace::Handle => {
                            // Check if it's a texture or sampler
                            match &ty.inner {
                                naga::TypeInner::Image { dim, arrayed, class: _ } => {
                                    // Convert naga dimension to wgpu dimension
                                    let view_dimension = match (dim, arrayed) {
                                        (naga::ImageDimension::D1, false) => wgt::TextureViewDimension::D1,
                                        (naga::ImageDimension::D2, false) => wgt::TextureViewDimension::D2,
                                        (naga::ImageDimension::D2, true) => wgt::TextureViewDimension::D2Array,
                                        (naga::ImageDimension::D3, _) => wgt::TextureViewDimension::D3,
                                        (naga::ImageDimension::Cube, false) => wgt::TextureViewDimension::Cube,
                                        (naga::ImageDimension::Cube, true) => wgt::TextureViewDimension::CubeArray,
                                        _ => wgt::TextureViewDimension::D2, // Default fallback
                                    };
                                    log::info!("Found texture at binding {}: dimension {:?} (naga dim={:?}, arrayed={}), name={:?})", binding.binding, view_dimension, dim, arrayed, var_name);
                                    (wgt::BindingType::Texture {
                                        sample_type: wgt::TextureSampleType::Float { filterable: true },
                                        view_dimension,
                                        multisampled: false,
                                    }, BindingLayoutType::Texture, None, var_name.clone())
                                }
                                naga::TypeInner::Sampler { .. } => {
                                    log::info!("Found sampler at binding {}: name={:?}", binding.binding, var_name);
                                    (wgt::BindingType::Sampler(wgt::SamplerBindingType::Filtering),
                                     BindingLayoutType::Sampler, None, var_name.clone())
                                }
                                _ => continue, // Skip unsupported types
                            }
                        }
                        _ => continue, // Skip other address spaces
                    };

                    // Always use VERTEX | FRAGMENT for maximum compatibility
                    // (even if shader only uses it in one stage)
                    let visibility = wgt::ShaderStages::VERTEX | wgt::ShaderStages::FRAGMENT;

                    bindings.entry(binding.binding)
                        .and_modify(|(e, _, min_sz, name)| {
                            e.visibility |= visibility;
                            // Keep the larger min_binding_size if both shaders define it
                            if let Some(new_size) = min_size {
                                *min_sz = Some(min_sz.map_or(new_size, |old| old.max(new_size)));
                            }
                            // Prefer non-None variable name
                            if name.is_none() && var_name.is_some() {
                                *name = var_name.clone();
                            }
                        })
                        .or_insert((wgt::BindGroupLayoutEntry {
                            binding: binding.binding,
                            visibility,
                            ty: binding_type,
                            count: None,
                        }, layout_type, min_size, binding_var_name));
                }
            }
        }
    };

    // Extract bindings from both shaders
    extract_bindings(vertex_module, &vertex_layouter, wgt::ShaderStages::VERTEX);
    extract_bindings(fragment_module, &fragment_layouter, wgt::ShaderStages::FRAGMENT);

    // Log final bindings after merging
    log::info!("Final merged bindings: {} entries", bindings.len());
    for (binding_num, (_entry, ty, _min_size, var_name)) in &bindings {
        log::info!("  Binding {}: {:?}, name: {:?}", binding_num, ty, var_name);
    }

    let global = context.inner();

    // Create single bind group layout for group 0
    let layout_entries: Vec<wgt::BindGroupLayoutEntry> = bindings.values()
        .map(|(e, _, _, _)| e.clone())
        .collect();

    let bgl_desc = binding_model::BindGroupLayoutDescriptor {
        label: Some(Cow::Borrowed("Pipeline Bind Group Layout (group 0)")),
        entries: Cow::Owned(layout_entries),
    };

    let (bgl_id, bgl_error) = global.device_create_bind_group_layout(device_id, &bgl_desc, None);

    if let Some(e) = bgl_error {
        return Err(BasaltError::resource_creation(
            "bind group layout",
            format!("Failed to create bind group layout: {:?}", e)
        ));
    }

    log::debug!("Created bind group layout for group 0: {:?}", bgl_id);

    // Collect binding layouts (group is always 0, omitted)
    let binding_layouts: Vec<BindingLayoutEntry> = bindings.iter()
        .map(|(binding, (entry, ty, min_size, var_name))| {
            let expected_dimension = if let wgt::BindingType::Texture { view_dimension, .. } = entry.ty {
                Some(view_dimension)
            } else {
                None
            };
            BindingLayoutEntry {
                binding: *binding,
                ty: *ty,
                min_binding_size: *min_size,
                expected_dimension,
                variable_name: var_name.clone(),
            }
        })
        .collect();

    log::info!("Creating pipeline layout with {} bindings:", binding_layouts.len());
    for (i, layout) in binding_layouts.iter().enumerate() {
        log::info!("  [{}] binding={}, ty={:?}, var_name={:?}",
            i, layout.binding, layout.ty, layout.variable_name);
    }

    // Create pipeline layout with single bind group
    let pl_desc = binding_model::PipelineLayoutDescriptor {
        label: Some(Cow::Borrowed("Pipeline Layout")),
        bind_group_layouts: Cow::Owned(vec![bgl_id]),
        // No immediates - using uniform buffers via bind groups instead
        immediate_size: 0,
    };

    let (pl_id, pl_error) = global.device_create_pipeline_layout(device_id, &pl_desc, None);

    if let Some(e) = pl_error {
        return Err(BasaltError::resource_creation(
            "pipeline layout",
            format!("Failed to create pipeline layout: {:?}", e)
        ));
    }

    Ok((bgl_id, pl_id, binding_layouts))
}

/// Create a render pipeline from pre-converted WGSL shaders
/// Uses PipelineCache for fast shader compilation and pipeline reuse
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_createNativePipelineFromWgsl(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    vertex_shader: JString,
    fragment_shader: JString,
    vertex_format: jint,
    primitive_topology: jint,
    depth_test_enabled: jboolean,
    depth_write_enabled: jboolean,
    depth_compare: jint,
    blend_enabled: jboolean,
    blend_src_color_factor: jint,
    blend_dst_color_factor: jint,
    blend_src_alpha_factor: jint,
    blend_dst_alpha_factor: jint,
) -> jlong {
    // Validate device pointer
    if device_ptr == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null device pointer");
        return 0;
    }

    // Get the device from the pointer - use the SAME device that was created during initialization
    let device = unsafe { &*(device_ptr as *const BasaltDevice) };
    let device_context = device.context();
    let device_id = device.id();

    // Check for null shaders
    if vertex_shader.is_null() {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Vertex shader string is null");
        return 0;
    }

    if fragment_shader.is_null() {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Fragment shader string is null");
        return 0;
    }

    // Extract WGSL strings from Java
    let vertex_wgsl: String = match env.get_string(&vertex_shader) {
        Ok(s) => s.into(),
        Err(e) => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", &format!("Invalid vertex shader string: {}", e));
            return 0;
        }
    };

    let fragment_wgsl: String = match env.get_string(&fragment_shader) {
        Ok(s) => s.into(),
        Err(e) => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", &format!("Invalid fragment shader string: {}", e));
            return 0;
        }
    };

    // Parse WGSL shaders once for layout creation and caching
    log::debug!("Parsing WGSL shaders for layout reflection...");
    let vertex_module = match shader::parse_wgsl_named(&vertex_wgsl, "vertex_shader") {
        Ok(module) => module,
        Err(e) => {
            let msg = format!("Failed to parse vertex WGSL: {}", e);
            log::error!("{}", msg);
            let _ = env.throw_new("java/lang/RuntimeException", &msg);
            return 0;
        }
    };

    let fragment_module = match shader::parse_wgsl_named(&fragment_wgsl, "fragment_shader") {
        Ok(module) => module,
        Err(e) => {
            let msg = format!("Failed to parse fragment WGSL: {}", e);
            log::error!("{}", msg);
            let _ = env.throw_new("java/lang/RuntimeException", &msg);
            return 0;
        }
    };
    log::debug!("WGSL shaders parsed for layout");

    // Create pipeline layout from shader reflection (needed for cache key)
    let (bind_group_layout_id, pipeline_layout_id, binding_layouts) = match create_layout_from_shaders(
        device_context,
        device_id,
        &vertex_module,
        &fragment_module,
    ) {
        Ok(layouts) => layouts,
        Err(e) => {
            let msg = format!("Failed to create pipeline layout from shaders: {:?}", e);
            log::error!("{}", msg);
            let _ = env.throw_new("java/lang/RuntimeException", &msg);
            return 0;
        }
    };
    log::debug!("Pipeline layout created for cache");

    // Map pipeline parameters
    let primitive_topology = match primitive_topology as u32 {
        0 => wgt::PrimitiveTopology::PointList,
        1 => wgt::PrimitiveTopology::LineList,
        2 => wgt::PrimitiveTopology::LineStrip,
        3 => wgt::PrimitiveTopology::TriangleList,
        4 => wgt::PrimitiveTopology::TriangleStrip,
        _ => wgt::PrimitiveTopology::TriangleList,
    };

    let depth_compare = match depth_compare as u32 {
        0 => wgt::CompareFunction::Never,
        1 => wgt::CompareFunction::Less,
        2 => wgt::CompareFunction::Equal,
        3 => wgt::CompareFunction::LessEqual,
        4 => wgt::CompareFunction::Greater,
        5 => wgt::CompareFunction::NotEqual,
        6 => wgt::CompareFunction::GreaterEqual,
        7 => wgt::CompareFunction::Always,
        _ => wgt::CompareFunction::Less,
    };

    // Detect post-processing shaders - they should use alpha blending to avoid overwriting GUI
    // Post-processing shaders typically:
    // 1. Have "minecraft:post/" in comments (original shader path)
    // 2. Use "input_texture" or "source_texture" (sampling from previous pass)
    // 3. Are blur/outline effects that need to blend with existing content
    let is_post_processing = fragment_wgsl.contains("minecraft:post/") ||
                             fragment_wgsl.contains("input_texture") ||
                             fragment_wgsl.contains("source_texture");

    // Force blending on for post-processing shaders to prevent overwriting GUI
    let effective_blend_enabled = if is_post_processing {
        log::info!("Detected post-processing shader, forcing alpha blending enabled");
        true
    } else {
        blend_enabled != 0
    };

    // Depth format - check if fragment shader writes depth, otherwise disable depth testing
    // GUI shaders and other 2D shaders don't write depth, so they shouldn't have depth state
    // Note: fragment_module was already parsed above, reuse it instead of re-parsing
    let shader_has_depth_output = shader_writes_depth(&fragment_module);
    let depth_format = if shader_has_depth_output {
        resource_handles::PipelineDepthFormat::Depth32Float
    } else {
        log::info!("Fragment shader does not write depth, disabling depth testing for this pipeline");
        resource_handles::PipelineDepthFormat::None
    };

    // Use PipelineCache for fast pipeline creation
    // The cache will:
    // 1. Check if we've seen this (vertex_shader, fragment_shader, topology, depth, blend) combo before
    // 2. If cached, return immediately
    // 3. If not, compile shaders and create pipeline, then cache for next time
    let cache_key = pipeline_registry::RenderPipelineKey {
        vertex_shader_hash: pipeline_registry::PipelineCache::hash_wgsl(&vertex_wgsl),
        fragment_shader_hash: pipeline_registry::PipelineCache::hash_wgsl(&fragment_wgsl),
        topology: primitive_topology,
        depth_test_enabled: depth_test_enabled != 0,
        depth_write_enabled: depth_write_enabled != 0,
        depth_compare,
        blend_enabled: effective_blend_enabled,
        blend_src_color_factor: if effective_blend_enabled { map_blend_factor_from_jni(blend_src_color_factor) } else { None },
        blend_dst_color_factor: if effective_blend_enabled { map_blend_factor_from_jni(blend_dst_color_factor) } else { None },
        blend_src_alpha_factor: if effective_blend_enabled { map_blend_factor_from_jni(blend_src_alpha_factor) } else { None },
        blend_dst_alpha_factor: if effective_blend_enabled { map_blend_factor_from_jni(blend_dst_alpha_factor) } else { None },
        target_format: device.swapchain_format(),  // Use actual swapchain format for compatibility
        depth_format,  // CRITICAL: Include depth format in cache key!
        depth_bias_constant: 0,  // TODO: Pass from Java when Minecraft uses depth bias
        depth_bias_slope_scale: 0,  // TODO: Pass from Java when Minecraft uses depth bias (stored as f32 bits)
    };

    let label = format!("NativePipeline_vfmt{}", vertex_format);

    log::debug!("Creating pipeline with vertex_format={}, topology={:?}, label={}, depth_test={}, blend={}",
        vertex_format, primitive_topology, label, depth_test_enabled, blend_enabled);
    log::debug!("Checking pipeline cache for key hash {:x}...", pipeline_registry::PipelineCache::hash_key(&cache_key));

    let cached_pipeline = match device.pipeline_cache.get_or_create_render_pipeline(
        device_context,
        device_id,
        cache_key,
        &vertex_wgsl,
        &fragment_wgsl,
        pipeline_layout_id,
        bind_group_layout_id,
        binding_layouts.clone(),
        depth_format,
        vertex_format as usize,
        &label,
    ) {
        Ok(pipeline) => pipeline,
        Err(e) => {
            let msg = format!("Failed to create pipeline (via cache): {:?}", e);
            log::error!("{}", msg);
            let _ = env.throw_new("java/lang/RuntimeException", &msg);
            return 0;
        }
    };

    // Log cache statistics
    let stats = device.pipeline_cache.stats();
    log::debug!("Pipeline cache stats - shader_hits: {}, shader_misses: {}, pipeline_hits: {}, pipeline_misses: {}, total_pipelines: {}",
        stats.shader_hits, stats.shader_misses, stats.pipeline_hits, stats.pipeline_misses, stats.total_pipelines);

    let pipeline_id = cached_pipeline.pipeline_id;
    log::debug!("Render pipeline created successfully via cache!");

    let num_bindings = binding_layouts.len();
    let handle = HANDLES.insert_render_pipeline(
        pipeline_id,
        bind_group_layout_id,
        binding_layouts,
        depth_format,
        depth_write_enabled != 0,  // Convert jboolean to bool
        depth_test_enabled != 0,   // Convert jboolean to bool
    );
    log::debug!("Created render pipeline via cache with handle {} (bgl: {:?}, bindings: {}, depth: {:?})",
               handle, bind_group_layout_id, num_bindings, depth_format);
    handle as jlong
}

// ============================================================================
// RENDER PASS OPERATIONS
// ============================================================================

/// Begin a render pass
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_beginRenderPass(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    color_view_handle: jlong,
    depth_view_handle: jlong,
    should_clear_color: jboolean,
    clear_color: jint,
    should_clear_depth: jboolean,
    clear_depth: jfloat,
    clear_stencil: jint,
    width: jint,
    height: jint,
) -> jlong {
    if device_ptr == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null device pointer");
        return 0;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Look up texture view IDs from handles
    let color_view = if color_view_handle != 0 {
        let view = HANDLES.get_texture_view(color_view_handle as u64);
        log::debug!("beginRenderPass: color_view_handle={}, resolved={:?}", color_view_handle, view);
        view
    } else {
        log::warn!("beginRenderPass: No color view handle provided!");
        None
    };

    // Use clear parameters from Java
    let do_clear_color = should_clear_color != 0;
    let clear_color_argb = clear_color as u32;

    // Always need depth view since pipelines always have depth_stencil
    // If MC doesn't provide one, create a matching-size depth texture
    let depth_view = if depth_view_handle != 0 {
        HANDLES.get_texture_view(depth_view_handle as u64)
    } else {
        // Create depth texture matching color texture dimensions
        log::debug!("MC didn't provide depth texture, creating one for {}x{}", width, height);
        match device.get_or_create_depth_view(width as u32, height as u32) {
            Ok(view) => Some(view),
            Err(e) => {
                log::error!("Failed to create depth texture: {}", e);
                None
            }
        }
    };

    // Extract the output texture ID from the color view for main framebuffer tracking
    // This will be set as the main framebuffer AFTER the render pass completes
    // IMPORTANT: Use our reliable view-to-texture mapping instead of potentially stale TextureViewInfo
    let output_texture = color_view.and_then(|resolved_view| {
        // Use our context's view-to-texture mapping for reliable lookups
        let tex_from_context = device.context().get_texture_from_view(resolved_view);

        // Fallback: check TextureViewInfo if not in our mapping (for externally created views)
        if tex_from_context.is_some() {
            tex_from_context
        } else {
            HANDLES.get_texture_view_info(color_view_handle as u64)
                .and_then(|view_info| {
                    if view_info.id == resolved_view {
                        log::debug!("beginRenderPass: Using TextureViewInfo fallback: texture {:?} (view={:?})",
                            view_info.texture_id, view_info.id);
                        Some(view_info.texture_id)
                    } else {
                        log::warn!("beginRenderPass: Stale TextureViewInfo! Stored view_id={:?} but resolved={:?}",
                            view_info.id, resolved_view);
                        None
                    }
                })
        }
    }).map(|tex_id| {
        log::debug!("beginRenderPass: output texture will be {:?} (from view={:?})", tex_id, color_view.unwrap());
        tex_id
    });

    // **CRITICAL FIX:** Auto-clear uninitialized textures
    // If texture hasn't been rendered to before, automatically clear it
    // This prevents black screen from LOAD on uninitialized textures
    let (do_clear_color, clear_color_argb) = if let Some(tex_id) = output_texture {
        let mut initialized = device.initialized_textures.lock();
        if !do_clear_color && !initialized.contains(&tex_id) {
            log::debug!("Auto-clearing uninitialized texture {:?} (first use)", tex_id);
            initialized.insert(tex_id);
            (true, 0xFF000000) // Clear to opaque black
        } else {
            (do_clear_color, clear_color_argb)
        }
    } else {
        (do_clear_color, clear_color_argb)
    };

    log::debug!("beginRenderPass: should_clear_color={}, should_clear_depth={}, output_texture={:?}",
        do_clear_color, should_clear_depth != 0, output_texture);

    // Create render pass state
    match render_pass::RenderPassState::new(
        device.context().clone(),
        device.id(),
        device.queue_id(),
        color_view,
        depth_view,
        output_texture, // Pass output texture for main framebuffer tracking
        do_clear_color,
        clear_color_argb,
        should_clear_depth != 0,
        clear_depth,
        clear_stencil as u32,
        width as u32,
        height as u32,
    ) {
        Ok(state) => {
            // Box the state and return as pointer
            let boxed = Box::new(state);
            let ptr = Box::into_raw(boxed);
            log::debug!("Created render pass at {:?}", ptr);
            ptr as jlong
        }
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to create render pass: {}", e));
            0
        }
    }
}

/// Set pipeline in render pass
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_setPipeline(
    _env: JNIEnv,
    _class: JClass,
    _device_ptr: jlong,
    render_pass_ptr: jlong,
    pipeline_handle: jlong,
) {
    if render_pass_ptr == 0 {
        return;
    }

    let state = unsafe { &mut *(render_pass_ptr as *mut render_pass::RenderPassState) };

    // Get pipeline info to extract depth write mode
    if let Some(pipeline_info) = HANDLES.get_render_pipeline_info(pipeline_handle as u64) {
        let has_depth_output = !matches!(pipeline_info.depth_format,
            resource_handles::PipelineDepthFormat::None);

        state.record_set_pipeline(
            pipeline_info.id,
            pipeline_info.depth_write_enabled,
            pipeline_info.depth_test_enabled,
            has_depth_output,
        );
        log::debug!("Recorded setPipeline (pipeline={}) depth_write={}, depth_test={}, has_depth={}",
            pipeline_handle, pipeline_info.depth_write_enabled,
            pipeline_info.depth_test_enabled, has_depth_output);
    } else {
        log::error!("Invalid pipeline handle: {}", pipeline_handle);
    }
}

/// Set vertex buffer
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_setVertexBuffer(
    _env: JNIEnv,
    _class: JClass,
    _device_ptr: jlong,
    render_pass_ptr: jlong,
    slot: jint,
    buffer_handle: jlong,
    offset: jlong,
) {
    log::debug!("[BassaltNative] setVertexBuffer called: slot={}, buffer_handle={}, offset={}", slot, buffer_handle, offset);

    if render_pass_ptr == 0 {
        log::error!("setVertexBuffer: render_pass_ptr is null!");
        return;
    }

    if buffer_handle == 0 {
        log::error!("setVertexBuffer: buffer_handle is null!");
        return;
    }

    let state = unsafe { &mut *(render_pass_ptr as *mut render_pass::RenderPassState) };

    if let Some(buffer_id) = HANDLES.get_buffer(buffer_handle as u64) {
        state.record_set_vertex_buffer(slot as u32, buffer_id, offset as u64, None);
        log::debug!("[BassaltNative] setVertexBuffer: slot={}, buffer={:?}, offset={}", slot, buffer_id, offset);
    } else {
        log::error!("setVertexBuffer: Invalid buffer handle: {}", buffer_handle);
    }
}

/// Set index buffer
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_setIndexBuffer(
    _env: JNIEnv,
    _class: JClass,
    _device_ptr: jlong,
    render_pass_ptr: jlong,
    buffer_handle: jlong,
    index_type: jint,
    offset: jlong,
) {
    log::debug!("[BassaltNative] setIndexBuffer called: buffer_handle={}, index_type={}, offset={}", buffer_handle, index_type, offset);

    if render_pass_ptr == 0 {
        log::error!("setIndexBuffer: render_pass_ptr is null!");
        return;
    }

    if buffer_handle == 0 {
        log::error!("setIndexBuffer: buffer_handle is null!");
        return;
    }

    let state = unsafe { &mut *(render_pass_ptr as *mut render_pass::RenderPassState) };

    let index_format = match index_type {
        0 => wgt::IndexFormat::Uint16,
        1 => wgt::IndexFormat::Uint32,
        _ => {
            log::error!("setIndexBuffer: Invalid index type: {}", index_type);
            return;
        }
    };

    if let Some(buffer_id) = HANDLES.get_buffer(buffer_handle as u64) {
        // **VALIDATION**: Calculate and store max index count for validation during draw calls
        if let Some(buffer_info) = HANDLES.get_buffer_info(buffer_handle as u64) {
            let bytes_per_index = match index_format {
                wgt::IndexFormat::Uint16 => 2,
                wgt::IndexFormat::Uint32 => 4,
            };
            // Check if offset is within buffer bounds
            if offset as u64 >= buffer_info.size {
                log::error!("setIndexBuffer: offset {} exceeds buffer size {}", offset, buffer_info.size);
                return;
            }
            let available_bytes = buffer_info.size - offset as u64;
            let max_indices = available_bytes / bytes_per_index as u64;
            state.set_max_index_count(max_indices);
            log::debug!("setIndexBuffer: Max indices = {} (buffer size={}, offset={})",
                max_indices, buffer_info.size, offset);
        }

        state.record_set_index_buffer(buffer_id, index_format, offset as u64, None);
        log::debug!("[BassaltNative] setIndexBuffer: buffer={:?}, index_format={:?}", buffer_id, index_format);
    } else {
        log::error!("setIndexBuffer: Invalid buffer handle: {}", buffer_handle);
    }
}

/// Draw indexed
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_drawIndexed(
    _env: JNIEnv,
    _class: JClass,
    _device_ptr: jlong,
    render_pass_ptr: jlong,
    index_count: jint,
    instance_count: jint,
    first_index: jint,
    base_vertex: jint,
    first_instance: jint,
) {
    log::debug!("NATIVE drawIndexed called: render_pass_ptr={}, indices={}", render_pass_ptr, index_count);

    if render_pass_ptr == 0 {
        log::error!("NATIVE drawIndexed: render_pass_ptr is 0, returning early!");
        return;
    }

    let state = unsafe { &mut *(render_pass_ptr as *mut render_pass::RenderPassState) };

    // **VALIDATION**: Check if index count is within buffer bounds
    if let Some(max_indices) = state.get_max_index_count() {
        let last_index = first_index as u64 + index_count as u64;
        if last_index > max_indices {
            log::error!("drawIndexed: Index count {} + first_index {} exceeds buffer size {}",
                index_count, first_index, max_indices);
            return;
        }
    }

    state.record_draw_indexed(
        index_count as u32,
        instance_count as u32,
        first_index as u32,
        base_vertex,
        first_instance as u32,
    );

    log::debug!("NATIVE drawIndexed: Recorded draw (indices={}, instances={}, first={}, base={}, firstInst={})",
        index_count, instance_count, first_index, base_vertex, first_instance);
}

/// Draw (non-indexed)
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_draw(
    _env: JNIEnv,
    _class: JClass,
    _device_ptr: jlong,
    render_pass_ptr: jlong,
    vertex_count: jint,
    instance_count: jint,
    first_vertex: jint,
    first_instance: jint,
) {
    if render_pass_ptr == 0 {
        return;
    }

    let state = unsafe { &mut *(render_pass_ptr as *mut render_pass::RenderPassState) };

    state.record_draw(
        vertex_count as u32,
        instance_count as u32,
        first_vertex as u32,
        first_instance as u32,
    );

    log::debug!("Recorded draw (vertices={}, instances={}, first={}, firstInst={})",
        vertex_count, instance_count, first_vertex, first_instance);
}

/// Set scissor rect
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_setScissorRect(
    _env: JNIEnv,
    _class: JClass,
    _device_ptr: jlong,
    render_pass_ptr: jlong,
    x: jint,
    y: jint,
    width: jint,
    height: jint,
) {
    if render_pass_ptr == 0 {
        return;
    }

    let state = unsafe { &mut *(render_pass_ptr as *mut render_pass::RenderPassState) };

    state.record_set_scissor_rect(x as u32, y as u32, width as u32, height as u32);

    log::debug!("Recorded setScissorRect (x={}, y={}, width={}, height={})",
        x, y, width, height);
}

/// Set push constants for per-draw data
///
/// This allows passing small amounts of data (up to 128 bytes) directly to shaders
/// without creating uniform buffers. Useful for:
/// - Model matrices
/// - Per-draw colors
/// - Animation parameters
///
/// # Arguments
/// * `render_pass_ptr` - The active render pass
/// * `offset` - Byte offset within the push constant range (must be 4-byte aligned)
/// * `data` - The data to write (as byte array, must be 4-byte aligned)
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_setPushConstants(
    env: JNIEnv,
    _class: JClass,
    _device_ptr: jlong,
    render_pass_ptr: jlong,
    offset: jint,
    data: JByteArray,
) {
    if render_pass_ptr == 0 {
        return;
    }

    let state = unsafe { &mut *(render_pass_ptr as *mut render_pass::RenderPassState) };

    // Convert Java byte array to Rust Vec
    let data_vec: Vec<u8> = match env.convert_byte_array(&data) {
        Ok(arr) => arr,
        Err(e) => {
            log::error!("Failed to get byte array for push constants: {}", e);
            return;
        }
    };

    // Ensure data is 4-byte aligned
    if data_vec.len() % 4 != 0 {
        log::error!("Push constants data must be 4-byte aligned, got {} bytes", data_vec.len());
        return;
    }

    state.record_set_push_constants_all(offset as u32, &data_vec);

    log::debug!("Recorded setPushConstants (offset={}, size={})", offset, data_vec.len());
}

/// End render pass and submit
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_endRenderPass(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    render_pass_ptr: jlong,
) {
    if render_pass_ptr == 0 || device_ptr == 0 {
        return;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Take ownership of the boxed RenderPassState
    let mut state = unsafe { Box::from_raw(render_pass_ptr as *mut render_pass::RenderPassState) };

    // Finish and submit - returns the output texture that was rendered
    match state.finish_and_submit(device.context().as_ref(), device.queue_id()) {
        Ok(output_texture) => {
            // Set the main framebuffer AFTER the render pass has successfully executed
            // This fixes the race condition where present_frame could be called before rendering completes
            if let Some(texture_id) = output_texture {
                device.set_main_framebuffer(texture_id);
                log::debug!("endRenderPass: Set main framebuffer to {:?} after successful render", texture_id);
            } else {
                log::debug!("endRenderPass: No output texture from this render pass");
            }
            log::debug!("Ended render pass at {:?}", render_pass_ptr as *const ());
        }
        Err(e) => {
            log::error!("Failed to end render pass: {}", e);
        }
    }

    // State is dropped here
}

// ============================================================================
// BIND GROUP OPERATIONS
// ============================================================================

/// Create a bind group from arrays of texture, sampler, and uniform bindings
/// Now takes a pipeline_handle to retrieve the correct bind group layout
/// Also takes uniform_offsets and uniform_sizes to properly bind buffer slices
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_pipeline_BassaltRenderPass_createBindGroup0(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    _render_pass_ptr: jlong,
    pipeline_handle: jlong,
    texture_names: JObject,
    texture_handles: JObject,
    sampler_handles: JObject,
    uniform_names: JObject,
    uniform_handles: JObject,
    uniform_offsets: JObject,
    uniform_sizes: JObject,
) -> jlong {
    if device_ptr == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null device pointer");
        return 0;
    }

    let device = unsafe { &*(device_ptr as *const device::BasaltDevice) };
    let context = device.context().clone();
    let device_id = device.id();

    // Get pipeline's bind group layout if pipeline handle is provided
    let pipeline_layout = if pipeline_handle != 0 {
        HANDLES.get_render_pipeline_info(pipeline_handle as u64)
    } else {
        None
    };

    // Create bind group builder
    let mut builder = bind_group::BindGroupBuilder::new(context.clone(), device_id);

    // Add texture bindings using NAME-based lookup (matches shader reflection)
    if !texture_handles.is_null() && !texture_names.is_null() {
        let tex_array: ::jni::objects::JPrimitiveArray<i64> = texture_handles.into();
        let samp_array: ::jni::objects::JPrimitiveArray<i64> = sampler_handles.into();
        let names_array: ::jni::objects::JObjectArray = texture_names.into();

        let texture_count = match env.get_array_length(&tex_array) {
            Ok(len) => len as usize,
            Err(_) => 0,
        };

        for i in 0..texture_count {
            // Get texture handle
            let mut tex_handle_buf = [0i64; 1];
            if env.get_long_array_region(&tex_array, i as i32, &mut tex_handle_buf).is_ok() {
                let tex_handle = tex_handle_buf[0];
                if tex_handle == 0 {
                    continue;
                }

                // Get texture name from Java array
                let texture_name: Option<String> = match env.get_object_array_element(&names_array, i as i32) {
                    Ok(name_obj) => {
                        if name_obj.is_null() {
                            None
                        } else {
                            match env.get_string(&name_obj.into()) {
                                Ok(jstr) => Some(jstr.to_string_lossy().into_owned()),
                                Err(_) => None,
                            }
                        }
                    }
                    Err(_) => None,
                };

                // Keep a reference for log messages (clone to avoid move)
                let texture_name_log = texture_name.clone();

                // Get texture view info which includes the dimension
                if let Some(view_info) = HANDLES.get_texture_view_info(tex_handle as u64) {
                    // Get sampler handle (if available)
                    let mut samp_handle_buf = [0i64; 1];
                    let sampler_id = if env.get_long_array_region(&samp_array, i as i32, &mut samp_handle_buf).is_ok() {
                        let samp_handle = samp_handle_buf[0];
                        if samp_handle != 0 {
                            HANDLES.get_sampler(samp_handle as u64)
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    // Find the correct binding slot by matching the texture name
                    let binding_slot = if let (Some(pipeline_info), Some(mc_name)) = (pipeline_layout.as_ref(), texture_name) {
                        let mc_name_lower = mc_name.to_lowercase();

                        // Try to match texture name to shader binding slot
                        let slot = pipeline_info.binding_layouts.iter()
                            .filter(|layout| layout.ty == resource_handles::BindingLayoutType::Texture)
                            .find(|layout| {
                                if let Some(ref var_name) = layout.variable_name {
                                    // Case-insensitive matching with common variations
                                    let var_lower = var_name.to_lowercase();
                                    var_lower == mc_name_lower ||
                                    var_lower.replace("_", "") == mc_name_lower ||
                                    var_lower.replace("_texture", "") == mc_name_lower.replace("texture", "") ||
                                    mc_name_lower.contains(&var_lower) ||
                                    var_lower.contains(&mc_name_lower)
                                } else {
                                    false
                                }
                            })
                            .map(|layout| layout.binding);

                        // Fallback: if no exact match, try sequential assignment
                        // This maintains compatibility with older code paths
                        if slot.is_none() {
                            log::debug!("No name match for texture '{}', using sequential assignment", mc_name);
                            // Use the current index to determine the binding slot
                            // Each texture+sampler pair uses 2 slots (texture at even, sampler at odd)
                            Some(i as u32 * 2)
                        } else {
                            slot
                        }
                    } else {
                        // No pipeline info or no name provided - use sequential assignment
                        // Each texture+sampler pair uses 2 slots (texture at even, sampler at odd)
                        Some(i as u32 * 2)
                    };

                    if let Some(slot) = binding_slot {
                        builder = builder.add_texture(slot, view_info.id, sampler_id, view_info.dimension, view_info.texture_id);
                        log::debug!("Bound texture '{}' to slot {}", texture_name_log.unwrap_or_else(|| format!("#{}", i)), slot);
                    } else {
                        log::warn!("Failed to find binding slot for texture '{}'", texture_name_log.unwrap_or_else(|| format!("#{}", i)));
                    }
                }
            }
        }
    } else if !texture_handles.is_null() {
        // Fallback: no texture names provided, use sequential ordering
        let tex_array: ::jni::objects::JPrimitiveArray<i64> = texture_handles.into();
        let samp_array: ::jni::objects::JPrimitiveArray<i64> = sampler_handles.into();

        let texture_count = match env.get_array_length(&tex_array) {
            Ok(len) => len as usize,
            Err(_) => 0,
        };

        let mut texture_binding_slot = 0u32;
        for i in 0..texture_count {
            let mut tex_handle_buf = [0i64; 1];
            if env.get_long_array_region(&tex_array, i as i32, &mut tex_handle_buf).is_ok() {
                let tex_handle = tex_handle_buf[0];
                if tex_handle != 0 {
                    if let Some(view_info) = HANDLES.get_texture_view_info(tex_handle as u64) {
                        let mut samp_handle_buf = [0i64; 1];
                        let sampler_id = if env.get_long_array_region(&samp_array, i as i32, &mut samp_handle_buf).is_ok() {
                            let samp_handle = samp_handle_buf[0];
                            if samp_handle != 0 {
                                HANDLES.get_sampler(samp_handle as u64)
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        builder = builder.add_texture(texture_binding_slot, view_info.id, sampler_id, view_info.dimension, view_info.texture_id);
                        texture_binding_slot += if sampler_id.is_some() { 2 } else { 1 };
                    }
                }
            }
        }
    }

    // Add uniform buffer bindings using NAME-based lookup
    if !uniform_handles.is_null() && !uniform_names.is_null() {
        let unif_array: ::jni::objects::JPrimitiveArray<i64> = uniform_handles.into();
        let names_array: ::jni::objects::JObjectArray = uniform_names.into();
        let offsets_array: ::jni::objects::JPrimitiveArray<i64> = uniform_offsets.into();
        let sizes_array: ::jni::objects::JPrimitiveArray<i64> = uniform_sizes.into();

        let uniform_count = match env.get_array_length(&unif_array) {
            Ok(len) => len as usize,
            Err(_) => 0,
        };

        // Build a map from variable names (shader) to Minecraft uniform names
        // Example: shader has "dynamic_transforms" → Minecraft sends "DynamicTransforms"
        for i in 0..uniform_count {
            // Get uniform buffer handle
            let mut unif_handle_buf = [0i64; 1];
            if env.get_long_array_region(&unif_array, i as i32, &mut unif_handle_buf).is_ok() {
                let unif_handle = unif_handle_buf[0];
                if unif_handle == 0 {
                    continue;
                }

                // Get uniform name from Java array
                let uniform_name: Option<String> = match env.get_object_array_element(&names_array, i as i32) {
                    Ok(name_obj) => {
                        if name_obj.is_null() {
                            None
                        } else {
                            match env.get_string(&name_obj.into()) {
                                Ok(jstr) => Some(jstr.to_string_lossy().into_owned()),
                                Err(_) => None,
                            }
                        }
                    }
                    Err(_) => None,
                };

                if let (Some(buffer_info), Some(mc_name)) = (HANDLES.get_buffer_info(unif_handle as u64), uniform_name) {
                    // Find the correct binding slot by matching the uniform name
                    // Use wgpu-mc style direct name matching
                    let binding_slot = if let Some(ref pipeline_info) = pipeline_layout {
                        log::debug!("Looking for binding slot for uniform '{}', pipeline has {} bindings",
                                   mc_name, pipeline_info.binding_layouts.len());

                        // First try: exact variable name match (case insensitive)
                        let mut slot = pipeline_info.binding_layouts.iter()
                            .find(|layout| {
                                if let Some(ref var_name) = layout.variable_name {
                                    let matches = var_name.to_lowercase() == mc_name.to_lowercase() ||
                                        var_name.replace("_", "").to_lowercase() == mc_name.to_lowercase();
                                    if matches {
                                        log::debug!("  Exact match found: '{}' == '{}'", var_name, mc_name);
                                    }
                                    matches
                                } else {
                                    false
                                }
                            })
                            .map(|layout| layout.binding);

                        // Second try: fuzzy matching for known uniforms
                        if slot.is_none() {
                            log::debug!("  No exact match, trying fuzzy match for '{}'", mc_name);
                            slot = match mc_name.as_str() {
                                // For common uniforms, find matching uniform buffer by name patterns
                                "Lighting" | "Projection" | "DynamicTransforms" | "DynamicUniforms" | "Fog" |
                                "ColorModulator" | "GameTime" | "ScreenSize" |
                                "Globals" | "ModelViewMat" | "ProjMat" => {
                                    // Try to match by simplified names
                                    let mc_lower = mc_name.to_lowercase();
                                    let mc_simple = mc_lower
                                        .replace("dynamic", "")
                                        .replace("uniforms", "")  // Handle both DynamicTransforms and DynamicUniforms
                                        .replace("color", "")
                                        .replace("modulator", "")
                                        .replace("game", "")
                                        .replace("screen", "");

                                    pipeline_info.binding_layouts.iter()
                                        .find(|l| {
                                            if l.ty != crate::resource_handles::BindingLayoutType::UniformBuffer {
                                                return false;
                                            }
                                            if let Some(ref var_name) = l.variable_name {
                                                let var_lower = var_name.to_lowercase();
                                                // Match if simplified names match, or if one contains the other
                                                let matches = var_lower == mc_simple ||
                                                    var_lower.contains(&mc_simple) ||
                                                    mc_lower.contains(&var_lower) ||
                                                    // Special case: "uniforms" and "transforms" are interchangeable
                                                    (var_lower == "uniforms" && mc_simple == "transforms") ||
                                                    (var_lower == "transforms" && mc_simple == "uniforms");
                                                if matches {
                                                    log::debug!("  Fuzzy match: '{}' ~= '{}' (base: '{}')",
                                                               var_name, mc_name, mc_simple);
                                                }
                                                matches
                                            } else {
                                                false
                                            }
                                        })
                                        .map(|l| l.binding)
                                }
                                _ => None,
                            };
                        }

                        if slot.is_none() {
                            log::warn!("No binding slot found for uniform '{}' (pipeline has {} bindings)",
                                       mc_name, pipeline_info.binding_layouts.len());
                        }

                        slot
                    } else {
                        None
                    };

                    if let Some(slot) = binding_slot {
                        // Get offset and size for this uniform buffer slice
                        let mut offset_buf = [0i64; 1];
                        let mut size_buf = [0i64; 1];

                        let offset = if env.get_long_array_region(&offsets_array, i as i32, &mut offset_buf).is_ok() {
                            offset_buf[0] as u64
                        } else {
                            0
                        };

                        let size = if env.get_long_array_region(&sizes_array, i as i32, &mut size_buf).is_ok() {
                            size_buf[0] as u64
                        } else {
                            buffer_info.size
                        };

                        log::debug!("Mapping uniform '{}' to binding slot {} (offset={}, size={})",
                                  mc_name, slot, offset, size);
                        builder = builder.add_uniform_buffer(slot, buffer_info.id, offset, size);
                    } else {
                        log::warn!("Failed to map uniform '{}' to any binding slot", mc_name);
                    }
                }
            }
        }
    }

    // Build the bind group - use pipeline layout if available, otherwise create new
    // For pipelines with 0 bindings, create an empty bind group (wgpu still requires it)
    if let Some(ref pipeline_info) = pipeline_layout {
        if pipeline_info.binding_layouts.is_empty() {
            log::debug!("Pipeline expects 0 bindings, creating empty bind group");
            // Use the builder to create an empty bind group with the pipeline's layout
            let empty_result = builder.build_with_layout(
                pipeline_info.bind_group_layout_id, 
                &pipeline_info.binding_layouts
            );
            match empty_result {
                Ok(bind_group_id) => {
                    let handle = HANDLES.insert_bind_group(bind_group_id);
                    log::debug!("Created empty bind group with handle {}", handle);
                    return handle as jlong;
                }
                Err(e) => {
                    let msg = format!("Failed to create empty bind group: {:?}", e);
                    log::error!("{}", msg);
                    let _ = env.throw_new("java/lang/RuntimeException", &msg);
                    return 0;
                }
            }
        }
    }
    
    let result = if let Some(ref pipeline_info) = pipeline_layout {
        log::debug!("Creating bind group with pipeline layout {:?} ({} bindings)", 
                   pipeline_info.bind_group_layout_id, pipeline_info.binding_layouts.len());
        builder.build_with_layout(pipeline_info.bind_group_layout_id, &pipeline_info.binding_layouts)
    } else {
        log::debug!("Creating bind group with dynamic layout (no pipeline specified)");
        builder.build()
    };

    match result {
        Ok(bind_group_id) => {
            let handle = HANDLES.insert_bind_group(bind_group_id);
            let binding_count = if let Some(ref pipeline_info) = pipeline_layout {
                pipeline_info.binding_layouts.len()
            } else {
                0
            };
            log::debug!("Created bind group 0 with {} bindings (handle={})", binding_count, handle);
            
            // If pipeline has more than 1 bind group layout, also set bind groups on render pass
            // and create empty bind groups for indices 1 and 2 if needed
            if _render_pass_ptr != 0 {
                let state = unsafe { &mut *(_render_pass_ptr as *mut render_pass::RenderPassState) };

                // Set bind group 0
                state.record_set_bind_group(0, Some(bind_group_id), Vec::new());
                log::debug!("Set bind group 0 with handle {} on render pass", handle);
            }

            handle as jlong
        }
        Err(e) => {
            // This is a critical failure - bind group creation failed
            let msg = format!("Failed to create bind group: {:?}", e);
            log::error!("{}", msg);
            let _ = env.throw_new("java/lang/RuntimeException", &msg);
            0
        }
    }
}


/// Create multiple bind groups (DEPRECATED - now uses single bind group approach)
///
/// This function is deprecated. All Bassalt shaders now use a single bind group (group 0).
/// Use createBindGroupWithLayout instead.
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_pipeline_BassaltRenderPass_createMultiBindGroups<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    _device_ptr: jlong,
    _render_pass_ptr: jlong,
    _pipeline_handle: jlong,
    _texture_names: JObject<'local>,
    _texture_handles: JObject<'local>,
    _sampler_handles: JObject<'local>,
    _uniform_names: JObject<'local>,
    _uniform_handles: JObject<'local>,
    _uniform_offsets: JObject<'local>,
    _uniform_sizes: JObject<'local>,
) -> jlong {
    log::warn!("createMultiBindGroups is deprecated and no longer supported. All shaders use single bind group (group 0). Use createBindGroupWithLayout instead.");
    let _ = env.throw_new("java/lang/UnsupportedOperationException", "createMultiBindGroups is deprecated. Use createBindGroupWithLayout instead.");
    0
}


/// Set a bind group on the render pass
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_pipeline_BassaltRenderPass_setBindGroup0(
    _env: JNIEnv,
    _class: JClass,
    _device_ptr: jlong,
    render_pass_ptr: jlong,
    index: jint,
    bind_group_handle: jlong,
) {
    if render_pass_ptr == 0 || bind_group_handle == 0 {
        return;
    }

    let state = unsafe { &mut *(render_pass_ptr as *mut render_pass::RenderPassState) };

    // Look up bind group ID
    if let Some(bind_group_id) = HANDLES.get_bind_group(bind_group_handle as u64) {
        log::debug!("setBindGroup0: setting bind group {:?} at index {}", bind_group_id, index);
        // Record the set bind group command
        state.record_set_bind_group(index as u32, Some(bind_group_id), Vec::new());
        log::debug!("Recorded setBindGroup (index={}, bind_group={})", index, bind_group_handle);
    } else {
        log::warn!("setBindGroup: invalid bind group handle {}", bind_group_handle);
        log::debug!("Bind group set (placeholder implementation)");
    }
}

// ============================================================================
// DEBUG GROUPS AND MARKERS
// ============================================================================

/// Push a debug group in the render pass
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_pipeline_BassaltRenderPass_pushDebugGroup(
    mut env: JNIEnv,
    _class: JClass,
    render_pass_ptr: jlong,
    label: JString,
) {
    if render_pass_ptr == 0 {
        return;
    }

    let state = unsafe { &mut *(render_pass_ptr as *mut render_pass::RenderPassState) };

    // Convert Java string to Rust String
    let label_str: String = match env.get_string(&label) {
        Ok(s) => s.into(),
        Err(e) => {
            log::error!("Failed to get label string: {:?}", e);
            return;
        }
    };

    state.record_push_debug_group(label_str.clone());
    log::debug!("Recorded pushDebugGroup: {}", label_str);
}

/// Pop a debug group in the render pass
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_pipeline_BassaltRenderPass_popDebugGroup(
    _env: JNIEnv,
    _class: JClass,
    render_pass_ptr: jlong,
) {
    if render_pass_ptr == 0 {
        return;
    }

    let state = unsafe { &mut *(render_pass_ptr as *mut render_pass::RenderPassState) };
    state.record_pop_debug_group();
    log::debug!("Recorded popDebugGroup");
}

/// Insert a debug marker in the render pass
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_pipeline_BassaltRenderPass_insertDebugMarker(
    mut env: JNIEnv,
    _class: JClass,
    render_pass_ptr: jlong,
    label: JString,
) {
    if render_pass_ptr == 0 {
        return;
    }

    let state = unsafe { &mut *(render_pass_ptr as *mut render_pass::RenderPassState) };

    // Convert Java string to Rust String
    let label_str: String = match env.get_string(&label) {
        Ok(s) => s.into(),
        Err(e) => {
            log::error!("Failed to get label string: {:?}", e);
            return;
        }
    };

    state.record_insert_debug_marker(label_str.clone());
    log::debug!("Recorded insertDebugMarker: {}", label_str);
}

// ============================================================================
// CLEAR OPERATIONS
// ============================================================================

/// Clear a color texture
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_pipeline_BassaltCommandEncoder_clearColorTexture0(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    texture_handle: jlong,
    clear_color: jint,
) {
    if device_ptr == 0 || texture_handle == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null pointer");
        return;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Look up texture ID
    let texture_id = match HANDLES.get_texture(texture_handle as u64) {
        Some(id) => id,
        None => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", "Invalid texture handle");
            return;
        }
    };

    // Convert clear color from packed ARGB (Minecraft format) to Color struct
    let a = ((clear_color >> 24) & 0xFF) as f64 / 255.0;
    let r = ((clear_color >> 16) & 0xFF) as f64 / 255.0;
    let g = ((clear_color >> 8) & 0xFF) as f64 / 255.0;
    let b = (clear_color & 0xFF) as f64 / 255.0;
    let color = wgt::Color { r, g, b, a };

    // Create a command encoder and clear the texture
    if let Err(e) = device.clear_texture(texture_id, Some(color), None) {
        let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to clear color texture: {}", e));
    }
}

/// Clear a depth texture
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_pipeline_BassaltCommandEncoder_clearDepthTexture0(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    texture_handle: jlong,
    clear_depth: jfloat,
) {
    if device_ptr == 0 || texture_handle == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null pointer");
        return;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Look up texture ID
    let texture_id = match HANDLES.get_texture(texture_handle as u64) {
        Some(id) => id,
        None => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", "Invalid texture handle");
            return;
        }
    };

    // Clear depth texture
    if let Err(e) = device.clear_texture(texture_id, None, Some(clear_depth)) {
        let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to clear depth texture: {}", e));
    }
}

/// Clear both color and depth textures (with region support)
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_pipeline_BassaltCommandEncoder_clearColorAndDepthTextures0(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    color_texture_handle: jlong,
    clear_color: jint,
    depth_texture_handle: jlong,
    clear_depth: jfloat,
    _x: jint,
    _y: jint,
    _width: jint,
    _height: jint,
) {
    if device_ptr == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null device pointer");
        return;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Clear color texture if provided
    if color_texture_handle != 0 {
        if let Some(color_id) = HANDLES.get_texture(color_texture_handle as u64) {
            let r = ((clear_color >> 24) & 0xFF) as f64 / 255.0;
            let g = ((clear_color >> 16) & 0xFF) as f64 / 255.0;
            let b = ((clear_color >> 8) & 0xFF) as f64 / 255.0;
            let a = (clear_color & 0xFF) as f64 / 255.0;
            let color = wgt::Color { r, g, b, a };

            if let Err(e) = device.clear_texture(color_id, Some(color), None) {
                let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to clear color texture: {}", e));
                return;
            }
        }
    }

    // Clear depth texture if provided
    if depth_texture_handle != 0 {
        if let Some(depth_id) = HANDLES.get_texture(depth_texture_handle as u64) {
            if let Err(e) = device.clear_texture(depth_id, None, Some(clear_depth)) {
                let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to clear depth texture: {}", e));
                return;
            }
        }
    }
}

/// Copy texture to texture
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_pipeline_BassaltCommandEncoder_copyTextureToTexture0(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    src_texture_handle: jlong,
    dst_texture_handle: jlong,
    mip_level: jint,
    dest_x: jint,
    dest_y: jint,
    source_x: jint,
    source_y: jint,
    width: jint,
    height: jint,
) {
    if device_ptr == 0 || src_texture_handle == 0 || dst_texture_handle == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null pointer");
        return;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Look up texture IDs
    let src_id = match HANDLES.get_texture(src_texture_handle as u64) {
        Some(id) => id,
        None => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", "Invalid source texture handle");
            return;
        }
    };

    let dst_id = match HANDLES.get_texture(dst_texture_handle as u64) {
        Some(id) => id,
        None => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", "Invalid destination texture handle");
            return;
        }
    };

    if let Err(e) = device.copy_texture_to_texture(
        src_id,
        dst_id,
        mip_level as u32,
        dest_x as u32,
        dest_y as u32,
        source_x as u32,
        source_y as u32,
        width as u32,
        height as u32,
    ) {
        let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to copy texture: {}", e));
    }
}

// ============================================================================
// COPY OPERATIONS
// ============================================================================

/// Write image data to texture
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_pipeline_BassaltCommandEncoder_writeToTexture0(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    texture_handle: jlong,
    data: JByteArray,
    mip_level: jint,
    _depth_or_layer: jint,
    dest_x: jint,
    dest_y: jint,
    width: jint,
    height: jint,
    _format: jint,
) {
    if device_ptr == 0 || texture_handle == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null pointer");
        return;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Look up texture ID
    let texture_id = match HANDLES.get_texture(texture_handle as u64) {
        Some(id) => id,
        None => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", "Invalid texture handle");
            return;
        }
    };

    // Convert Java byte array to Rust Vec
    let data_vec: Vec<u8> = match env.convert_byte_array(&data) {
        Ok(arr) => arr,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to get byte array: {}", e));
            return;
        }
    };

    if let Err(e) = device.write_texture(
        texture_id,
        &data_vec,
        mip_level as u32,
        dest_x as u32,
        dest_y as u32,
        _depth_or_layer as u32, // Use the depth_or_layer parameter as origin_z for cubemap faces
        width as u32,
        height as u32,
    ) {
        let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to write texture: {}", e));
    } else {
        log::debug!("Wrote {}x{} texture data ({} bytes) to texture {:?} at ({}, {}, layer={})",
                  width, height, data_vec.len(), texture_id, dest_x, dest_y, _depth_or_layer);
    }
}

/// Copy buffer to buffer
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_pipeline_BassaltCommandEncoder_copyToBuffer0(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    src_buffer_handle: jlong,
    dst_buffer_handle: jlong,
    src_offset: jlong,
    dst_offset: jlong,
    size: jlong,
) {
    if device_ptr == 0 || src_buffer_handle == 0 || dst_buffer_handle == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null pointer");
        return;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Look up buffer IDs
    let src_id = match HANDLES.get_buffer(src_buffer_handle as u64) {
        Some(id) => id,
        None => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", "Invalid source buffer handle");
            return;
        }
    };

    let dst_id = match HANDLES.get_buffer(dst_buffer_handle as u64) {
        Some(id) => id,
        None => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", "Invalid destination buffer handle");
            return;
        }
    };

    if let Err(e) = device.copy_buffer_to_buffer(
        src_id,
        src_offset as u64,
        dst_id,
        dst_offset as u64,
        size as u64,
    ) {
        let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to copy buffer: {}", e));
    } else {
        log::debug!("Copied {} bytes from buffer to buffer", size);
    }
}

/// Copy texture to buffer (readback)
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_pipeline_BassaltCommandEncoder_copyTextureToBuffer0(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    texture_handle: jlong,
    buffer_handle: jlong,
    buffer_offset: jlong,
    mip_level: jint,
    width: jint,
    height: jint,
) {
    if device_ptr == 0 || texture_handle == 0 || buffer_handle == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null pointer");
        return;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Look up texture and buffer IDs
    let texture_id = match HANDLES.get_texture(texture_handle as u64) {
        Some(id) => id,
        None => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", "Invalid texture handle");
            return;
        }
    };

    let buffer_id = match HANDLES.get_buffer(buffer_handle as u64) {
        Some(id) => id,
        None => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", "Invalid buffer handle");
            return;
        }
    };

    if let Err(e) = device.copy_texture_to_buffer(
        texture_id,
        buffer_id,
        buffer_offset as u64,
        mip_level as u32,
        width as u32,
        height as u32,
    ) {
        let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to copy texture to buffer: {}", e));
    } else {
        log::debug!("Copied {}x{} texture to buffer at offset {}", width, height, buffer_offset);
    }
}

// ============================================================================
// FENCE AND SYNCHRONIZATION
// ============================================================================

/// Get current submission index for fence tracking
/// In wgpu, we use device polling instead of explicit submission indices
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_sync_BassaltFence_getSubmissionIndex(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
) -> jlong {
    if device_ptr == 0 {
        return 0;
    }
    let device = unsafe { &*(device_ptr as *const BasaltDevice) };
    
    // Poll device to process any pending work
    let _ = device.context().inner().device_poll(
        device.id(),
        wgt::PollType::Poll,
    );
    
    // Return current timestamp as submission index (simplified)
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
}

/// Poll device for completed work
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_sync_BassaltFence_pollDevice(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    wait: jboolean,
) -> jboolean {
    if device_ptr == 0 {
        return 1; // Return true if no device
    }
    let device = unsafe { &*(device_ptr as *const BasaltDevice) };
    
    let poll_type = if wait != 0 {
        wgt::PollType::wait_indefinitely()
    } else {
        wgt::PollType::Poll
    };
    
    match device.context().inner().device_poll(device.id(), poll_type) {
        Ok(status) => {
            if status.is_queue_empty() { 1 } else { 0 }
        }
        Err(e) => {
            log::warn!("Device poll error: {:?}", e);
            1 // Treat as complete on error
        }
    }
}

/// Check if work up to submission index is complete
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_sync_BassaltFence_isWorkComplete(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    _submission_index: jlong,
) -> jboolean {
    if device_ptr == 0 {
        return 1; // Treat as complete if no device
    }
    let device = unsafe { &*(device_ptr as *const BasaltDevice) };
    
    // Poll and check if queue is empty
    match device.context().inner().device_poll(device.id(), wgt::PollType::Poll) {
        Ok(status) => {
            if status.is_queue_empty() { 1 } else { 0 }
        }
        Err(_) => 1 // Treat as complete on error
    }
}

// ============================================================================
// TIMER QUERIES
// ============================================================================

/// Check if timestamp queries are supported
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_sync_BassaltQuery_isTimestampQuerySupported(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
) -> jboolean {
    if device_ptr == 0 {
        return 0;
    }
    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Query device features from wgpu-core
    let features = device.get_context().inner().device_features(device.get_device_id());

    // Check if TIMESTAMP_QUERY feature is enabled
    if features.contains(wgt::Features::TIMESTAMP_QUERY) {
        log::debug!("Timestamp queries are supported");
        1
    } else {
        log::warn!("Timestamp queries are NOT supported on this device");
        0
    }
}

/// Create a timestamp query set
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_sync_BassaltQuery_createTimestampQuery(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    num_queries: jint,
) -> jlong {
    if device_ptr == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null device pointer");
        return 0;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Check feature support first
    let features = device.get_context().inner().device_features(device.get_device_id());
    if !features.contains(wgt::Features::TIMESTAMP_QUERY) {
        let _ = env.throw_new("java/lang/UnsupportedOperationException", "Timestamp queries not supported on this device");
        return 0;
    }

    match timestamp_queries::TimestampQuerySet::new(
        device.context(),
        device.id(),
        num_queries.max(1) as u64,
    ) {
        Ok(queries) => {
            log::debug!("Created timestamp query set with {} queries", num_queries);
            // Box the query set and return as pointer
            Box::into_raw(Box::new(queries)) as jlong
        }
        Err(e) => {
            let msg = format!("Failed to create timestamp query set: {}", e);
            log::error!("{}", msg);
            let _ = env.throw_new("java/lang/RuntimeException", &msg);
            0
        }
    }
}

/// Destroy a timestamp query set
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_sync_BassaltQuery_destroyTimestampQuery(
    _env: JNIEnv,
    _class: JClass,
    _device_ptr: jlong,
    query_ptr: jlong,
) {
    if query_ptr == 0 {
        return;
    }

    // Take ownership of the boxed TimestampQuerySet and drop it
    let _queries = unsafe { Box::from_raw(query_ptr as *mut timestamp_queries::TimestampQuerySet) };
    log::debug!("Destroyed timestamp query set");
}

/// Write a timestamp at the specified query index
///
/// # Arguments
/// - `device_ptr` - Pointer to BasaltDevice
/// - `query_ptr` - Pointer to TimestampQuerySet
/// - `encoder_ptr` - Pointer to CommandEncoder (optional, for immediate writes)
/// - `query_index` - The index in the query set to write to
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_sync_BassaltQuery_writeTimestamp(
    mut env: JNIEnv,
    _class: JClass,
    _device_ptr: jlong,
    query_ptr: jlong,
    _encoder_ptr: jlong,
    query_index: jint,
) {
    if query_ptr == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null query pointer");
        return;
    }

    let queries = unsafe { &mut *(query_ptr as *mut timestamp_queries::TimestampQuerySet) };

    if let Err(e) = queries.write_timestamp(query_index as u32) {
        let msg = format!("Failed to write timestamp: {}", e);
        log::error!("{}", msg);
        let _ = env.throw_new("java/lang/RuntimeException", &msg);
    }
}

/// Resolve timestamps to buffer and read them
///
/// # Arguments
/// - `device_ptr` - Pointer to BasaltDevice
/// - `query_ptr` - Pointer to TimestampQuerySet
/// - `start_query` - First query index to resolve
/// - `query_count` - Number of queries to resolve
///
/// # Returns
/// Array of timestamp values (in nanoseconds, scaled by timestamp period)
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_sync_BassaltQuery_getTimestampValue(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    query_ptr: jlong,
    start_query: jint,
    query_count: jint,
) -> jlongArray {
    if device_ptr == 0 || query_ptr == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null pointer");
        return std::ptr::null_mut();
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };
    let queries = unsafe { &*(query_ptr as *const timestamp_queries::TimestampQuerySet) };

    let start = start_query.max(0) as u32;
    let end = (start + query_count.max(0) as u32).min(queries.num_queries as u32);

    if start >= end {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Invalid query range");
        return std::ptr::null_mut();
    }

    // Read timestamps from the buffer
    match queries.read(device.context(), device.id(), start..end) {
        Ok(raw_timestamps) => {
            // Get the timestamp period to convert to nanoseconds
            let period = match timestamp_queries::TimestampQuerySet::get_timestamp_period(
                device.context(),
                device.queue_id(),
            ) {
                Ok(p) => p,
                Err(e) => {
                    log::warn!("Failed to get timestamp period: {}, using 1.0", e);
                    1.0
                }
            };

            // Convert raw timestamps to nanoseconds and create Java array
            let timestamps_ns: Vec<jlong> = raw_timestamps.iter()
                .map(|&ts| (ts as f64 * period as f64) as jlong)
                .collect();

            let result = match env.new_long_array(timestamps_ns.len() as i32) {
                Ok(arr) => arr,
                Err(_) => {
                    let _ = env.throw_new("java/lang/RuntimeException", "Failed to create long array");
                    return std::ptr::null_mut();
                }
            };

            match env.set_long_array_region(&result, 0, &timestamps_ns) {
                Ok(_) => result.into_raw(),
                Err(_) => {
                    let _ = env.throw_new("java/lang/RuntimeException", "Failed to populate long array");
                    std::ptr::null_mut()
                }
            }
        }
        Err(e) => {
            let msg = format!("Failed to read timestamps: {}", e);
            log::error!("{}", msg);
            let _ = env.throw_new("java/lang/RuntimeException", &msg);
            std::ptr::null_mut()
        }
    }
}

/// Get the number of skipped undersized buffers (for statistics)
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_sync_BassaltQuery_getSkippedBufferCount(
    _env: JNIEnv,
    _class: JClass,
) -> jlong {
    timestamp_queries::get_skipped_buffer_count() as jlong
}

// ============================================================================
// MSAA (MULTISAMPLE ANTI-ALIASING) SUPPORT
// ============================================================================

/// Get the maximum supported MSAA sample count for a texture format
///
/// # Returns
/// - 1 = No MSAA
/// - 2 = 2x MSAA
/// - 4 = 4x MSAA
/// - 8 = 8x MSAA
/// - 16 = 16x MSAA
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_getMaxSupportedSamples(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    format_int: jint,
) -> jint {
    if device_ptr == 0 {
        return 1; // Fallback to no MSAA
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Convert Java format constant to wgpu format
    let format = match format_int {
        1 => wgt::TextureFormat::Rgba8UnormSrgb,
        2 => wgt::TextureFormat::Bgra8UnormSrgb,
        3 => wgt::TextureFormat::Rgba8Unorm,
        4 => wgt::TextureFormat::Bgra8Unorm,
        _ => wgt::TextureFormat::Bgra8UnormSrgb,
    };

    // Get adapter ID from device
    let adapter_id = device.adapter_id();

    match msaa::MSAAConfig::get_max_supported_samples(device.context(), adapter_id, format) {
        Ok(samples) => samples as jint,
        Err(e) => {
            log::warn!("Failed to get max MSAA samples: {}", e);
            1 // Fallback to no MSAA
        }
    }
}

/// Create an MSAA framebuffer configuration
///
/// # Arguments
/// - `device_ptr` - Pointer to BasaltDevice
/// - `width` - Width in pixels
/// - `height` - Height in pixels
/// - `format` - Texture format (as Java constant)
/// - `sample_count` - Desired sample count (will be clamped to max supported)
///
/// # Returns
/// A pointer to MSAAConfig (boxed), or 0 on failure
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_createMSAAConfig(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    width: jint,
    height: jint,
    format_int: jint,
    sample_count: jint,
) -> jlong {
    if device_ptr == 0 {
        return 0;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Convert Java format constant to wgpu format
    let format = match format_int {
        1 => wgt::TextureFormat::Rgba8UnormSrgb,
        2 => wgt::TextureFormat::Bgra8UnormSrgb,
        3 => wgt::TextureFormat::Rgba8Unorm,
        4 => wgt::TextureFormat::Bgra8Unorm,
        _ => wgt::TextureFormat::Bgra8UnormSrgb,
    };

    match msaa::MSAAConfig::new(
        device.context(),
        device.id(),
        width.max(1) as u32,
        height.max(1) as u32,
        format,
        sample_count.max(1) as u32,
    ) {
        Ok(msaa_config) => {
            log::debug!("Created MSAA config: {}x{} with {} samples", width, height, msaa_config.sample_count);
            // Box the config and return the pointer
            Box::into_raw(Box::new(msaa_config)) as jlong
        }
        Err(e) => {
            log::error!("Failed to create MSAA config: {}", e);
            0
        }
    }
}

/// Destroy an MSAA configuration
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_destroyMSAAConfig(
    _env: JNIEnv,
    _class: JClass,
    _device_ptr: jlong,
    msaa_ptr: jlong,
) {
    if msaa_ptr == 0 {
        return;
    }

    // Take ownership of the boxed MSAAConfig and drop it
    let _msaa_config = unsafe { Box::from_raw(msaa_ptr as *mut msaa::MSAAConfig) };

    log::debug!("Destroyed MSAA config");
}

/// Get the sample count from an MSAA configuration
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_getMSAASampleCount(
    _env: JNIEnv,
    _class: JClass,
    _device_ptr: jlong,
    msaa_ptr: jlong,
) -> jint {
    if msaa_ptr == 0 {
        return 1;
    }

    let msaa_config = unsafe { &*(msaa_ptr as *const msaa::MSAAConfig) };
    msaa_config.sample_count as jint
}

// ============================================================================
// RENDER BUNDLE SUPPORT
// ============================================================================

/// Create a render bundle encoder
///
/// # Arguments
/// - `device_ptr` - Pointer to BasaltDevice
/// - `color_format` - Color attachment format (as Java constant)
/// - `sample_count` - MSAA sample count
///
/// # Returns
/// A pointer to RenderBundleEncoder (boxed), or 0 on failure
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_createRenderBundleEncoder(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    color_format_int: jint,
    sample_count: jint,
) -> jlong {
    if device_ptr == 0 {
        return 0;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Convert Java format constant to wgpu format
    let color_format = match color_format_int {
        1 => wgt::TextureFormat::Rgba8UnormSrgb,
        2 => wgt::TextureFormat::Bgra8UnormSrgb,
        3 => wgt::TextureFormat::Rgba8Unorm,
        4 => wgt::TextureFormat::Bgra8Unorm,
        _ => wgt::TextureFormat::Bgra8UnormSrgb,
    };

    match render_bundle::create_simple_encoder(
        device.context(),
        device.id(),
        color_format,
        sample_count.max(1) as u32,
    ) {
        Ok(encoder) => {
            log::debug!("Created render bundle encoder");
            // Box the encoder and return the pointer
            Box::into_raw(Box::new(encoder)) as jlong
        }
        Err(e) => {
            log::error!("Failed to create render bundle encoder: {}", e);
            0
        }
    }
}

/// Destroy a render bundle encoder
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_destroyRenderBundleEncoder(
    _env: JNIEnv,
    _class: JClass,
    _device_ptr: jlong,
    encoder_ptr: jlong,
) {
    if encoder_ptr == 0 {
        return;
    }

    // Take ownership of the boxed encoder and drop it
    let _encoder = unsafe { Box::from_raw(encoder_ptr as *mut wgpu_core::command::RenderBundleEncoder) };

    log::debug!("Destroyed render bundle encoder");
}

// ============================================================================
// SHADER COMPILATION INFO SUPPORT
// ============================================================================

use crate::error::CompilationInfo;

/// Get shader compilation info for WGSL source code
///
/// # Arguments
/// - `wgsl_source` - WGSL shader source code as Java string
///
/// # Returns
/// A pointer to boxed CompilationInfo, or 0 if the shader compiles successfully
/// (with no errors/warnings) or on allocation failure.
///
/// # Safety
/// The returned pointer must be freed with `destroyCompilationInfo`
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltBackend_getWgslCompilationInfo(
    mut env: JNIEnv,
    _class: JClass,
    wgsl_source: JString,
) -> jlong {
    let source: String = match env.get_string(&wgsl_source) {
        Ok(s) => s.into(),
        Err(e) => {
            log::error!("Failed to get WGSL source string: {}", e);
            return 0;
        }
    };

    let info = shader::get_wgsl_compilation_info(&source);

    // Only return non-null if there are actual messages
    if info.messages.is_empty() {
        return 0;
    }

    Box::into_raw(Box::new(info)) as jlong
}

/// Get the number of messages in compilation info
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltBackend_getCompilationInfoMessageCount(
    _env: JNIEnv,
    _class: JClass,
    info_ptr: jlong,
) -> jint {
    if info_ptr == 0 {
        return 0;
    }

    let info = unsafe { &*(info_ptr as *const CompilationInfo) };
    info.messages.len() as jint
}

/// Get the message text at a given index
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltBackend_getCompilationInfoMessage(
    env: JNIEnv,
    _class: JClass,
    info_ptr: jlong,
    index: jint,
) -> jstring {
    if info_ptr == 0 || index < 0 {
        return std::ptr::null_mut();
    }

    let info = unsafe { &*(info_ptr as *const CompilationInfo) };
    let idx = index as usize;

    if idx >= info.messages.len() {
        return std::ptr::null_mut();
    }

    match env.new_string(&info.messages[idx].message) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Get the message type at a given index
/// Returns: 0 = Error, 1 = Warning, 2 = Info
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltBackend_getCompilationInfoMessageType(
    _env: JNIEnv,
    _class: JClass,
    info_ptr: jlong,
    index: jint,
) -> jint {
    if info_ptr == 0 || index < 0 {
        return 0;
    }

    let info = unsafe { &*(info_ptr as *const CompilationInfo) };
    let idx = index as usize;

    if idx >= info.messages.len() {
        return 0;
    }

    info.messages[idx].message_type.to_i32()
}

/// Get the line number at a given index (1-based)
/// Returns 0 if no location information is available
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltBackend_getCompilationInfoLineNumber(
    _env: JNIEnv,
    _class: JClass,
    info_ptr: jlong,
    index: jint,
) -> jint {
    if info_ptr == 0 || index < 0 {
        return 0;
    }

    let info = unsafe { &*(info_ptr as *const CompilationInfo) };
    let idx = index as usize;

    if idx >= info.messages.len() {
        return 0;
    }

    info.messages[idx]
        .location
        .as_ref()
        .map(|loc| loc.line_number as jint)
        .unwrap_or(0)
}

/// Get the line position (column) at a given index (1-based, in bytes)
/// Returns 0 if no location information is available
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltBackend_getCompilationInfoLinePosition(
    _env: JNIEnv,
    _class: JClass,
    info_ptr: jlong,
    index: jint,
) -> jint {
    if info_ptr == 0 || index < 0 {
        return 0;
    }

    let info = unsafe { &*(info_ptr as *const CompilationInfo) };
    let idx = index as usize;

    if idx >= info.messages.len() {
        return 0;
    }

    info.messages[idx]
        .location
        .as_ref()
        .map(|loc| loc.line_position as jint)
        .unwrap_or(0)
}

/// Get the byte offset at a given index (0-based)
/// Returns 0 if no location information is available
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltBackend_getCompilationInfoOffset(
    _env: JNIEnv,
    _class: JClass,
    info_ptr: jlong,
    index: jint,
) -> jint {
    if info_ptr == 0 || index < 0 {
        return 0;
    }

    let info = unsafe { &*(info_ptr as *const CompilationInfo) };
    let idx = index as usize;

    if idx >= info.messages.len() {
        return 0;
    }

    info.messages[idx]
        .location
        .as_ref()
        .map(|loc| loc.offset as jint)
        .unwrap_or(0)
}

/// Get the length in bytes at a given index
/// Returns 0 if no location information is available
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltBackend_getCompilationInfoLength(
    _env: JNIEnv,
    _class: JClass,
    info_ptr: jlong,
    index: jint,
) -> jint {
    if info_ptr == 0 || index < 0 {
        return 0;
    }

    let info = unsafe { &*(info_ptr as *const CompilationInfo) };
    let idx = index as usize;

    if idx >= info.messages.len() {
        return 0;
    }

    info.messages[idx]
        .location
        .as_ref()
        .map(|loc| loc.length as jint)
        .unwrap_or(0)
}

/// Get the compilation info as a formatted string
///
/// This returns a human-readable string with all messages including
/// line/column information where available.
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltBackend_getCompilationInfoString(
    env: JNIEnv,
    _class: JClass,
    info_ptr: jlong,
) -> jstring {
    if info_ptr == 0 {
        return match env.new_string("No compilation info") {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        };
    }

    let info = unsafe { &*(info_ptr as *const CompilationInfo) };

    match env.new_string(info.to_string()) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Check if the compilation info has any errors
///
/// Returns: 1 if there are errors, 0 otherwise
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltBackend_compilationInfoHasErrors(
    _env: JNIEnv,
    _class: JClass,
    info_ptr: jlong,
) -> jboolean {
    if info_ptr == 0 {
        return 0;
    }

    let info = unsafe { &*(info_ptr as *const CompilationInfo) };
    if info.has_errors() { 1 } else { 0 }
}

/// Destroy compilation info and free memory
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltBackend_destroyCompilationInfo(
    _env: JNIEnv,
    _class: JClass,
    info_ptr: jlong,
) {
    if info_ptr == 0 {
        return;
    }

    // Take ownership of the boxed CompilationInfo and drop it
    let _info = unsafe { Box::from_raw(info_ptr as *mut CompilationInfo) };

    log::debug!("Destroyed compilation info");
}
