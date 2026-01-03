// Spider clip post-processing fragment shader
// Converts Minecraft GLSL spiderclip.fsh to WGSL
// Used for spider vision effect with rotation/scaling/vignette

struct SamplerInfo {
    OutSize: vec2<f32>,
    InSize: vec2<f32>,
}

struct SpiderConfig {
    Scissor: vec4<f32>,
    Vignette: vec4<f32>,
}

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var blur_texture: texture_2d<f32>;
@group(0) @binding(3) var blur_sampler: sampler;
@group(0) @binding(4) var<uniform> sampler_info: SamplerInfo;
@group(0) @binding(5) var<uniform> spider_config: SpiderConfig;

@fragment
fn main(
    @location(0) texCoord: vec2<f32>,
    @location(1) scaledCoord: vec2<f32>
) -> @location(0) vec4<f32> {
    let scaledTexel = textureSample(input_texture, input_sampler, scaledCoord);
    let blurTexel = textureSample(blur_texture, blur_sampler, texCoord);
    var outTexel = scaledTexel;

    // -- Alpha Clipping --
    if (scaledCoord.x < spider_config.Scissor.x) { outTexel = blurTexel; }
    if (scaledCoord.y < spider_config.Scissor.y) { outTexel = blurTexel; }
    if (scaledCoord.x > spider_config.Scissor.z) { outTexel = blurTexel; }
    if (scaledCoord.y > spider_config.Scissor.w) { outTexel = blurTexel; }

    let clampedScaled = clamp(scaledCoord, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0));

    if (scaledCoord.x < spider_config.Vignette.x) {
        outTexel = mix(blurTexel, outTexel, (spider_config.Scissor.x - scaledCoord.x) / (spider_config.Scissor.x - spider_config.Vignette.x));
    }
    if (scaledCoord.y < spider_config.Vignette.y) {
        outTexel = mix(blurTexel, outTexel, (spider_config.Scissor.y - scaledCoord.y) / (spider_config.Scissor.y - spider_config.Vignette.y));
    }
    if (scaledCoord.x > spider_config.Vignette.z) {
        outTexel = mix(blurTexel, outTexel, (spider_config.Scissor.z - scaledCoord.x) / (spider_config.Scissor.z - spider_config.Vignette.z));
    }
    if (scaledCoord.y > spider_config.Vignette.w) {
        outTexel = mix(blurTexel, outTexel, (spider_config.Scissor.w - scaledCoord.y) / (spider_config.Scissor.w - spider_config.Vignette.w));
    }

    return vec4<f32>(outTexel.rgb, 1.0);
}
