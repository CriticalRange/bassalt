// Terrain fragment shader
// Converted from terrain.fsh (simplified - without RGSS)

struct FragmentInput {
    @location(0) spherical_vertex_distance: f32,
    @location(1) cylindrical_vertex_distance: f32,
    @location(2) vertex_color: vec4<f32>,
    @location(3) tex_coord0: vec2<f32>,
}

struct DynamicTransforms {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    _padding: f32,
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

struct ChunkSection {
    ChunkPosition: vec3<f32>,
    _padding0: f32,
    ChunkVisibility: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    TextureSize: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> dynamic_transforms: DynamicTransforms;

@group(0) @binding(2)
var texture0: texture_2d<f32>;

@group(0) @binding(3)
var sampler0: sampler;

@group(0) @binding(4)
var<uniform> fog: Fog;

@group(0) @binding(6)
var<uniform> chunk_section: ChunkSection;

// Fog utility functions
fn linear_fog_value(vertex_distance: f32, fog_start: f32, fog_end: f32) -> f32 {
    if (vertex_distance <= fog_start) {
        return 0.0;
    } else if (vertex_distance >= fog_end) {
        return 1.0;
    }
    return (vertex_distance - fog_start) / (fog_end - fog_start);
}

fn total_fog_value(spherical_dist: f32, cylindrical_dist: f32, env_start: f32, env_end: f32, render_start: f32, render_end: f32) -> f32 {
    return max(linear_fog_value(spherical_dist, env_start, env_end), linear_fog_value(cylindrical_dist, render_start, render_end));
}

fn apply_fog(in_color: vec4<f32>, spherical_dist: f32, cylindrical_dist: f32, fog_color: vec4<f32>, env_start: f32, env_end: f32, render_start: f32, render_end: f32) -> vec4<f32> {
    let fog_value = total_fog_value(spherical_dist, cylindrical_dist, env_start, env_end, render_start, render_end);
    return vec4<f32>(mix(in_color.rgb, fog_color.rgb, fog_value * fog_color.a), in_color.a);
}

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(texture0, sampler0, in.tex_coord0);
    var color = tex_color * in.vertex_color;
    
    // Mix with fog color based on chunk visibility
    color = mix(fog.FogColor * vec4<f32>(1.0, 1.0, 1.0, color.a), color, chunk_section.ChunkVisibility);
    
    // Alpha cutout
    if (color.a < 0.5) {
        discard;
    }
    
    return apply_fog(color, in.spherical_vertex_distance, in.cylindrical_vertex_distance, fog.FogColor, fog.FogEnvironmentalStart, fog.FogEnvironmentalEnd, fog.FogRenderDistanceStart, fog.FogRenderDistanceEnd);
}
