// Entity vertex shader
// Used for mobs, players, items
//
// Simplified layout:
// Group 0: Textures (entity texture, overlay, lightmap)
// Group 1: DynamicTransforms
// Group 2: Projection

struct DynamicTransforms {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    _pad0: f32,
    TextureMat: mat4x4<f32>,
}

struct Projection {
    ProjMat: mat4x4<f32>,
}

// Group 0: Textures
@group(0) @binding(0) var Sampler0: texture_2d<f32>;
@group(0) @binding(1) var Sampler0Sampler: sampler;
@group(0) @binding(2) var Sampler2: texture_2d<f32>;
@group(0) @binding(3) var Sampler2Sampler: sampler;

// Group 1: DynamicTransforms
@group(1) @binding(0) var<uniform> transforms: DynamicTransforms;

// Group 2: Projection
@group(2) @binding(0) var<uniform> projection: Projection;

// Entity vertex format: POSITION_COLOR_TEX_TEX_NORMAL
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv0: vec2<f32>,
    @location(3) uv2: vec2<f32>,
    @location(4) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) vertex_color: vec4<f32>,
}

fn sample_lightmap(uv: vec2<f32>) -> vec4<f32> {
    let uv_clamped = clamp((uv / 256.0) + 0.5 / 16.0, vec2<f32>(0.5 / 16.0), vec2<f32>(15.5 / 16.0));
    return textureSampleLevel(Sampler2, Sampler2Sampler, uv_clamped, 0.0);
}

@vertex
fn main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    out.position = projection.ProjMat * transforms.ModelViewMat * vec4<f32>(in.position, 1.0);
    
    // Sample lightmap
    let lightmapColor = sample_lightmap(in.uv2);
    out.vertex_color = in.color * lightmapColor;
    
    // Apply texture matrix to UV
    let transformed_uv = transforms.TextureMat * vec4<f32>(in.uv0, 0.0, 1.0);
    out.tex_coord = transformed_uv.xy;
    
    return out;
}
