// Terrain fragment shader
// Converted from Minecraft GLSL with proper fog and texture sampling
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

// ChunkSection matches GLSL std140 layout exactly (96 bytes)
struct ChunkSection {
    ModelViewMat: mat4x4<f32>,
    ChunkVisibility: f32,
    _pad0: vec2<f32>,             // padding for alignment - GLSL uses ivec2 TextureSize
    ChunkPosition: vec3<i32>,     // GLSL uses ivec3
    _pad1: f32,
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

@group(0) @binding(0) var Sampler0: texture_2d<f32>;
@group(0) @binding(1) var Sampler0Sampler: sampler;
@group(0) @binding(2) var Sampler2: texture_2d<f32>;
@group(0) @binding(3) var Sampler2Sampler: sampler;
@group(0) @binding(4) var<uniform> transforms: DynamicTransforms;
@group(0) @binding(5) var<uniform> projection: Projection;
@group(0) @binding(6) var<uniform> chunk_section: ChunkSection;
@group(0) @binding(8) var<uniform> fog: Fog;

struct FragmentInput {
    @location(0) tex_coord: vec2<f32>,
    @location(1) vertex_color: vec4<f32>,
    @location(2) spherical_dist: f32,
    @location(3) cylindrical_dist: f32,
}

fn sampleNearest(source: texture_2d<f32>, samp: sampler, uv: vec2<f32>, pixelSize: vec2<f32>) -> vec4<f32> {
    let du = dpdx(uv);
    let dv = dpdy(uv);
    let texelScreenSize = sqrt(du * du + dv * dv);
    let uvTexelCoords = uv / pixelSize;
    let texelCenter = round(uvTexelCoords) - 0.5;
    let texelOffset = uvTexelCoords - texelCenter;
    let adjustedOffset = (texelOffset - 0.5) * pixelSize / texelScreenSize + 0.5;
    let clampedOffset = clamp(adjustedOffset, vec2<f32>(0.0), vec2<f32>(1.0));
    let adjustedUV = (texelCenter + clampedOffset) * pixelSize;
    return textureSampleGrad(source, samp, adjustedUV, du, dv);
}

fn sampleTerrain(source: texture_2d<f32>, samp: sampler, uv: vec2<f32>) -> vec4<f32> {
    let pixelSize = 1.0 / chunk_section._pad0;  // TextureSize is stored in _pad0 for alignment
    return sampleNearest(source, samp, uv, pixelSize);
}

fn linear_fog_value(vertexDistance: f32, fogStart: f32, fogEnd: f32) -> f32 {
    if (vertexDistance <= fogStart) {
        return 0.0;
    } else if (vertexDistance >= fogEnd) {
        return 1.0;
    }
    return (vertexDistance - fogStart) / (fogEnd - fogStart);
}

fn total_fog_value(sphericalVertexDistance: f32, cylindricalVertexDistance: f32) -> f32 {
    let envFog = linear_fog_value(sphericalVertexDistance, fog.FogEnvironmentalStart, fog.FogEnvironmentalEnd);
    let renderFog = linear_fog_value(cylindricalVertexDistance, fog.FogRenderDistanceStart, fog.FogRenderDistanceEnd);
    return max(envFog, renderFog);
}

fn apply_fog(inColor: vec4<f32>, sphericalVertexDistance: f32, cylindricalVertexDistance: f32) -> vec4<f32> {
    let fogValue = total_fog_value(sphericalVertexDistance, cylindricalVertexDistance);
    let rgb = mix(inColor.rgb, fog.FogColor.rgb, fogValue * fog.FogColor.a);
    return vec4<f32>(rgb, inColor.a);
}

@fragment
fn main(in: FragmentInput) -> @location(0) vec4<f32> {
    let tex_color = sampleTerrain(Sampler0, Sampler0Sampler, in.tex_coord);
    var color = tex_color * in.vertex_color * transforms.ColorModulator;

    color = mix(fog.FogColor * vec4<f32>(1.0, 1.0, 1.0, color.a), color, chunk_section.ChunkVisibility);

    if (color.a < 0.1) {
        discard;
    }

    color = apply_fog(color, in.spherical_dist, in.cylindrical_dist);

    return color;
}
