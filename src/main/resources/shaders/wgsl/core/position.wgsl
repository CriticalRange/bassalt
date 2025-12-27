// Position shader - renders colored quads with fog support
// Converted from position.vsh/position.fsh

struct VertexInput {
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) spherical_distance: f32,
    @location(1) cylindrical_distance: f32,
}

struct DynamicTransforms {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    TextureMat: mat4x4<f32>,
}

struct Fog {
    FogColor: vec4<f32>,
    FogEnvironmentalStart: f32,
    FogEnvironmentalEnd: f32,
    FogRenderDistanceStart: f32,
    FogRenderDistanceEnd: f32,
    FogSkyEnd: f32,
    FogCloudsEnd: f32,
}

struct Projection {
    ProjMat: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> dynamic_transforms: DynamicTransforms;

@group(0) @binding(1)
var<uniform> projection: Projection;

@group(0) @binding(2)
var<uniform> fog: Fog;

@vertex
fn main_vs(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = projection.ProjMat * dynamic_transforms.ModelViewMat * vec4<f32>(in.position, 1.0);

    // Calculate fog distances
    out.spherical_distance = length(in.position);
    let dist_xz = length(in.position.xz);
    let dist_y = abs(in.position.y);
    out.cylindrical_distance = max(dist_xz, dist_y);

    return out;
}

struct FragmentInput {
    @location(0) spherical_distance: f32,
    @location(1) cylindrical_distance: f32,
}

fn linear_fog_value(vertex_distance: f32, fog_start: f32, fog_end: f32) -> f32 {
    if (vertex_distance <= fog_start) {
        return 0.0;
    }
    if (vertex_distance >= fog_end) {
        return 1.0;
    }
    return (vertex_distance - fog_start) / (fog_end - fog_start);
}

fn total_fog_value(
    spherical: f32,
    cylindrical: f32,
    env_start: f32,
    env_end: f32,
    render_start: f32,
    render_end: f32
) -> f32 {
    let env_fog = linear_fog_value(spherical, env_start, env_end);
    let render_fog = linear_fog_value(cylindrical, render_start, render_end);
    return max(env_fog, render_fog);
}

fn apply_fog(
    color: vec4<f32>,
    spherical: f32,
    cylindrical: f32,
    env_start: f32,
    env_end: f32,
    render_start: f32,
    render_end: f32,
    fog_color: vec4<f32>
) -> vec4<f32> {
    let fog_value = total_fog_value(spherical, cylindrical, env_start, env_end, render_start, render_end);
    let mixed_color = mix(color.rgb, fog_color.rgb, fog_value * fog_color.a);
    return vec4<f32>(mixed_color, color.a);
}

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    return apply_fog(
        dynamic_transforms.ColorModulator,
        in.spherical_distance,
        in.cylindrical_distance,
        fog.FogEnvironmentalStart,
        fog.FogEnvironmentalEnd,
        fog.FogRenderDistanceStart,
        fog.FogRenderDistanceEnd,
        fog.FogColor
    );
}
