// Entity outline box blur post-processing fragment shader
// Converts Minecraft GLSL entity_outline_box_blur.fsh to WGSL
// Note: Original uses #moj_import <minecraft:globals.glsl> for MenuBlurRadius

struct SamplerInfo {
    OutSize: vec2<f32>,
    InSize: vec2<f32>,
}

struct BlurConfig {
    BlurDir: vec2<f32>,
    Radius: f32,
}

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var<uniform> sampler_info: SamplerInfo;
@group(0) @binding(3) var<uniform> blur_config: BlurConfig;

// This shader relies on GL_LINEAR sampling to reduce the amount of texture samples in half.
// Instead of sampling each pixel position with a step of 1 we sample between pixels with a step of 2.
// In the end we sample the last pixel with a half weight, since the amount of pixels to sample is always odd (actualRadius * 2 + 1).

@fragment
fn main(@location(0) texCoord: vec2<f32>) -> @location(0) vec4<f32> {
    let oneTexel = 1.0 / sampler_info.InSize;
    let sampleStep = oneTexel * blur_config.BlurDir;

    var blurred = vec4<f32>(0.0);
    let actualRadius = select(0.0, floor(blur_config.Radius + 0.5), blur_config.Radius >= 0.5);

    var a: f32 = -actualRadius + 0.5;
    loop {
        if (a > actualRadius) { break; }
        blurred = blurred + textureSample(input_texture, input_sampler, texCoord + sampleStep * a);
        a = a + 2.0;
    }

    blurred = blurred + textureSample(input_texture, input_sampler, texCoord + sampleStep * actualRadius) / 2.0;
    return blurred / (actualRadius + 0.5);
}
