// Position-only vertex shader
// Used for weather, particles, etc. that need fog but no textures
//
// All bindings in group 0 to match Bassalt's single bind group approach

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

// Fog matches GLSL std140 layout exactly (48 bytes)
struct Fog_t {
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

// Group 0 bindings
@group(0) @binding(4) var<uniform> DynamicTransforms: DynamicTransforms_t;
@group(0) @binding(5) var<uniform> Projection: Projection_t;
@group(0) @binding(8) var<uniform> Fog: Fog_t;

struct VertexInput {
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) spherical_dist: f32,
    @location(1) cylindrical_dist: f32,
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

    out.position = Projection.ProjMat * DynamicTransforms.ModelViewMat * vec4<f32>(in.position, 1.0);

    out.spherical_dist = fog_spherical_distance(in.position);
    out.cylindrical_dist = fog_cylindrical_distance(in.position);

    return out;
}
