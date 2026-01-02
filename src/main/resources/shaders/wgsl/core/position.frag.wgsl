// Position-only fragment shader
// Used for weather, particles, etc. that need fog but no textures
//
// Matches GLSL: fragColor = apply_fog(ColorModulator, sphericalVertexDistance, ...)

struct DynamicTransforms {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    _pad0: f32,
    TextureMat: mat4x4<f32>,
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

// Group 0 bindings
@group(0) @binding(4) var<uniform> transforms: DynamicTransforms;
@group(0) @binding(8) var<uniform> fog: Fog;

struct FragmentInput {
    @location(0) spherical_dist: f32,
    @location(1) cylindrical_dist: f32,
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
    // GLSL: fragColor = apply_fog(ColorModulator, sphericalVertexDistance, cylindricalVertexDistance, ...)
    return apply_fog(transforms.ColorModulator, in.spherical_dist, in.cylindrical_dist);
}
