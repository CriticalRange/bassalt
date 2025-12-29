// Blit vertex shader - fullscreen triangle pattern from rend3/Bevy
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
}

@vertex
fn main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Standard fullscreen triangle pattern used by rend3, Bevy, wgpu-examples
    // Creates oversized triangle that covers entire screen with 3 vertices
    out.position = vec4<f32>(
        f32(vertex_index / 2u) * 4.0 - 1.0,
        f32(vertex_index % 2u) * 4.0 - 1.0,
        0.0,
        1.0
    );
    out.tex_coord = vec2<f32>(
        f32(vertex_index / 2u) * 2.0,
        1.0 - f32(vertex_index % 2u) * 2.0
    );
    return out;
}
