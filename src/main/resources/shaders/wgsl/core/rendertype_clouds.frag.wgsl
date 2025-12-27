// Clouds fragment shader
// Converted from rendertype_clouds.fsh

struct FragmentInput {
    @location(0) vertex_distance: f32,
    @location(1) vertex_color: vec4<f32>,
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

struct DynamicTransforms {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    _padding: f32,
    TextureMat: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> dynamic_transforms: DynamicTransforms;

@group(0) @binding(4)
var<uniform> fog: Fog;

fn linear_fog_value(vertex_distance: f32, fog_start: f32, fog_end: f32) -> f32 {
    if (vertex_distance <= fog_start) {
        return 0.0;
    } else if (vertex_distance >= fog_end) {
        return 1.0;
    }
    return (vertex_distance - fog_start) / (fog_end - fog_start);
}

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    var color = in.vertex_color;
    color.a = color.a * (1.0 - linear_fog_value(in.vertex_distance, 0.0, fog.FogCloudsEnd));
    return color;
}
