// Box blur combined shader
// Post-processing effect for menu blur and other blur effects
// Uses optimized sampling that samples between pixels for efficiency

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
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
    _padding: f32, // WGSL requires 16-byte alignment
}

// Default menu blur radius (matches Minecraft's default)
const MENU_BLUR_RADIUS: f32 = 8.0;

@group(0) @binding(0)
var<uniform> sampler_info: SamplerInfo;

@group(0) @binding(1)
var<uniform> blur_config: BlurConfig;

@group(0) @binding(2)
var in_texture: texture_2d<f32>;

@group(0) @binding(3)
var in_sampler: sampler;

@vertex
fn main_vs(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Generate fullscreen triangle/quad UVs from vertex index
    let uv = vec2<f32>(
        f32((vertex_index << 1u) & 2u),
        f32(vertex_index & 2u)
    );
    let pos = vec4<f32>(uv * vec2<f32>(2.0, 2.0) + vec2<f32>(-1.0, -1.0), 0.0, 1.0);
    out.position = pos;
    out.tex_coord = uv;
    return out;
}

struct FragmentInput {
    @location(0) tex_coord: vec2<f32>,
}

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    let one_texel = 1.0 / sampler_info.InSize;
    let sample_step = one_texel * blur_config.BlurDir;
    
    var blurred = vec4<f32>(0.0);
    
    // Calculate actual radius - use blur config or default menu blur radius
    var actual_radius: f32;
    if (blur_config.Radius >= 0.5) {
        actual_radius = round(blur_config.Radius);
    } else {
        actual_radius = MENU_BLUR_RADIUS;
    }
    
    // This shader relies on linear sampling to reduce texture samples in half.
    // Instead of sampling each pixel position with a step of 1, we sample between pixels with a step of 2.
    // Start at -actualRadius + 0.5 and step by 2.0
    var a = -actual_radius + 0.5;
    loop {
        if (a > actual_radius) {
            break;
        }
        blurred += textureSample(in_texture, in_sampler, in.tex_coord + sample_step * a);
        a += 2.0;
    }
    
    // Sample the last pixel with half weight (amount of pixels is always odd: actualRadius * 2 + 1)
    blurred += textureSample(in_texture, in_sampler, in.tex_coord + sample_step * actual_radius) / 2.0;
    
    // Normalize by the number of samples
    return blurred / (actual_radius + 0.5);
}
