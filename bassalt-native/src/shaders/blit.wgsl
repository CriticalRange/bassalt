// Blit shader - fullscreen triangle pattern from rend3/Bevy/wgpu-examples
// Blits from source texture to swapchain

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Use wgpu-mc style: 6 vertices for two triangles (quad)
    // This matches wgpu's coordinate system better
    // UV: For WebGPU, texture origin is top-left, so V=0 at top
    // Flip V to correct the image orientation
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
    return textureSample(src_texture, src_sampler, in.tex_coord);
}
