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

use std::sync::Arc;
use jni::JNIEnv;
use jni::objects::{JClass, JObject, JString, JObjectArray};
use jni::sys::{jlong, jint, jboolean, jstring, jbyteArray};
use once_cell::sync::OnceCell;
use log::info;

use crate::context::BasaltContext;
use crate::device::BasaltDevice;
use crate::error::{BasaltError, Result};

/// Global context singleton
static GLOBAL_CONTEXT: OnceCell<Arc<BasaltContext>> = OnceCell::new();

/// Initialize the Basalt renderer
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltBackend_init(
    mut env: JNIEnv,
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
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltBackend_createDevice(
    mut env: JNIEnv,
    _class: JClass,
    context_ptr: jlong,
    window_ptr: jlong,
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

    match device::create_device_from_window(context_clone, window_ptr, width as u32, height as u32) {
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
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltBackend_getAdapterInfo(
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

    match env.new_string(info) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Release a device
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_release(
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
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_presentFrame(
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
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_setVsync(
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
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_getImplementationInfo(
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

    match env.new_string(info) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Get vendor name
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_getVendor(
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

    match env.new_string(vendor) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Get renderer name
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_getRenderer(
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

    match env.new_string(renderer) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Get driver version
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_getVersion(
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

    match env.new_string(version) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Get max texture size
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_getMaxTextureSize(
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
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_getUniformOffsetAlignment(
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
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_isZZeroToOne(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
) -> jboolean {
    if device_ptr == 0 {
        return 0;
    }

    // WebGPU always uses 0-1 Z range
    1
}

/// Create an empty buffer
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_createBufferEmpty(
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
        Ok(buffer_id) => buffer_id.into_raw() as jlong,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to create buffer: {}", e));
            0
        }
    }
}

/// Create a buffer with initial data
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_createBufferData(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    data: jbyteArray,
    usage: jint,
) -> jlong {
    if device_ptr == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null device pointer");
        return 0;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    // Get byte array data
    let data_vec: Vec<u8> = match env.convert_byte_array(data) {
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
            let _ = device.write_buffer(buffer_id, 0, &data_vec);
            buffer_id.into_raw() as jlong
        }
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to create buffer: {}", e));
            0
        }
    }
}

/// Write data to a buffer
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_writeBuffer(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    buffer_ptr: jlong,
    data_ptr: jbyteArray,
    offset: jlong,
) {
    if device_ptr == 0 || buffer_ptr == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null pointer");
        return;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };
    let buffer_id = wgpu_core::id::BufferId::from_raw(buffer_ptr as u64);

    // Get byte array data
    let data: Vec<u8> = match env.convert_byte_array(data_ptr) {
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
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_destroyBuffer(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    buffer_ptr: jlong,
) {
    if device_ptr != 0 && buffer_ptr != 0 {
        unsafe {
            let device = &*(device_ptr as *const BasaltDevice);
            let buffer_id = wgpu_core::id::BufferId::from_raw(buffer_ptr as u64);
            device.destroy_buffer(buffer_id);
        }
    }
}

/// Create a texture
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_createTexture(
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

    match device.create_texture(width as u32, height as u32, depth as u32, mip_levels as u32, format as u32, usage as u32) {
        Ok(texture_id) => texture_id.into_raw() as jlong,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to create texture: {}", e));
            0
        }
    }
}

/// Destroy a texture
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_destroyTexture(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    texture_ptr: jlong,
) {
    if device_ptr != 0 && texture_ptr != 0 {
        unsafe {
            let device = &*(device_ptr as *const BasaltDevice);
            let texture_id = wgpu_core::id::TextureId::from_raw(texture_ptr as u64);
            device.destroy_texture(texture_id);
        }
    }
}

/// Create a texture view
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_createTextureView(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    texture_ptr: jlong,
) -> jlong {
    if device_ptr == 0 || texture_ptr == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null pointer");
        return 0;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };
    let texture_id = wgpu_core::id::TextureId::from_raw(texture_ptr as u64);

    match device.create_texture_view(texture_id) {
        Ok(view_id) => view_id.into_raw() as jlong,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to create texture view: {}", e));
            0
        }
    }
}

/// Close/release the device
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_close(
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

/// Create a sampler
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_createSampler(
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
        Ok(sampler_id) => sampler_id.into_raw() as jlong,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to create sampler: {}", e));
            0
        }
    }
}

/// Translate GLSL shader to WGSL
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_shader_WgslCompiler_translateGlslToWgsl(
    mut env: JNIEnv,
    _class: JClass,
    glsl_source: JString,
    stage: jint,
) -> jstring {
    let glsl_str = match env.get_string(&glsl_source) {
        Ok(s) => s,
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

/// Create a render pipeline
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_createRenderPipeline(
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
    blend_color_factor: jint,
    blend_alpha_factor: jint,
) -> jlong {
    if device_ptr == 0 {
        let _ = env.throw_new("java/lang/IllegalArgumentException", "Null device pointer");
        return 0;
    }

    let device = unsafe { &*(device_ptr as *const BasaltDevice) };

    let vs_str = match env.get_string(&vertex_shader) {
        Ok(s) => s.into(),
        Err(e) => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", &format!("Invalid vertex shader: {}", e));
            return 0;
        }
    };

    let fs_str = match env.get_string(&fragment_shader) {
        Ok(s) => s.into(),
        Err(e) => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", &format!("Invalid fragment shader: {}", e));
            return 0;
        }
    };

    let desc = pipeline::RenderPipelineDescriptor {
        vertex_shader: vs_str,
        fragment_shader: Some(fs_str),
        vertex_format: vertex_format as u32,
        primitive_topology: primitive_topology as u32,
        depth_test_enabled: depth_test_enabled != 0,
        depth_write_enabled: depth_write_enabled != 0,
        depth_compare: depth_compare as u32,
        blend_enabled: blend_enabled != 0,
        blend_color_factor: blend_color_factor as u32,
        blend_alpha_factor: blend_alpha_factor as u32,
    };

    match device.create_render_pipeline(desc) {
        Ok(pipeline_id) => pipeline_id.into_raw() as jlong,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to create pipeline: {}", e));
            0
        }
    }
}

/// Begin a render pass
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_beginRenderPass(
    mut env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    color_texture_ptr: jlong,
    depth_texture_ptr: jlong,
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

    let color_view = if color_texture_ptr != 0 {
        Some(wgpu_core::id::TextureViewId::from_raw(color_texture_ptr as u64))
    } else {
        None
    };

    let depth_view = if depth_texture_ptr != 0 {
        Some(wgpu_core::id::TextureViewId::from_raw(depth_texture_ptr as u64))
    } else {
        None
    };

    match device.begin_render_pass(color_view, depth_view, clear_color, clear_depth, clear_stencil, width as u32, height as u32) {
        Ok(encoder_id) => encoder_id.into_raw() as jlong,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &format!("Failed to begin render pass: {}", e));
            0
        }
    }
}

/// Set pipeline in render pass
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_setPipeline(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    render_pass_ptr: jlong,
    pipeline_ptr: jlong,
) {
    if device_ptr != 0 && render_pass_ptr != 0 && pipeline_ptr != 0 {
        unsafe {
            let device = &*(device_ptr as *const BasaltDevice);
            let render_pass_id = wgpu_core::id::CommandEncoderId::from_raw(render_pass_ptr as u64);
            let pipeline_id = wgpu_core::id::RenderPipelineId::from_raw(pipeline_ptr as u64);
            let _ = device.set_pipeline(render_pass_id, pipeline_id);
        }
    }
}

/// Set vertex buffer
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_setVertexBuffer(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    render_pass_ptr: jlong,
    slot: jint,
    buffer_ptr: jlong,
    offset: jlong,
) {
    if device_ptr != 0 && render_pass_ptr != 0 && buffer_ptr != 0 {
        unsafe {
            let device = &*(device_ptr as *const BasaltDevice);
            let render_pass_id = wgpu_core::id::CommandEncoderId::from_raw(render_pass_ptr as u64);
            let buffer_id = wgpu_core::id::BufferId::from_raw(buffer_ptr as u64);
            let _ = device.set_vertex_buffer(render_pass_id, slot as u32, buffer_id, offset as u64);
        }
    }
}

/// Set index buffer
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_setIndexBuffer(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    render_pass_ptr: jlong,
    buffer_ptr: jlong,
    index_type: jint,
    offset: jlong,
) {
    if device_ptr != 0 && render_pass_ptr != 0 && buffer_ptr != 0 {
        unsafe {
            let device = &*(device_ptr as *const BasaltDevice);
            let render_pass_id = wgpu_core::id::CommandEncoderId::from_raw(render_pass_ptr as u64);
            let buffer_id = wgpu_core::id::BufferId::from_raw(buffer_ptr as u64);
            let _ = device.set_index_buffer(render_pass_id, buffer_id, index_type as u32, offset as u64);
        }
    }
}

/// Draw indexed
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_drawIndexed(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    render_pass_ptr: jlong,
    index_count: jint,
    instance_count: jint,
    first_index: jint,
    base_vertex: jint,
    first_instance: jint,
) {
    if device_ptr != 0 && render_pass_ptr != 0 {
        unsafe {
            let device = &*(device_ptr as *const BasaltDevice);
            let render_pass_id = wgpu_core::id::CommandEncoderId::from_raw(render_pass_ptr as u64);
            let _ = device.draw_indexed(render_pass_id, index_count as u32, instance_count as u32, first_index as u32, base_vertex as i32, first_instance as u32);
        }
    }
}

/// End render pass and submit
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_basalt_backend_BasaltDevice_endRenderPass(
    _env: JNIEnv,
    _class: JClass,
    device_ptr: jlong,
    render_pass_ptr: jlong,
) {
    if device_ptr != 0 && render_pass_ptr != 0 {
        unsafe {
            let device = &*(device_ptr as *const BasaltDevice);
            let render_pass_id = wgpu_core::id::CommandEncoderId::from_raw(render_pass_ptr as u64);
            if let Err(e) = device.end_render_pass(render_pass_id) {
                log::error!("Failed to end render pass: {}", e);
            }
        }
    }
}
