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
var<private> vertexColor_1: vec4<f32>;
var<private> fragColor: vec4<f32>;

fn main_1() {
    var color: vec4<f32>;

    let _e10 = vertexColor_1;
    color = _e10;
    let _e12 = color;
    if (_e12.w < 0.1f) {
        {
            discard;
        }
    }
    let _e16 = color;
    let _e17 = global.ColorModulator;
    fragColor = (_e16 * _e17);
    return;
}

@fragment 
fn main(@location(0) vertexColor: vec4<f32>) -> FragmentOutput {
    vertexColor_1 = vertexColor;
    main_1();
    let _e15 = fragColor;
    return FragmentOutput(_e15);
}
