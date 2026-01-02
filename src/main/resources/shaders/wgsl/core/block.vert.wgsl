// Block vertex shader
// Used for rendering blocks
//
// All bindings in group 0 to match Bassalt's single bind group approach

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

// Fog matches GLSL std140 layout exactly (48 bytes)
struct Fog {
    FogColor: vec4<f32>,
    FogEnvironmentalStart: f32,
    FogEnvironmentalEnd: f32,
    FogRenderDistanceStart: f32,
    FogRenderDistanceEnd: f32,
    FogSkyEnd: f32,
    FogCloudsEnd: f32,
    _pad3: f32,
    _pad4: f32,
}

// All bindings in group 0 with different binding indices
// Bindings 0-3: Textures (Sampler0, Sampler0Sampler, Sampler2, Sampler2Sampler)
// Binding 4: DynamicTransforms
// Binding 5: Projection
// Binding 8: Fog

@group(0) @binding(0) var Sampler0: texture_2d<f32>;
@group(0) @binding(1) var Sampler0Sampler: sampler;
@group(0) @binding(2) var Sampler2: texture_2d<f32>;
@group(0) @binding(3) var Sampler2Sampler: sampler;

@group(0) @binding(4) var<uniform> transforms: DynamicTransforms;
@group(0) @binding(5) var<uniform> projection: Projection;
@group(0) @binding(8) var<uniform> fog: Fog;

// Vertex format: POSITION_COLOR_TEX_TEX_NORMAL (format 7)
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
    @location(2) spherical_dist: f32,
    @location(3) cylindrical_dist: f32,
}

fn fog_spherical_distance(pos: vec3<f32>) -> f32 {
    return length(pos);
}

fn fog_cylindrical_distance(pos: vec3<f32>) -> f32 {
    let distXZ = length(pos.xz);
    let distY = abs(pos.y);
    return max(distXZ, distY);
}

fn sample_lightmap(uv: vec2<f32>) -> vec4<f32> {
    let uv_clamped = clamp((uv / 256.0) + 0.5 / 16.0, vec2<f32>(0.5 / 16.0), vec2<f32>(15.5 / 16.0));
    return textureSampleLevel(Sampler2, Sampler2Sampler, uv_clamped, 0.0);
}

@vertex
fn main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let pos = in.position + transforms.ModelOffset;
    out.position = projection.ProjMat * transforms.ModelViewMat * vec4<f32>(pos, 1.0);

    out.spherical_dist = fog_spherical_distance(pos);
    out.cylindrical_dist = fog_cylindrical_distance(pos);

    // Sample lightmap
    let lightmapColor = sample_lightmap(in.uv2);
    out.vertex_color = in.color * lightmapColor;

    out.tex_coord = in.uv0;

    return out;
}
