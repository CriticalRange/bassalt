// Text intensity vertex shader
//
// All bindings in group 0 to match Bassalt's single bind group approach
// Vertex format: POSITION_COLOR_TEX_LIGHTMAP (position, color, uv0, uv2)

struct DynamicTransforms_t {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    _pad0: f32,
    TextureMat: mat4x4<f32>,
}

struct Projection_t {
    ProjMat: mat4x4<f32>,
}

// Group 0 bindings
@group(0) @binding(0) var Sampler0: texture_2d<f32>;
@group(0) @binding(1) var Sampler0Sampler: sampler;
@group(0) @binding(2) var Sampler2: texture_2d<f32>;
@group(0) @binding(3) var Sampler2Sampler: sampler;
@group(0) @binding(4) var<uniform> DynamicTransforms: DynamicTransforms_t;
@group(0) @binding(5) var<uniform> Projection: Projection_t;

// Text vertex format: POSITION_COLOR_TEX_LIGHTMAP
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv0: vec2<f32>,
    @location(3) uv2: vec2<f32>,  // Lightmap coordinates
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) vertex_color: vec4<f32>,
    @location(1) tex_coord: vec2<f32>,
}

// Sample lightmap using integer UV2 coordinates
fn minecraft_sample_lightmap(uv: vec2<f32>) -> vec4<f32> {
    let uv_clamped = clamp((uv / 256.0) + 0.5 / 16.0, vec2<f32>(0.5 / 16.0), vec2<f32>(15.5 / 16.0));
    return textureSampleLevel(Sampler2, Sampler2Sampler, uv_clamped, 0.0);
}

@vertex
fn main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = Projection.ProjMat * DynamicTransforms.ModelViewMat * vec4<f32>(in.position, 1.0);

    // Apply lightmap color to vertex color
    let lightmapColor = minecraft_sample_lightmap(in.uv2);
    out.vertex_color = in.color * lightmapColor;

    out.tex_coord = in.uv0;
    return out;
}
