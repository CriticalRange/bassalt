//! Shader compilation and translation using naga

use naga::{ShaderStage, Module, front, back, valid};
use crate::error::{BasaltError, Result, CompilationInfo};

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
        .map_err(|e| {
            // Extract line/column info from naga's ShaderError
            let compilation_info: CompilationInfo = naga::error::ShaderError {
                source: glsl_source.to_string(),
                label: None,
                inner: Box::new(e),
            }.into();

            BasaltError::ShaderCompilation {
                shader_name: "glsl_to_wgsl".to_string(),
                error: compilation_info.to_string(),
                stage: format!("{:?}", stage),
                source: None,
            }
        })?;

    // Validate the module
    let mut validator = valid::Validator::new(
        valid::ValidationFlags::all(),
        valid::Capabilities::all(),
    );

    let module_info = validator
        .validate(&module)
        .map_err(|e| {
            // Extract line/column info from validation error
            let compilation_info: CompilationInfo = naga::error::ShaderError {
                source: String::new(), // Validation errors don't always have source
                label: None,
                inner: Box::new(e),
            }.into();

            BasaltError::ShaderValidation {
                shader_name: "glsl_to_wgsl".to_string(),
                error: compilation_info.to_string(),
            }
        })?;

    // Write to WGSL with WriterFlags
    let wgsl = back::wgsl::write_string(&module, &module_info, back::wgsl::WriterFlags::empty())
        .map_err(|e| BasaltError::shader_compilation(
            "glsl_to_wgsl",
            format!("WGSL generation error: {}", e),
            "wgsl_write",
        ))?;

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
        .map_err(|e| {
            // Extract line/column info from naga's ShaderError
            let compilation_info: CompilationInfo = naga::error::ShaderError {
                source: glsl_source.to_string(),
                label: None,
                inner: Box::new(e),
            }.into();

            BasaltError::ShaderCompilation {
                shader_name: "glsl_to_module".to_string(),
                error: compilation_info.to_string(),
                stage: format!("{:?}", stage),
                source: None,
            }
        })?;

    // Validate the module
    let mut validator = valid::Validator::new(
        valid::ValidationFlags::all(),
        valid::Capabilities::all(),
    );

    let _module_info = validator
        .validate(&module)
        .map_err(|e| {
            // Extract line/column info from validation error
            let compilation_info: CompilationInfo = naga::error::ShaderError {
                source: String::new(),
                label: None,
                inner: Box::new(e),
            }.into();

            BasaltError::ShaderValidation {
                shader_name: "glsl_to_module".to_string(),
                error: compilation_info.to_string(),
            }
        })?;

    Ok(module)
}

/// Compile WGSL directly to a module with detailed error information
pub fn parse_wgsl(wgsl_source: &str) -> Result<Module> {
    parse_wgsl_named(wgsl_source, "unknown")
}

/// Compile WGSL with a shader name for better error logging
pub fn parse_wgsl_named(wgsl_source: &str, shader_name: &str) -> Result<Module> {
    front::wgsl::parse_str(&wgsl_source).map_err(|e| {
        // Extract line/column info from naga's ShaderError
        let compilation_info: CompilationInfo = naga::error::ShaderError {
            source: wgsl_source.to_string(),
            label: Some(shader_name.to_string()),
            inner: Box::new(e),
        }.into();

        // Log the compilation info with all messages
        log_compilation_info(shader_name, &compilation_info);

        // Format the error with location info
        if let Some(msg) = compilation_info.messages.first() {
            if let Some(loc) = &msg.location {
                BasaltError::ShaderParse {
                    error: msg.message.clone(),
                    line: Some(loc.line_number as usize),
                    column: Some(loc.line_position as usize),
                }
            } else {
                BasaltError::ShaderParse {
                    error: msg.message.clone(),
                    line: None,
                    column: None,
                }
            }
        } else {
            BasaltError::ShaderParse {
                error: "Unknown parse error".to_string(),
                line: None,
                column: None,
            }
        }
    })
}

/// Log compilation info to the logger
fn log_compilation_info(shader_name: &str, info: &CompilationInfo) {
    if info.messages.is_empty() {
        return;
    }

    for msg in &info.messages {
        let level = match msg.message_type {
            crate::error::CompilationMessageType::Error => log::Level::Error,
            crate::error::CompilationMessageType::Warning => log::Level::Warn,
            crate::error::CompilationMessageType::Info => log::Level::Info,
        };

        let location_str = if let Some(loc) = &msg.location {
            format!("{}:{}:{}", loc.line_number, loc.line_position, msg.message_type.as_str())
        } else {
            format!("{}:{}", msg.message_type.as_str(), shader_name)
        };

        log::log!(level, "Shader '{}': {} - {}", shader_name, location_str, msg.message);
    }
}

/// Get shader compilation info for WGSL source without creating a module
///
/// This is useful for getting detailed error messages with line/column information
/// for debugging shader compilation issues.
pub fn get_wgsl_compilation_info(wgsl_source: &str) -> CompilationInfo {
    match front::wgsl::parse_str(&wgsl_source) {
        Ok(_) => CompilationInfo::new(),
        Err(e) => {
            naga::error::ShaderError {
                source: wgsl_source.to_string(),
                label: None,
                inner: Box::new(e),
            }.into()
        }
    }
}

/// Get shader stage from string
pub fn parse_shader_stage(stage: &str) -> Result<ShaderStage> {
    match stage.to_lowercase().as_str() {
        "vertex" | "vs" => Ok(ShaderStage::Vertex),
        "fragment" | "fs" | "pixel" | "ps" => Ok(ShaderStage::Fragment),
        "compute" | "cs" => Ok(ShaderStage::Compute),
        _ => Err(BasaltError::invalid_parameter(
            "stage",
            format!("Unknown shader stage: {}", stage),
        )),
    }
}
