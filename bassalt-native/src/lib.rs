#![allow(clippy::missing_safety_doc)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

mod jni;
mod context;
mod device;
mod adapter;
mod surface;
mod buffer;
mod texture;
mod sampler;
mod pipeline;
mod shader;
mod command;
mod error;
mod resource_handles;
mod render_pass;
mod bind_group;

use std::borrow::Cow;
use std::sync::Arc;
use ::jni::JNIEnv;
use ::jni::objects::{JByteArray, JClass, JString, JObject};
use ::jni::sys::{jlong, jint, jboolean, jstring, jfloat, jvalue};
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
    jni::init_logging();

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
            log::debug!("Created buffer with handle {} (size={})", handle, size);
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
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_writeBuffer(
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

    match device.create_texture(
        width as u32,
        height as u32,
        depth as u32,
        mip_levels as u32,
        format as u32,
        usage as u32,
    ) {
        Ok(texture_id) => {
            // Store texture with array layer info for view dimension detection
            let handle = HANDLES.insert_texture(
                texture_id,
                depth as u32,
                wgt::TextureDimension::D2, // All our textures are 2D for now
            );
            log::debug!("Created texture with handle {} ({}x{}x{})", handle, width, height, depth);
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
            let handle = HANDLES.insert_texture_view(view_id, dimension);
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
        // 1 = POSITION_COLOR (3 floats + 4 floats)
        1 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
            array_stride: 28, // 12 + 16 = 28 bytes
            step_mode: wgt::VertexStepMode::Vertex,
            attributes: Cow::Owned(vec![
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x4,
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
        // 3 = POSITION_TEX_COLOR (3 floats + 2 floats + 4 floats)
        3 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
            array_stride: 36, // 12 + 8 + 16 = 36 bytes
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
        }]),
        // 4 = POSITION_TEX_COLOR_NORMAL (3 floats + 2 floats + 4 floats + 3 floats)
        4 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
            array_stride: 48, // 12 + 8 + 16 + 12 = 48 bytes
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
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x3,
                    offset: 36,
                    shader_location: 3,
                },
            ]),
        }]),
        // 5 = POSITION_COLOR_TEX (3 floats + 4 floats + 2 floats)
        5 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
            array_stride: 36, // 12 + 16 + 8 = 36 bytes
            step_mode: wgt::VertexStepMode::Vertex,
            attributes: Cow::Owned(vec![
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0, // position
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x4,
                    offset: 12,
                    shader_location: 1, // color
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x2,
                    offset: 28,
                    shader_location: 2, // uv
                },
            ]),
        }]),
        // 6 = POSITION_COLOR_TEX_TEX_TEX_NORMAL (position, color, uv0, uv1, uv2, normal)
        6 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
            array_stride: 64, // 12 + 16 + 8 + 8 + 8 + 12 = 64 bytes
            step_mode: wgt::VertexStepMode::Vertex,
            attributes: Cow::Owned(vec![
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0, // position
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x4,
                    offset: 12,
                    shader_location: 1, // color
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x2,
                    offset: 28,
                    shader_location: 2, // uv0
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x2,
                    offset: 36,
                    shader_location: 3, // uv1
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x2,
                    offset: 44,
                    shader_location: 4, // uv2
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x3,
                    offset: 52,
                    shader_location: 5, // normal
                },
            ]),
        }]),
        // 7 = POSITION_COLOR_TEX_TEX_NORMAL (position, color, uv0, uv2, normal - skips uv1)
        7 => Cow::Owned(vec![wgpu_core::pipeline::VertexBufferLayout {
            array_stride: 56, // 12 + 16 + 8 + 8 + 12 = 56 bytes
            step_mode: wgt::VertexStepMode::Vertex,
            attributes: Cow::Owned(vec![
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0, // position
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x4,
                    offset: 12,
                    shader_location: 1, // color
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x2,
                    offset: 28,
                    shader_location: 2, // uv0
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x2,
                    offset: 36,
                    shader_location: 3, // uv2
                },
                wgt::VertexAttribute {
                    format: wgt::VertexFormat::Float32x3,
                    offset: 44,
                    shader_location: 4, // normal
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

/// Helper function to create a bind group layout from shader reflection
fn create_layout_from_shaders(
    context: &Arc<BasaltContext>,
    device_id: wgpu_core::id::DeviceId,
    vertex_module: &naga::Module,
    fragment_module: &naga::Module,
) -> Result<(wgpu_core::id::BindGroupLayoutId, wgpu_core::id::PipelineLayoutId), BasaltError> {
    use std::collections::BTreeMap;
    use std::borrow::Cow;
    use wgpu_core::binding_model;

    // Collect all bindings from both shaders
    let mut bindings: BTreeMap<u32, wgt::BindGroupLayoutEntry> = BTreeMap::new();

    // Helper to extract bindings from a module
    let mut extract_bindings = |module: &naga::Module, stage: wgt::ShaderStages| {
        for (_handle, global_var) in module.global_variables.iter() {
            if let Some(binding) = &global_var.binding {
                // Only process group 0 bindings (Minecraft uses group 0)
                if binding.group == 0 {
                    let ty = &module.types[global_var.ty];

                    let binding_type = match global_var.space {
                        naga::AddressSpace::Uniform => {
                            wgt::BindingType::Buffer {
                                ty: wgt::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            }
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
                                    log::debug!("Found texture at binding {}: dimension {:?}", binding.binding, view_dimension);
                                    wgt::BindingType::Texture {
                                        sample_type: wgt::TextureSampleType::Float { filterable: true },
                                        view_dimension,
                                        multisampled: false,
                                    }
                                }
                                naga::TypeInner::Sampler { .. } => {
                                    wgt::BindingType::Sampler(wgt::SamplerBindingType::Filtering)
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
                        .and_modify(|e| e.visibility |= visibility)
                        .or_insert(wgt::BindGroupLayoutEntry {
                            binding: binding.binding,
                            visibility,
                            ty: binding_type,
                            count: None,
                        });
                }
            }
        }
    };

    // Extract bindings from both shaders
    extract_bindings(vertex_module, wgt::ShaderStages::VERTEX);
    extract_bindings(fragment_module, wgt::ShaderStages::FRAGMENT);

    // Create bind group layout entries vector (sorted by binding number)
    let layout_entries: Vec<wgt::BindGroupLayoutEntry> = bindings.into_values().collect();

    log::debug!("Creating pipeline layout with {} bindings", layout_entries.len());

    // Create bind group layout
    let bgl_desc = binding_model::BindGroupLayoutDescriptor {
        label: Some(Cow::Borrowed("Pipeline Bind Group Layout")),
        entries: Cow::Owned(layout_entries),
    };

    let global = context.inner();
    let (bgl_id, bgl_error) = global.device_create_bind_group_layout(device_id, &bgl_desc, None);

    if let Some(e) = bgl_error {
        return Err(BasaltError::Device(format!(
            "Failed to create bind group layout: {:?}",
            e
        )));
    }

    // Create pipeline layout
    let pl_desc = binding_model::PipelineLayoutDescriptor {
        label: Some(Cow::Borrowed("Pipeline Layout")),
        bind_group_layouts: Cow::Owned(vec![bgl_id]),
        push_constant_ranges: Cow::Borrowed(&[]),
    };

    let (pl_id, pl_error) = global.device_create_pipeline_layout(device_id, &pl_desc, None);

    if let Some(e) = pl_error {
        return Err(BasaltError::Device(format!(
            "Failed to create pipeline layout: {:?}",
            e
        )));
    }

    Ok((bgl_id, pl_id))
}

/// Create a render pipeline from pre-converted WGSL shaders
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_createNativePipelineFromWgsl(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    vertex_shader: JString,
    fragment_shader: JString,
    _vertex_format: jint,
    primitive_topology: jint,
    depth_test_enabled: jboolean,
    depth_write_enabled: jboolean,
    depth_compare: jint,
    blend_enabled: jboolean,
    blend_color_factor: jint,
    blend_alpha_factor: jint,
) -> jlong {
    use std::borrow::Cow;
    use wgpu_core::pipeline;
    use naga::front;

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

    // Parse WGSL shaders
    let vertex_module = match front::wgsl::parse_str(&vertex_wgsl) {
        Ok(module) => module,
        Err(e) => {
            let msg = format!("Failed to parse vertex WGSL: {:?}", e);
            log::error!("{}", msg);
            let _ = env.throw_new("java/lang/RuntimeException", &msg);
            return 0;
        }
    };

    let fragment_module = match front::wgsl::parse_str(&fragment_wgsl) {
        Ok(module) => module,
        Err(e) => {
            let msg = format!("Failed to parse fragment WGSL: {:?}", e);
            log::error!("{}", msg);
            let _ = env.throw_new("java/lang/RuntimeException", &msg);
            return 0;
        }
    };

    // Create pipeline layout from shader reflection
    let (bind_group_layout_id, pipeline_layout_id) = match create_layout_from_shaders(
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

    // Create shader modules
    let vs_desc = pipeline::ShaderModuleDescriptor {
        label: Some(Cow::Borrowed("Vertex Shader")),
        runtime_checks: wgt::ShaderRuntimeChecks::default(),
    };
    let vs_source = pipeline::ShaderModuleSource::Naga(Cow::Owned(vertex_module));

    let (vertex_shader_id, vs_error) = device_context.inner()
        .device_create_shader_module(device_id, &vs_desc, vs_source, None);

    if let Some(e) = vs_error {
        let msg = format!("Failed to create vertex shader module: {:?}", e);
        log::error!("{}", msg);
        let _ = env.throw_new("java/lang/RuntimeException", &msg);
        return 0;
    }

    let fs_desc = pipeline::ShaderModuleDescriptor {
        label: Some(Cow::Borrowed("Fragment Shader")),
        runtime_checks: wgt::ShaderRuntimeChecks::default(),
    };
    let fs_source = pipeline::ShaderModuleSource::Naga(Cow::Owned(fragment_module));

    let (fragment_shader_id, fs_error) = device_context.inner()
        .device_create_shader_module(device_id, &fs_desc, fs_source, None);

    if let Some(e) = fs_error {
        let msg = format!("Failed to create fragment shader module: {:?}", e);
        log::error!("{}", msg);
        let _ = env.throw_new("java/lang/RuntimeException", &msg);
        return 0;
    }

    // Map pipeline parameters (same as createRenderPipeline)
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

    let blend_state = if blend_enabled != 0 {
        let color_factor = match blend_color_factor as u32 {
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
            _ => wgt::BlendFactor::One,
        };
        let alpha_factor = match blend_alpha_factor as u32 {
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
            _ => wgt::BlendFactor::One,
        };

        Some(wgt::BlendState {
            color: wgt::BlendComponent {
                src_factor: color_factor,
                dst_factor: wgt::BlendFactor::OneMinusSrc,
                operation: wgt::BlendOperation::Add,
            },
            alpha: wgt::BlendComponent {
                src_factor: alpha_factor,
                dst_factor: wgt::BlendFactor::OneMinusSrc,
                operation: wgt::BlendOperation::Add,
            },
        })
    } else {
        None
    };

    // Use the pipeline layout created from shader reflection
    // (pipeline_layout_id is already set above from create_layout_from_shaders)

    // Create render pipeline descriptor with the reflected layout
    let pipeline_desc = pipeline::RenderPipelineDescriptor {
        label: Some(Cow::Borrowed("Basalt Render Pipeline")),
        layout: Some(pipeline_layout_id),
        vertex: pipeline::VertexState {
            stage: pipeline::ProgrammableStageDescriptor {
                module: vertex_shader_id,
                entry_point: Some(Cow::Borrowed("main_vs")),
                constants: Default::default(),
                zero_initialize_workgroup_memory: true,
            },
            // Create vertex buffer layout based on vertex format
            // 0 = POSITION (3 floats)
            // 1 = POSITION_COLOR (3 floats + 4 floats)
            // 2 = POSITION_TEX (3 floats + 2 floats)
            // 3 = POSITION_TEX_COLOR (3 floats + 2 floats + 4 floats)
            // 4 = POSITION_TEX_COLOR_NORMAL (3 floats + 2 floats + 4 floats + 3 floats)
            buffers: create_vertex_buffer_layout(_vertex_format as usize),
        },
        primitive: wgt::PrimitiveState {
            topology: primitive_topology,
            strip_index_format: None,
            front_face: wgt::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgt::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: Some(wgt::DepthStencilState {
            format: wgt::TextureFormat::Depth32Float,
            depth_write_enabled: depth_write_enabled != 0,
            depth_compare: if depth_test_enabled != 0 { depth_compare } else { wgt::CompareFunction::Always },
            stencil: wgt::StencilState::default(),
            bias: wgt::DepthBiasState::default(),
        }),
        multisample: wgt::MultisampleState::default(),
        fragment: Some(pipeline::FragmentState {
            stage: pipeline::ProgrammableStageDescriptor {
                module: fragment_shader_id,
                entry_point: Some(Cow::Borrowed("main_fs")),
                constants: Default::default(),
                zero_initialize_workgroup_memory: true,
            },
            targets: Cow::Owned(vec![Some(wgt::ColorTargetState {
                format: wgt::TextureFormat::Rgba8UnormSrgb,
                blend: blend_state,
                write_mask: wgt::ColorWrites::ALL,
            })]),
        }),
        multiview: None,
        cache: None,
    };

    // Create the render pipeline
    let (pipeline_id, pipeline_error) = device_context.inner()
        .device_create_render_pipeline(device_id, &pipeline_desc, None);

    if let Some(e) = pipeline_error {
        let msg = format!("Failed to create render pipeline: {:?}", e);
        log::error!("{}", msg);
        let _ = env.throw_new("java/lang/RuntimeException", &msg);
        return 0;
    }

    let handle = HANDLES.insert_render_pipeline(pipeline_id);
    log::info!("Created render pipeline from WGSL with handle {}", handle);
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
    clear_color: jint,
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
        HANDLES.get_texture_view(color_view_handle as u64)
    } else {
        None
    };

    let depth_view = if depth_view_handle != 0 {
        HANDLES.get_texture_view(depth_view_handle as u64)
    } else {
        None
    };

    // Create render pass state
    match render_pass::RenderPassState::new(
        device.context().clone(),
        device.id(),
        device.queue_id(),
        color_view,
        depth_view,
        clear_color as u32,
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

    if let Some(pipeline_id) = HANDLES.get_render_pipeline(pipeline_handle as u64) {
        state.record_set_pipeline(pipeline_id);
        log::debug!("Recorded setPipeline (pipeline={})", pipeline_handle);
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
    if render_pass_ptr == 0 {
        return;
    }

    let state = unsafe { &mut *(render_pass_ptr as *mut render_pass::RenderPassState) };

    if let Some(buffer_id) = HANDLES.get_buffer(buffer_handle as u64) {
        state.record_set_vertex_buffer(slot as u32, buffer_id, offset as u64, None);
        log::debug!("Recorded setVertexBuffer (slot={}, buffer={}, offset={})",
            slot, buffer_handle, offset);
    } else {
        log::error!("Invalid buffer handle: {}", buffer_handle);
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
    if render_pass_ptr == 0 {
        return;
    }

    let state = unsafe { &mut *(render_pass_ptr as *mut render_pass::RenderPassState) };

    let index_format = match index_type {
        0 => wgt::IndexFormat::Uint16,
        1 => wgt::IndexFormat::Uint32,
        _ => {
            log::error!("Invalid index type: {}", index_type);
            return;
        }
    };

    if let Some(buffer_id) = HANDLES.get_buffer(buffer_handle as u64) {
        state.record_set_index_buffer(buffer_id, index_format, offset as u64, None);
        log::debug!("Recorded setIndexBuffer (buffer={}, type={}, offset={})",
            buffer_handle, index_type, offset);
    } else {
        log::error!("Invalid buffer handle: {}", buffer_handle);
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
    if render_pass_ptr == 0 {
        return;
    }

    let state = unsafe { &mut *(render_pass_ptr as *mut render_pass::RenderPassState) };

    state.record_draw_indexed(
        index_count as u32,
        instance_count as u32,
        first_index as u32,
        base_vertex,
        first_instance as u32,
    );

    log::debug!("Recorded drawIndexed (indices={}, instances={}, first={}, base={}, firstInst={})",
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
    
    // Finish and submit
    if let Err(e) = state.finish_and_submit(device.context().as_ref(), device.queue_id()) {
        log::error!("Failed to end render pass: {}", e);
    } else {
        log::debug!("Ended render pass at {:?}", render_pass_ptr as *const ());
    }
    
    // State is dropped here
}

// ============================================================================
// BIND GROUP OPERATIONS
// ============================================================================

/// Create a bind group from arrays of texture, sampler, and uniform bindings
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_pipeline_BassaltRenderPass_createBindGroup0(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    _render_pass_ptr: jlong,
    texture_names: JObject,
    texture_handles: JObject,
    sampler_handles: JObject,
    uniform_names: JObject,
    uniform_handles: JObject,
) -> jlong {
    if device_ptr == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null device pointer");
        return 0;
    }

    let device = unsafe { &*(device_ptr as *const device::BasaltDevice) };
    let context = device.context().clone();
    let device_id = device.id();

    // Create bind group builder (it will create its own layout based on bindings)
    let mut builder = bind_group::BindGroupBuilder::new(context, device_id);
    let mut binding_slot = 0u32;

    // Add texture bindings
    if !texture_handles.is_null() {
        let tex_array: ::jni::objects::JPrimitiveArray<i64> = texture_handles.into();
        let samp_array: ::jni::objects::JPrimitiveArray<i64> = sampler_handles.into();

        let texture_count = match env.get_array_length(&tex_array) {
            Ok(len) => len as usize,
            Err(_) => 0,
        };

        for i in 0..texture_count {
            // Get texture handle
            let mut tex_handle_buf = [0i64; 1];
            if env.get_long_array_region(&tex_array, i as i32, &mut tex_handle_buf).is_ok() {
                let tex_handle = tex_handle_buf[0];
                if tex_handle != 0 {
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

                        builder = builder.add_texture(binding_slot, view_info.id, sampler_id, view_info.dimension);
                        binding_slot += if sampler_id.is_some() { 2 } else { 1 };
                    }
                }
            }
        }
    }

    // Add uniform buffer bindings
    if !uniform_handles.is_null() {
        let unif_array: ::jni::objects::JPrimitiveArray<i64> = uniform_handles.into();

        let uniform_count = match env.get_array_length(&unif_array) {
            Ok(len) => len as usize,
            Err(_) => 0,
        };

        for i in 0..uniform_count {
            let mut unif_handle_buf = [0i64; 1];
            if env.get_long_array_region(&unif_array, i as i32, &mut unif_handle_buf).is_ok() {
                let unif_handle = unif_handle_buf[0];
                if unif_handle != 0 {
                    if let Some(buffer_info) = HANDLES.get_buffer_info(unif_handle as u64) {
                        // Bind entire buffer (offset=0, size=whole buffer)
                        // TODO: Support buffer slices with offset/size from UniformBinding
                        builder = builder.add_uniform_buffer(binding_slot, buffer_info.id, 0, buffer_info.size);
                        binding_slot += 1;
                    }
                }
            }
        }
    }

    // Build the bind group
    match builder.build() {
        Ok(bind_group_id) => {
            let handle = HANDLES.insert_bind_group(bind_group_id);
            log::debug!("Created bind group with {} bindings (handle={})", binding_slot, handle);
            handle as jlong
        }
        Err(e) => {
            log::error!("Failed to create bind group: {:?}", e);
            let _ = env.throw_new("java/lang/RuntimeException", format!("Failed to create bind group: {:?}", e));
            0
        }
    }
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
        // Record the set bind group command
        state.record_set_bind_group(index as u32, Some(bind_group_id), Vec::new());
        log::debug!("Recorded setBindGroup (index={}, bind_group={})", index, bind_group_handle);
    } else {
        log::warn!("setBindGroup: invalid bind group handle {}", bind_group_handle);
        log::debug!("Bind group set (placeholder implementation)");
    }
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

    // Convert clear color from packed RGBA to Color struct
    let r = ((clear_color >> 24) & 0xFF) as f64 / 255.0;
    let g = ((clear_color >> 16) & 0xFF) as f64 / 255.0;
    let b = ((clear_color >> 8) & 0xFF) as f64 / 255.0;
    let a = (clear_color & 0xFF) as f64 / 255.0;
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
        width as u32,
        height as u32,
    ) {
        let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to write texture: {}", e));
    } else {
        log::debug!("Wrote {}x{} to texture at ({}, {})", width, height, dest_x, dest_y);
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
