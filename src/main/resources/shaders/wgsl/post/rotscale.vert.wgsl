// Rotscale vertex shader for post-processing
// Converts Minecraft GLSL rotscale.vsh to WGSL
// Used for spider vision effect with rotation and scaling

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) texCoord: vec2<f32>,
    @location(1) scaledCoord: vec2<f32>,
}

struct SamplerInfo {
    OutSize: vec2<f32>,
    InSize: vec2<f32>,
}

struct RotScaleConfig {
    InScale: vec2<f32>,
    InOffset: vec2<f32>,
    InRotation: f32,
}

@group(0) @binding(0) var<uniform> sampler_info: SamplerInfo;
@group(0) @binding(1) var<uniform> rot_scale_config: RotScaleConfig;

@vertex
fn main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // Standard fullscreen triangle pattern
    let uv = vec2<f32>(f32((vertex_index << 1u) & 2u), f32(vertex_index & 2u));
    out.position = vec4<f32>(uv * vec2<f32>(2.0, 2.0) + vec2<f32>(-1.0, -1.0), 0.0, 1.0);
    out.texCoord = uv;

    // Apply rotation and scaling
    let deg2Rad = 0.0174532925; // PI / 180
    let inRadians = rot_scale_config.InRotation * deg2Rad;
    let cosine = cos(inRadians);
    let sine = sin(inRadians);
    let rotU = out.texCoord.x * cosine - out.texCoord.y * sine;
    let rotV = out.texCoord.y * cosine + out.texCoord.x * sine;
    out.scaledCoord = vec2<f32>(rotU, rotV) * rot_scale_config.InScale + rot_scale_config.InOffset;

    return out;
}
