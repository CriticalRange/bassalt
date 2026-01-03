// Bits post-processing fragment shader
// Converts Minecraft GLSL bits.fsh to WGSL

struct SamplerInfo {
    OutSize: vec2<f32>,
    InSize: vec2<f32>,
}

struct BitsConfig {
    Resolution: f32,
    MosaicSize: f32,
}

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var<uniform> sampler_info: SamplerInfo;
@group(0) @binding(3) var<uniform> config: BitsConfig;

const SATURATION: f32 = 1.5;

@fragment
fn main(@location(0) texCoord: vec2<f32>) -> @location(0) vec4<f32> {
    let oneTexel = 1.0 / sampler_info.InSize;
    let mosaicInSize = sampler_info.InSize / config.MosaicSize;
    let fractPix = fract(texCoord * mosaicInSize) / mosaicInSize;

    let baseTexel = textureSample(input_texture, input_sampler, texCoord - fractPix);

    let fractTexel = baseTexel.rgb - fract(baseTexel.rgb * config.Resolution) / config.Resolution;
    let luma = dot(fractTexel, vec3<f32>(0.3, 0.59, 0.11));
    let chroma = (fractTexel - luma) * SATURATION;
    let outColor = luma + chroma;

    return vec4<f32>(outColor, 1.0);
}
