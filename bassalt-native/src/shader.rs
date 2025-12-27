//! Shader compilation and translation using naga

use naga::{ShaderStage, Module, front, back, valid};
use crate::error::{BasaltError, Result};

/// Translate GLSL to WGSL
pub fn glsl_to_wgsl(glsl_source: &str, stage: ShaderStage) -> Result<String> {
    // Parse GLSL
    let parser = front::glsl::Parser::default();

    let glsl_options = front::glsl::Options {
        stage,
        defines: std::collections::HashMap::new(),
    };

    let module = parser
        .parse(&glsl_options, glsl_source)
        .map_err(|e| BasaltError::ShaderCompilation(format!("GLSL parse error: {}", e)))?;

    // Validate the module
    let validator = valid::Validator::new(
        valid::ValidationFlags::all(),
        valid::Capabilities::default(),
    );

    let module_info = validator
        .validate(&module)
        .map_err(|e| BasaltError::ShaderValidation(format!("Validation error: {}", e)))?;

    // Write to WGSL
    let wgsl = back::wgsl::write_string(&module, &module_info)
        .map_err(|e| BasaltError::ShaderCompilation(format!("WGSL generation error: {}", e)))?;

    Ok(wgsl)
}

/// Translate SPIR-V to WGSL
pub fn spirv_to_wgsl(spirv_data: &[u32], stage: ShaderStage) -> Result<String> {
    let parser = front::spv::Parser::default();

    let module = parser
        .parse(spirv_data)
        .map_err(|e| BasaltError::ShaderCompilation(format!("SPIR-V parse error: {}", e)))?;

    // Validate
    let validator = valid::Validator::new(
        valid::ValidationFlags::all(),
        valid::Capabilities::all(),
    );

    let module_info = validator
        .validate(&module)
        .map_err(|e| BasaltError::ShaderValidation(format!("Validation error: {}", e)))?;

    // Write to WGSL
    let wgsl = back::wgsl::write_string(&module, &module_info)
        .map_err(|e| BasaltError::ShaderCompilation(format!("WGSL generation error: {}", e)))?;

    Ok(wgsl)
}

/// Compile WGSL directly to a module
pub fn parse_wgsl(wgsl_source: &str) -> Result<Module> {
    let parser = front::wgsl::Parser::default();

    parser.parse(&wgsl_source).map_err(|e| {
        BasaltError::ShaderCompilation(format!("WGSL parse error: {}", e))
    })
}

/// Get shader stage from string
pub fn parse_shader_stage(stage: &str) -> Result<ShaderStage> {
    match stage.to_lowercase().as_str() {
        "vertex" | "vs" => Ok(ShaderStage::Vertex),
        "fragment" | "fs" | "pixel" | "ps" => Ok(ShaderStage::Fragment),
        "compute" | "cs" => Ok(ShaderStage::Compute),
        _ => Err(BasaltError::InvalidParameter(format!(
            "Unknown shader stage: {}",
            stage
        ))),
    }
}
