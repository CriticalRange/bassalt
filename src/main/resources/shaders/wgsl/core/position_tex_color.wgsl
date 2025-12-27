// Position + Texture + Color shader - renders textured colored quads
// Converted from position_tex_color.vsh/position_tex_color.fsh

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv0: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) vertex_color: vec4<f32>,
}

struct DynamicTransforms {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    TextureMat: mat4x4<f32>,
}

struct Projection {
    ProjMat: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> dynamic_transforms: DynamicTransforms;

@group(0) @binding(1)
var<uniform> projection: Projection;

@vertex
fn main_vs(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = projection.ProjMat * dynamic_transforms.ModelViewMat * vec4<f32>(in.position, 1.0);
    out.tex_coord = in.uv0;
    out.vertex_color = in.color;
    return out;
}

struct FragmentInput {
    @location(0) tex_coord: vec2<f32>,
    @location(1) vertex_color: vec4<f32>,
}

@group(0) @binding(2)
var tex_sampler: sampler;
@group(0) @binding(3)
var tex_texture: texture_2d<f32>;

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(tex_texture, tex_sampler, in.tex_coord);
    let color = tex_color * in.vertex_color;
    if (color.a == 0.0) {
        discard;
    }
    return color * dynamic_transforms.ColorModulator;
}
