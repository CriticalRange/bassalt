// Entity outline box blur fragment shader
// Converted from post/entity_outline_box_blur.fsh
// A simplified box blur with fixed radius for entity outlines

struct FragmentInput {
    @location(0) tex_coord: vec2<f32>,
}

// Sampler info uniform
struct SamplerInfo {
    OutSize: vec2<f32>,
    InSize: vec2<f32>,
}

// Blur configuration uniform
struct BlurConfig {
    BlurDir: vec2<f32>,
    Radius: f32,
    _padding: f32,
}

@group(0) @binding(0)
var<uniform> sampler_info: SamplerInfo;

@group(0) @binding(1)
var<uniform> blur_config: BlurConfig;

@group(0) @binding(2)
var in_texture: texture_2d<f32>;

@group(0) @binding(3)
var in_sampler: sampler;

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    let one_texel = 1.0 / sampler_info.InSize;
    let sample_step = one_texel * blur_config.BlurDir;
    
    var blurred = vec4<f32>(0.0);
    let radius = 2.0; // Fixed radius for entity outlines
    
    // Sample between pixels with step of 2
    var a = -radius + 0.5;
    loop {
        if (a > radius) {
            break;
        }
        blurred += textureSample(in_texture, in_sampler, in.tex_coord + sample_step * a);
        a += 2.0;
    }
    
    // Last sample with half weight
    blurred += textureSample(in_texture, in_sampler, in.tex_coord + sample_step * radius) / 2.0;
    
    // Normalize RGB but preserve original alpha
    let normalized = blurred / (radius + 0.5);
    return vec4<f32>(normalized.rgb, blurred.a);
}
