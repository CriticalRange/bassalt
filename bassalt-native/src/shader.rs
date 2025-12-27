//! Shader compilation and translation using naga

use naga::{ShaderStage, Module, front, back, valid};
use crate::error::{BasaltError, Result};

/// Translate GLSL to WGSL
pub fn glsl_to_wgsl(glsl_source: &str, stage: ShaderStage) -> Result<String> {
    // Parse GLSL with the new naga 27 API
    let mut frontend = front::glsl::Frontend::default();

    let glsl_options = front::glsl::Options {
        stage,
        defines: Default::default(), // Uses naga's internal FastHashMap
    };

    let module = frontend
        .parse(&glsl_options, glsl_source)
        .map_err(|e| BasaltError::ShaderCompilation(format!("GLSL parse error: {:?}", e)))?;

    // Validate the module
    let mut validator = valid::Validator::new(
        valid::ValidationFlags::all(),
        valid::Capabilities::all(),
    );

    let module_info = validator
        .validate(&module)
        .map_err(|e| BasaltError::ShaderValidation(format!("Validation error: {:?}", e)))?;

    // Write to WGSL with WriterFlags
    let wgsl = back::wgsl::write_string(&module, &module_info, back::wgsl::WriterFlags::empty())
        .map_err(|e| BasaltError::ShaderCompilation(format!("WGSL generation error: {}", e)))?;

    Ok(wgsl)
}

/// Compile GLSL directly to a naga Module
pub fn glsl_to_module(glsl_source: &str, stage: ShaderStage) -> Result<Module> {
    let mut frontend = front::glsl::Frontend::default();
    let glsl_options = front::glsl::Options {
        stage,
        defines: Default::default(),
    };

    let module = frontend
        .parse(&glsl_options, glsl_source)
        .map_err(|e| BasaltError::ShaderCompilation(format!("GLSL parse error: {:?}", e)))?;

    // Validate the module
    let mut validator = valid::Validator::new(
        valid::ValidationFlags::all(),
        valid::Capabilities::all(),
    );

    let _module_info = validator
        .validate(&module)
        .map_err(|e| BasaltError::ShaderValidation(format!("Validation error: {:?}", e)))?;

    Ok(module)
}

/// Compile WGSL directly to a module
pub fn parse_wgsl(wgsl_source: &str) -> Result<Module> {
    front::wgsl::parse_str(&wgsl_source).map_err(|e| {
        BasaltError::ShaderCompilation(format!("WGSL parse error: {:?}", e))
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

