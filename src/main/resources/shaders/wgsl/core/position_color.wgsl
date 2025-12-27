// Position + Color shader - renders colored quads (no texture)
// Converted from position_color.vsh/position_color.fsh

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) vertex_color: vec4<f32>,
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
    out.vertex_color = in.color;
    return out;
}

struct FragmentInput {
    @location(0) vertex_color: vec4<f32>,
}

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    let color = in.vertex_color;
    if (color.a == 0.0) {
        discard;
    }
    return color * dynamic_transforms.ColorModulator;
}
