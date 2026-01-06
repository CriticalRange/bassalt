use naga::{front::wgsl::Parser, valid::Validator, Module};

fn main() {
    let wgsl_code = std::fs::read_to_string("../src/main/resources/shaders/wgsl/core/position.frag.wgsl").unwrap();
    
    let parser = Parser::new();
    let module: Module = parser.parse(&wgsl_code).unwrap();
    
    println!("Fragment shader global variables: {}", module.global_variables.len());
    for (handle, var) in module.global_variables.iter() {
        if let Some(binding) = &var.binding {
            println!("  Handle {:?}: binding={:?}, name={:?}", handle, binding, var.name);
        }
    }
}
