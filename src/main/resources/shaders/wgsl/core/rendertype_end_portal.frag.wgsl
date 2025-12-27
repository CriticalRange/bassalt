// End portal fragment shader
// Converted from rendertype_end_portal.fsh (simplified - without layered effect)

struct FragmentInput {
    @location(0) tex_proj0: vec4<f32>,
    @location(1) spherical_vertex_distance: f32,
    @location(2) cylindrical_vertex_distance: f32,
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

@group(0) @binding(2)
var texture0: texture_2d<f32>;

@group(0) @binding(3)
var sampler0: sampler;

@group(0) @binding(4)
var<uniform> fog: Fog;

// Portal colors
const PORTAL_COLOR: vec3<f32> = vec3<f32>(0.1, 0.1, 0.3);

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
    // Simplified portal effect using projective texturing
    let tex_coord = in.tex_proj0.xy / in.tex_proj0.w;
    let tex_color = textureSample(texture0, sampler0, tex_coord);
    
    // Blend with portal color for effect
    var color = vec4<f32>(tex_color.rgb * PORTAL_COLOR + PORTAL_COLOR * 0.5, 1.0);
    
    return apply_fog(color, in.spherical_vertex_distance, in.cylindrical_vertex_distance, fog.FogColor, fog.FogEnvironmentalStart, fog.FogEnvironmentalEnd, fog.FogRenderDistanceStart, fog.FogRenderDistanceEnd);
}
