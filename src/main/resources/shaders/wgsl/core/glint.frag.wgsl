// Glint fragment shader
// Converted from glint.fsh

struct FragmentInput {
    @location(0) spherical_vertex_distance: f32,
    @location(1) cylindrical_vertex_distance: f32,
    @location(2) tex_coord0: vec2<f32>,
}

struct DynamicTransforms {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    _padding: f32,
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

struct Globals {
    CameraBlockPos: vec3<i32>,
    _padding0: i32,
    CameraOffset: vec3<f32>,
    _padding1: f32,
    ScreenSize: vec2<f32>,
    GlintAlpha: f32,
    GameTime: f32,
}

@group(0) @binding(0)
var<uniform> dynamic_transforms: DynamicTransforms;

@group(0) @binding(2)
var texture0: texture_2d<f32>;

@group(0) @binding(3)
var sampler0: sampler;

@group(0) @binding(4)
var<uniform> fog: Fog;

@group(0) @binding(5)
var<uniform> globals: Globals;

fn linear_fog_value(vertex_distance: f32, fog_start: f32, fog_end: f32) -> f32 {
    if (vertex_distance <= fog_start) {
        return 0.0;
    } else if (vertex_distance >= fog_end) {
        return 1.0;
    }
    return (vertex_distance - fog_start) / (fog_end - fog_start);
}

fn total_fog_value(spherical_dist: f32, cylindrical_dist: f32, env_start: f32, env_end: f32, render_start: f32, render_end: f32) -> f32 {
    return max(linear_fog_value(spherical_dist, env_start, env_end), linear_fog_value(cylindrical_dist, render_start, render_end));
}

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    var color = textureSample(texture0, sampler0, in.tex_coord0) * dynamic_transforms.ColorModulator;
    if (color.a < 0.1) {
        discard;
    }
    let fog_val = total_fog_value(in.spherical_vertex_distance, in.cylindrical_vertex_distance, fog.FogEnvironmentalStart, fog.FogEnvironmentalEnd, fog.FogRenderDistanceStart, fog.FogRenderDistanceEnd);
    let fade = (1.0 - fog_val) * globals.GlintAlpha;
    return vec4<f32>(color.rgb * fade, color.a);
}
