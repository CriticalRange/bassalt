// Blit shader - fullscreen triangle pattern from rend3/Bevy/wgpu-examples
// Blits from source texture to swapchain

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Fullscreen quad with Y-flipped UVs to handle OpenGL->WebGPU coordinate system
    // OpenGL has Y=0 at bottom, WebGPU has Y=0 at top
    // UV.y is flipped: 0->1 and 1->0 to flip the image vertically
    var verts = array(
        vec4<f32>(-1.0, -1.0, 0.0, 0.0),  // bottom-left, uv (0,0) - was (0,1)
        vec4<f32>(1.0, 1.0, 1.0, 1.0),    // top-right, uv (1,1) - was (1,0)
        vec4<f32>(-1.0, 1.0, 0.0, 1.0),   // top-left, uv (0,1) - was (0,0)
        vec4<f32>(-1.0, -1.0, 0.0, 0.0),  // bottom-left, uv (0,0) - was (0,1)
        vec4<f32>(1.0, -1.0, 1.0, 0.0),   // bottom-right, uv (1,0) - was (1,1)
        vec4<f32>(1.0, 1.0, 1.0, 1.0),    // top-right, uv (1,1) - was (1,0)
    );
    
    var pos = verts[vertex_index];
    out.position = vec4<f32>(pos.x, pos.y, 0.0, 1.0);
    out.tex_coord = vec2<f32>(pos.z, pos.w);
    return out;
}

@group(0) @binding(0) var src_texture: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(src_texture, src_sampler, in.tex_coord);
    // Swizzle RGBA -> BGRA for swapchain format compatibility
    // Source texture is RGBA, swapchain is typically BGRA on most platforms
    return vec4<f32>(color.b, color.g, color.r, color.a);
}
