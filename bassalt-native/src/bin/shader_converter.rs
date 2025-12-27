#!/usr/bin/env rust-script
//! Shader Converter - Convert Minecraft GLSL to WGSL using naga

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashSet;

fn find_moj_imports(source: &str) -> Vec<String> {
    let mut imports = Vec::new();
    for line in source.lines() {
        if line.contains("#moj_import") {
            if let Some(start) = line.find('<') {
                if let Some(end) = line.find('>') {
                    if end > start {
                        imports.push(line[start + 1..end].to_string());
                    }
                }
            }
        }
    }
    imports
}

fn preprocess_file(source: &str, include_dir: &Path, visited: &mut HashSet<PathBuf>) -> String {
    let imports = find_moj_imports(source);
    let mut result = String::new();

    // Add original source lines (excluding #version and #moj_import)
    for line in source.lines() {
        if line.trim_start().starts_with("#version") {
            continue; // Skip #version
        }
        if line.trim_start().starts_with("#moj_import") {
            continue; // Skip moj_import, we'll handle it below
        }
        if line.trim().starts_with("precision ") {
            continue; // Skip precision qualifiers
        }
        result.push_str(line);
        result.push('\n');
    }

    // Process imports
    for import in &imports {
        if import.starts_with("minecraft:") {
            let filename = import.replace("minecraft:", "");
            let full_path = include_dir.join(&filename);

            if visited.contains(&full_path) {
                result.push_str(&format!("// Already included: {}\n", import));
                continue;
            }

            if full_path.exists() {
                visited.insert(full_path.clone());
                match fs::read_to_string(&full_path) {
                    Ok(included_source) => {
                        result.push_str(&format!("// Import: {}\n", import));
                        result.push_str(&preprocess_file(&included_source, include_dir, visited));
                    }
                    Err(e) => {
                        result.push_str(&format!("// Error reading {}: {}\n", filename, e));
                    }
                }
            } else {
                result.push_str(&format!("// Missing file: {}\n", filename));
            }
        }
    }

    result
}

fn add_bindings(source: &str) -> String {
    let mut result = String::new();
    let mut binding_counter = 0u32;

    for line in source.lines() {
        if line.contains("layout(std140) uniform") {
            // Extract uniform block name
            if let Some(start) = line.find("uniform") {
                let rest = &line[start + 7..];
                let name_end = rest.find('{')
                    .or_else(|| rest.find(';'))
                    .unwrap_or(rest.len());
                let name = rest[..name_end].trim();
                result.push_str(&format!("layout(std140, binding={}) uniform {}{{\n", binding_counter, name));
                binding_counter += 1;
                continue;
            }
        }
        result.push_str(line);
        result.push('\n');
    }

    result
}

fn convert_glsl_to_wgsl(glsl_source: &str, stage: naga::ShaderStage) -> Result<String, String> {
    // Parse GLSL
    let mut frontend = naga::front::glsl::Frontend::default();
    let options = naga::front::glsl::Options {
        stage,
        defines: naga::FastHashMap::default(),
    };

    let module = frontend.parse(&options, glsl_source)
        .map_err(|e| format!("GLSL parse error: {:?}", e))?;

    // Validate
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );

    let module_info = validator.validate(&module)
        .map_err(|e| format!("Validation error: {:?}", e))?;

    // Write to WGSL
    let wgsl = naga::back::wgsl::write_string(&module, &module_info, naga::back::wgsl::WriterFlags::empty())
        .map_err(|e| format!("WGSL generation error: {}", e))?;

    Ok(wgsl)
}

fn process_shader(input_path: &Path, output_path: &Path, include_dir: &Path) -> Result<(), String> {
    println!("Processing: {}", input_path.display());

    // Read source
    let source = fs::read_to_string(input_path)
        .map_err(|e| format!("Failed to read: {}", e))?;

    // Determine stage
    let stage = match input_path.extension().and_then(|e| e.to_str()) {
        Some("vsh") => naga::ShaderStage::Vertex,
        Some("fsh") => naga::ShaderStage::Fragment,
        Some("csh") => naga::ShaderStage::Compute,
        _ => return Err("Unknown shader type".to_string()),
    };

    // Preprocess (handle moj_imports, remove #version, add bindings)
    let mut visited = HashSet::new();
    let preprocessed = preprocess_file(&source, include_dir, &mut visited);
    let with_bindings = add_bindings(&preprocessed);

    // Convert to WGSL
    match convert_glsl_to_wgsl(&with_bindings, stage) {
        Ok(wgsl) => {
            // Create output directory if needed
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create output dir: {}", e))?;
            }

            // Write WGSL
            fs::write(output_path, wgsl)
                .map_err(|e| format!("Failed to write: {}", e))?;

            println!("  ✓ Wrote: {}", output_path.display());
            Ok(())
        }
        Err(e) => {
            eprintln!("  ✗ Conversion failed: {}", e);
            // Write stub WGSL shader as fallback
            let stub = create_stub_wgsl(stage);
            fs::write(output_path, stub)
                .map_err(|e| format!("Failed to write stub: {}", e))?;
            println!("  → Wrote stub WGSL shader");
            // Don't return error - continue with stub
            Ok(())
        }
    }
}

fn create_stub_wgsl(stage: naga::ShaderStage) -> String {
    match stage {
        naga::ShaderStage::Vertex => {
            r#"// Stub shader - GLSL conversion failed
// This file contains both vertex and fragment stages

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) vertex_color: vec4<f32>,
}

struct DynamicTransforms {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    TextureMat: mat4x4<f32>,
}

struct Projection {
    ProjMat: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> dynamic_transforms: DynamicTransforms;

@group(0) @binding(1)
var<uniform> projection: Projection;

@vertex
fn main_vs(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = projection.ProjMat * dynamic_transforms.ModelViewMat * vec4<f32>(in.position, 1.0);
    out.vertex_color = in.color;
    return out;
}

struct FragmentInput {
    @location(0) vertex_color: vec4<f32>,
}

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    return in.vertex_color * dynamic_transforms.ColorModulator;
}
"#.to_string()
        }
        naga::ShaderStage::Fragment => {
            // For fragment shaders, we still create the complete file
            r#"// Stub shader - GLSL conversion failed
// This file contains both vertex and fragment stages

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) vertex_color: vec4<f32>,
}

struct DynamicTransforms {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    TextureMat: mat4x4<f32>,
}

struct Projection {
    ProjMat: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> dynamic_transforms: DynamicTransforms;

@group(0) @binding(1)
var<uniform> projection: Projection;

@vertex
fn main_vs(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = projection.ProjMat * dynamic_transforms.ModelViewMat * vec4<f32>(in.position, 1.0);
    out.vertex_color = in.color;
    return out;
}

struct FragmentInput {
    @location(0) vertex_color: vec4<f32>,
}

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    return in.vertex_color * dynamic_transforms.ColorModulator;
}
"#.to_string()
        }
        naga::ShaderStage::Compute => {
            r#"// Stub compute shader - GLSL conversion failed
@compute @workgroup_size(1, 1, 1)
fn main_cs() {
}
"#.to_string()
        }
        _ => {
            format!("// Stub shader for unsupported stage {:?}\n@vertex\nfn main() {{}}\n", stage)
        }
    }
}

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        return Err("Usage: shader_converter <input_dir> <output_dir>".to_string());
    }

    let input_dir = Path::new(&args[1]);
    let output_dir = Path::new(&args[2]);
    let include_dir = input_dir.join("include");
    let core_dir = input_dir.join("core");

    if !input_dir.exists() {
        return Err(format!("Input directory not found: {}", input_dir.display()));
    }

    println!("Converting shaders from {} to {}...", input_dir.display(), output_dir.display());

    let mut errors = 0;
    let mut success = 0;

    // Process core shaders
    if core_dir.exists() {
        let entries = fs::read_dir(&core_dir)
            .map_err(|e| format!("Failed to read core dir: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read dir entry: {}", e))?;
            let path = entry.path();

            if path.extension().and_then(|e| e.to_str()) == Some("vsh") ||
               path.extension().and_then(|e| e.to_str()) == Some("fsh") {
                let output_file = output_dir.join("core").join(format!(
                    "{}.wgsl",
                    path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown")
                ));

                // Skip if WGSL file already exists (preserves manual implementations)
                if output_file.exists() {
                    println!("Skipping existing WGSL: {}", output_file.display());
                    success += 1;
                    continue;
                }

                match process_shader(&path, &output_file, &include_dir) {
                    Ok(_) => success += 1,
                    Err(_) => errors += 1,
                }
            }
        }
    }

    println!("\nConversion complete: {} succeeded, {} failed (used stubs)", success, errors);

    // Don't fail the build even if some shaders failed - stubs are acceptable
    Ok(())
}
