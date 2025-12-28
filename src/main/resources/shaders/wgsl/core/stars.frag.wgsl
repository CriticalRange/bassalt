struct DynamicTransforms {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    TextureMat: mat4x4<f32>,
}

struct FragmentOutput {
    @location(0) fragColor: vec4<f32>,
}

@group(0) @binding(0) 
var<uniform> global: DynamicTransforms;
var<private> fragColor: vec4<f32>;

fn main_1() {
    let _e9 = global.ColorModulator;
    fragColor = _e9;
    return;
}

@fragment 
fn main() -> FragmentOutput {
    main_1();
    let _e11 = fragColor;
    return FragmentOutput(_e11);
}
