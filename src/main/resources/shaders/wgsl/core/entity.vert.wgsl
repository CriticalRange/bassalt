// Entity vertex shader
// Used for mobs, players, items
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

// Entity vertex format: POSITION_COLOR_TEX_TEX_NORMAL
// UV2 is integer lightmap coordinates (stored as floats in vertex buffer)
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv0: vec2<f32>,
    @location(3) uv2_int: vec2<f32>,  // Lightmap integer coords (read as float, convert to i32)
    @location(4) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) vertex_color: vec4<f32>,
    @location(2) spherical_dist: f32,
    @location(3) cylindrical_dist: f32,
}

// Convert float to integer and fetch lightmap texel
// Matches GLSL: lightMapColor = texelFetch(Sampler2, UV2 / 16, 0);
fn minecraft_fetch_lightmap(uv_int: vec2<f32>) -> vec4<f32> {
    // Convert float to integer (bit cast since data is stored as int)
    let icoords = vec2<i32>(i32(uv_int.x), i32(uv_int.y));
    // Divide by 16 and fetch
    return textureLoad(Sampler2, icoords / 16, 0);
}

fn fog_spherical_distance(pos: vec3<f32>) -> f32 {
    return length(pos);
}

fn fog_cylindrical_distance(pos: vec3<f32>) -> f32 {
    let distXZ = length(pos.xz);
    let distY = abs(pos.y);
    return max(distXZ, distY);
}

@vertex
fn main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    out.position = projection.ProjMat * transforms.ModelViewMat * vec4<f32>(in.position, 1.0);

    out.spherical_dist = fog_spherical_distance(in.position);
    out.cylindrical_dist = fog_cylindrical_distance(in.position);

    // Fetch lightmap using integer coordinates
    let lightmapColor = minecraft_fetch_lightmap(in.uv2_int);
    out.vertex_color = in.color * lightmapColor;

    // Apply texture matrix to UV
    let transformed_uv = transforms.TextureMat * vec4<f32>(in.uv0, 0.0, 1.0);
    out.tex_coord = transformed_uv.xy;

    return out;
}
