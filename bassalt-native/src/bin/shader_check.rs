//! Shader Validation Test Binary
//!
//! Validates WGSL shaders and optionally compares against Minecraft's GLSL source
//!
//! Usage:
//!   cargo run --bin shader_check                           # Validate all WGSL shaders
//!   cargo run --bin shader_check -- --mc-source ~/source   # Compare against MC source
//!   cargo run --bin shader_check -- --wgsl <path>          # Custom WGSL path
//!   cargo run --bin shader_check -- --filter <pattern>     # Filter shaders by name

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::{HashMap, HashSet};

use naga::{Module, ShaderStage};

// ANSI colors for output
const ANSI_RESET: &str = "\x1b[0m";
const ANSI_RED: &str = "\x1b[31m";
const ANSI_GREEN: &str = "\x1b[32m";
const ANSI_YELLOW: &str = "\x1b[33m";
const ANSI_CYAN: &str = "\x1b[36m";
const ANSI_BOLD: &str = "\x1b[1m";

// ============================================================================
// Reflection Types
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct BindingInfo {
    pub binding: u32,
    pub group: u32,
    pub name: String,
    pub resource_type: ResourceType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    Texture,
    Sampler,
    UniformBuffer,
    StorageBuffer,
}

#[derive(Debug, Clone)]
pub struct UniformStructInfo {
    pub name: String,
    pub size: u32,
    pub members: Vec<String>, // Simplified: just field names
}

#[derive(Debug, Clone)]
pub struct VertexAttributeInfo {
    pub location: u32,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ShaderReflectionInfo {
    pub module_name: String,
    pub stage: ShaderStage,
    pub bindings: Vec<BindingInfo>,
    pub uniform_structs: Vec<UniformStructInfo>,
    pub vertex_attributes: Vec<VertexAttributeInfo>,
}

impl ShaderReflectionInfo {
    pub fn new(module_name: String, stage: ShaderStage) -> Self {
        Self {
            module_name,
            stage,
            bindings: Vec::new(),
            uniform_structs: Vec::new(),
            vertex_attributes: Vec::new(),
        }
    }

    pub fn get_bindings_sorted(&self) -> Vec<&BindingInfo> {
        let mut bindings: Vec<_> = self.bindings.iter().collect();
        bindings.sort_by_key(|b| (b.group, b.binding));
        bindings
    }
}

#[derive(Debug, Clone)]
pub enum ComparisonIssue {
    MissingBinding { slot: u32, wgsl_type: String },
    ExtraBinding { slot: u32, wgsl_type: String },
    TypeMismatch { slot: u32, wgsl: String, glsl: String },
    MissingUniform { name: String },
    UniformSizeMismatch { name: String, wgsl: u32, glsl: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueSeverity {
    Warning,
    Error,
}

impl ComparisonIssue {
    pub fn severity(&self) -> IssueSeverity {
        match self {
            ComparisonIssue::MissingBinding { .. } => IssueSeverity::Error,
            ComparisonIssue::ExtraBinding { .. } => IssueSeverity::Warning,
            ComparisonIssue::TypeMismatch { .. } => IssueSeverity::Error,
            ComparisonIssue::MissingUniform { .. } => IssueSeverity::Warning,
            ComparisonIssue::UniformSizeMismatch { .. } => IssueSeverity::Error,
        }
    }

    pub fn description(&self) -> String {
        match self {
            ComparisonIssue::MissingBinding { slot, wgsl_type } => {
                format!("WGSL missing binding at slot {} (has {})", slot, wgsl_type)
            }
            ComparisonIssue::ExtraBinding { slot, wgsl_type } => {
                format!("WGSL has extra binding at slot {} ({} not in GLSL)", slot, wgsl_type)
            }
            ComparisonIssue::TypeMismatch { slot, wgsl, glsl } => {
                format!("Binding slot {} type mismatch: WGSL={}, GLSL={}", slot, wgsl, glsl)
            }
            ComparisonIssue::MissingUniform { name } => {
                format!("WGSL missing uniform struct '{}'", name)
            }
            ComparisonIssue::UniformSizeMismatch { name, wgsl, glsl } => {
                format!("Uniform '{}' size: WGSL={} bytes, GLSL={} bytes", name, wgsl, glsl)
            }
        }
    }
}

pub struct ComparisonReport {
    pub shader_name: String,
    pub issues: Vec<ComparisonIssue>,
}

// ============================================================================
// Shader Check Types
// ============================================================================

struct ShaderCheckConfig {
    wgsl_dir: PathBuf,
    mc_source_dir: Option<PathBuf>,
    filter: Option<String>,
    verbose: bool,
}

struct ShaderFile {
    name: String,
    content: String,
    stage: naga::ShaderStage,
}

struct ValidationResult {
    shader_name: String,
    stage: String,
    wgsl_result: ParseResult,
    glsl_result: Option<ParseResult>,
    comparison: Option<ComparisonReport>,
}

enum ParseResult {
    Success(ShaderReflectionInfo),
    ParseError(String),
    ValidationError(String),
}

impl ParseResult {
    fn is_success(&self) -> bool {
        matches!(self, Self::Success(_))
    }

    fn get_info(&self) -> Option<&ShaderReflectionInfo> {
        match self {
            Self::Success(info) => Some(info),
            _ => None,
        }
    }
}

// ============================================================================
// Main Program
// ============================================================================

fn main() {
    let config = parse_args();

    println!("{}Bassalt Shader Validation Tool{}", ANSI_BOLD, ANSI_RESET);
    println!("{}\n", "=".repeat(50));

    let shaders = collect_shaders(&config);
    if shaders.is_empty() {
        println!("{}No shaders found!{}", ANSI_YELLOW, ANSI_RESET);
        println!("WGSL directory: {}", config.wgsl_dir.display());
        return;
    }

    println!("Found {} WGSL shaders to validate\n", shaders.len());

    let mut results = Vec::new();

    // For each WGSL shader
    for shader in &shaders {
        let wgsl_result = parse_wgsl(&shader.content, &shader.name, shader.stage);

        // Try to find corresponding GLSL
        let glsl_result = if let Some(ref mc_dir) = config.mc_source_dir {
            if let Some((_glsl_path, glsl_content)) = find_glsl(mc_dir, &shader.name, shader.stage) {
                Some(parse_glsl(&glsl_content, &shader.name, shader.stage))
            } else {
                None
            }
        } else {
            None
        };

        // Compare if both parsed successfully
        let comparison = match (&wgsl_result, &glsl_result) {
            (ParseResult::Success(wgsl_info), Some(ParseResult::Success(glsl_info))) => {
                Some(compare_shaders(wgsl_info, glsl_info))
            }
            _ => None,
        };

        results.push(ValidationResult {
            shader_name: shader.name.clone(),
            stage: format!("{:?}", shader.stage),
            wgsl_result,
            glsl_result,
            comparison,
        });
    }

    generate_report(&results, &config);

    let error_count = results.iter()
        .filter(|r| !r.wgsl_result.is_success())
        .count();

    if error_count > 0 {
        println!("\n{}{} shader(s) failed validation{}", ANSI_RED, error_count, ANSI_RESET);
        std::process::exit(1);
    }
}

fn parse_args() -> ShaderCheckConfig {
    let args: Vec<String> = env::args().collect();

    let mut wgsl_dir = PathBuf::from("src/main/resources/shaders/wgsl");
    let mut mc_source_dir = None;
    let mut filter = None;
    let mut verbose = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--wgsl" | "-w" => {
                i += 1;
                if i < args.len() {
                    wgsl_dir = PathBuf::from(&args[i]);
                }
            }
            "--mc-source" | "-m" => {
                i += 1;
                if i < args.len() {
                    mc_source_dir = Some(PathBuf::from(&args[i]));
                }
            }
            "--filter" | "-f" => {
                i += 1;
                if i < args.len() {
                    filter = Some(args[i].clone());
                }
            }
            "--verbose" | "-v" => verbose = true,
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }

    ShaderCheckConfig {
        wgsl_dir,
        mc_source_dir,
        filter,
        verbose,
    }
}

fn print_usage() {
    println!("Usage: shader_check [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --wgsl, -w <path>       WGSL shaders directory (default: src/main/resources/shaders/wgsl)");
    println!("  --mc-source, -m <path>  Minecraft source directory for GLSL comparison");
    println!("  --filter, -f <pattern>  Only check shaders matching this pattern");
    println!("  --verbose, -v           Show detailed information");
    println!("  --help, -h              Show this help");
    println!();
    println!("Examples:");
    println!("  cargo run --bin shader_check");
    println!("  cargo run --bin shader_check -- --mc-source ~/source");
    println!("  cargo run --bin shader_check -- --filter entity");
}

fn collect_shaders(config: &ShaderCheckConfig) -> Vec<ShaderFile> {
    let mut shaders = Vec::new();

    for subdir in &["core", "post"] {
        let dir = config.wgsl_dir.join(subdir);
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() { continue; }

                let file_name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                let (stage, ext) = if file_name.ends_with(".vert.wgsl") {
                    (naga::ShaderStage::Vertex, "vert")
                } else if file_name.ends_with(".frag.wgsl") {
                    (naga::ShaderStage::Fragment, "frag")
                } else if file_name.ends_with(".comp.wgsl") {
                    (naga::ShaderStage::Compute, "comp")
                } else {
                    continue;
                };

                let base_name = file_name
                    .strip_suffix(&format!(".{}.wgsl", ext))
                    .unwrap_or(file_name);

                if let Some(ref filter) = config.filter {
                    if !base_name.contains(filter) {
                        continue;
                    }
                }

                if let Ok(content) = fs::read_to_string(&path) {
                    shaders.push(ShaderFile {
                        name: base_name.to_string(),
                        content,
                        stage,
                    });
                }
            }
        }
    }

    shaders.sort_by(|a, b| a.name.cmp(&b.name));
    shaders
}

fn find_glsl(mc_dir: &Path, base_name: &str, stage: naga::ShaderStage) -> Option<(PathBuf, String)> {
    let shader_base = mc_dir.join("assets/minecraft/shaders");

    let ext = match stage {
        naga::ShaderStage::Vertex => "vsh",
        naga::ShaderStage::Fragment => "fsh",
        _ => return None,
    };

    for subdir in &["core", "post"] {
        let glsl_path = shader_base.join(subdir).join(format!("{}.{}", base_name, ext));
        if glsl_path.exists() {
            if let Ok(content) = fs::read_to_string(&glsl_path) {
                return Some((glsl_path, content));
            }
        }
    }

    None
}

fn parse_wgsl(source: &str, name: &str, _stage: naga::ShaderStage) -> ParseResult {
    let module = match naga::front::wgsl::parse_str(source) {
        Ok(m) => m,
        Err(e) => return ParseResult::ParseError(format!("{:?}", e)),
    };

    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );

    if let Err(e) = validator.validate(&module) {
        return ParseResult::ValidationError(format!("{:?}", e));
    }

    match reflect_module(&module, name.to_string()) {
        Ok(info) => ParseResult::Success(info),
        Err(e) => ParseResult::ParseError(e),
    }
}

fn parse_glsl(source: &str, name: &str, stage: naga::ShaderStage) -> ParseResult {
    // Preprocess GLSL
    let preprocessed = preprocess_glsl(source);

    let mut frontend = naga::front::glsl::Frontend::default();
    let options = naga::front::glsl::Options {
        stage,
        defines: Default::default(),
    };

    let module = match frontend.parse(&options, &preprocessed) {
        Ok(m) => m,
        Err(e) => return ParseResult::ParseError(format!("GLSL parse: {:?}", e)),
    };

    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );

    if let Err(e) = validator.validate(&module) {
        return ParseResult::ValidationError(format!("{:?}", e));
    }

    match reflect_module(&module, format!("{} (GLSL)", name)) {
        Ok(info) => ParseResult::Success(info),
        Err(e) => ParseResult::ParseError(e),
    }
}

fn preprocess_glsl(source: &str) -> String {
    let mut result = String::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#version") || trimmed.starts_with("precision ")
            || trimmed.starts_with("#moj_import") || trimmed.starts_with("#if")
            || trimmed.starts_with("#else") || trimmed.starts_with("#endif") {
            continue;
        }
        result.push_str(line);
        result.push('\n');
    }
    result
}

fn reflect_module(module: &Module, module_name: String) -> Result<ShaderReflectionInfo, String> {
    let entry = module.entry_points.first()
        .ok_or("No entry point")?;

    let mut info = ShaderReflectionInfo::new(module_name, entry.stage);

    // Collect bindings from global variables
    for (_handle, var) in module.global_variables.iter() {
        if let Some(binding) = &var.binding {
            let ty = &module.types[var.ty];

            let resource_type = match &ty.inner {
                naga::TypeInner::Image { .. } => ResourceType::Texture,
                naga::TypeInner::Sampler { .. } => ResourceType::Sampler,
                naga::TypeInner::Struct { .. } => {
                    match var.space {
                        naga::AddressSpace::Uniform => ResourceType::UniformBuffer,
                        naga::AddressSpace::Storage { .. } => ResourceType::StorageBuffer,
                        _ => continue,
                    }
                }
                _ => continue,
            };

            info.bindings.push(BindingInfo {
                binding: binding.binding,
                group: binding.group,
                name: var.name.clone().unwrap_or_default(),
                resource_type,
            });
        }
    }

    // Collect uniform structs
    for (_handle, ty) in module.types.iter() {
        if let naga::TypeInner::Struct { members, span } = &ty.inner {
            if let Some(name) = &ty.name {
                let field_names: Vec<String> = members.iter()
                    .filter_map(|m| m.name.clone())
                    .collect();

                info.uniform_structs.push(UniformStructInfo {
                    name: name.clone(),
                    size: *span as u32,
                    members: field_names,
                });
            }
        }
    }

    // Collect vertex attributes from entry point arguments
    for arg in &entry.function.arguments {
        if let Some(naga::Binding::Location { location, .. }) = &arg.binding {
            info.vertex_attributes.push(VertexAttributeInfo {
                location: *location,
                name: arg.name.clone().unwrap_or_default(),
            });
        }
    }

    Ok(info)
}

fn compare_shaders(wgsl: &ShaderReflectionInfo, glsl: &ShaderReflectionInfo) -> ComparisonReport {
    let mut issues = Vec::new();

    let wgsl_bindings: HashMap<u32, &BindingInfo> = wgsl.bindings.iter()
        .map(|b| (b.binding, b))
        .collect();

    let glsl_bindings: HashMap<u32, &BindingInfo> = glsl.bindings.iter()
        .map(|b| (b.binding, b))
        .collect();

    // Check for missing or mismatched bindings
    for (&slot, glsl_b) in &glsl_bindings {
        if let Some(wgsl_b) = wgsl_bindings.get(&slot) {
            if wgsl_b.resource_type != glsl_b.resource_type {
                issues.push(ComparisonIssue::TypeMismatch {
                    slot,
                    wgsl: format!("{:?}", wgsl_b.resource_type),
                    glsl: format!("{:?}", glsl_b.resource_type),
                });
            }
        } else {
            issues.push(ComparisonIssue::MissingBinding {
                slot,
                wgsl_type: format!("{:?}", glsl_b.resource_type),
            });
        }
    }

    for (&slot, wgsl_b) in &wgsl_bindings {
        if !glsl_bindings.contains_key(&slot) {
            issues.push(ComparisonIssue::ExtraBinding {
                slot,
                wgsl_type: format!("{:?}", wgsl_b.resource_type),
            });
        }
    }

    // Compare uniform structs
    let wgsl_structs: HashMap<&str, &UniformStructInfo> = wgsl.uniform_structs.iter()
        .map(|s| (s.name.as_str(), s))
        .collect();

    let glsl_structs: HashMap<&str, &UniformStructInfo> = glsl.uniform_structs.iter()
        .map(|s| (s.name.as_str(), s))
        .collect();

    for (&name, glsl_s) in &glsl_structs {
        if let Some(wgsl_s) = wgsl_structs.get(name) {
            if wgsl_s.size != glsl_s.size {
                issues.push(ComparisonIssue::UniformSizeMismatch {
                    name: name.to_string(),
                    wgsl: wgsl_s.size,
                    glsl: glsl_s.size,
                });
            }
        } else {
            issues.push(ComparisonIssue::MissingUniform { name: name.to_string() });
        }
    }

    ComparisonReport {
        shader_name: wgsl.module_name.clone(),
        issues,
    }
}

fn generate_report(results: &[ValidationResult], config: &ShaderCheckConfig) {
    println!();
    println!("{}Validation Report{}", ANSI_BOLD, ANSI_RESET);
    println!("{}", "=".repeat(50));

    let mut total = 0;
    let mut clean = 0;
    let mut wgsl_errors = 0;
    let mut glsl_errors = 0;
    let mut with_issues = 0;

    for result in results {
        total += 1;

        let icon = match result.wgsl_result {
            ParseResult::Success(_) => {
                if result.comparison.as_ref().map_or(true, |c| c.issues.is_empty()) {
                    clean += 1;
                    format!("{}+{}", ANSI_GREEN, ANSI_RESET)
                } else {
                    format!("{}~{}", ANSI_YELLOW, ANSI_RESET)
                }
            }
            _ => {
                wgsl_errors += 1;
                format!("{}X{}", ANSI_RED, ANSI_RESET)
            }
        };

        println!("{} {} ({}):", icon, result.shader_name, result.stage);

        if let ParseResult::ParseError(ref e) | ParseResult::ValidationError(ref e) = result.wgsl_result {
            println!("  {}WGSL Error:{} {}", ANSI_RED, ANSI_RESET, e);
        } else if config.verbose {
            if let Some(info) = result.wgsl_result.get_info() {
                print_shader_info(info);
            }
        }

        if let Some(ref glsl_result) = result.glsl_result {
            if let ParseResult::ParseError(ref e) | ParseResult::ValidationError(ref e) = glsl_result {
                glsl_errors += 1;
                println!("  {}GLSL Error:{} {}", ANSI_YELLOW, ANSI_RESET, e);
            }
        }

        if let Some(ref comp) = result.comparison {
            if !comp.issues.is_empty() {
                with_issues += 1;
                for issue in &comp.issues {
                    let severity = issue.severity();
                    let color = match severity {
                        IssueSeverity::Error => ANSI_RED,
                        IssueSeverity::Warning => ANSI_YELLOW,
                    };
                    println!("  {}{}:{} {}", color, format!("{:?}", severity), ANSI_RESET, issue.description());
                }
            }
        }
    }

    println!();
    println!("{}Summary{}", ANSI_BOLD, ANSI_RESET);
    println!("{}", "-".repeat(50));
    println!("Total: {}", total);
    println!("{}Clean: {}{}", ANSI_GREEN, clean, ANSI_RESET);

    if wgsl_errors > 0 {
        println!("{}WGSL Errors: {}{}", ANSI_RED, wgsl_errors, ANSI_RESET);
    }
    if glsl_errors > 0 {
        println!("{}GLSL Errors: {}{}", ANSI_YELLOW, glsl_errors, ANSI_RESET);
    }
    if with_issues > 0 {
        println!("{}With comparison issues: {}{}", ANSI_YELLOW, with_issues, ANSI_RESET);
    }

    if config.verbose {
        print_detailed_analysis(results);
    }
}

fn print_shader_info(info: &ShaderReflectionInfo) {
    println!("  {}Bindings:{}", ANSI_CYAN, ANSI_RESET);
    for binding in info.get_bindings_sorted() {
        println!("    [{}:{}] {} {:?}", binding.group, binding.binding,
                 format_resource_type(&binding.resource_type), binding.name);
    }

    if !info.uniform_structs.is_empty() {
        println!("  {}Uniform structs:{}", ANSI_CYAN, ANSI_RESET);
        for s in &info.uniform_structs {
            println!("    {} ({} bytes): {}", s.name, s.size, s.members.join(", "));
        }
    }

    if !info.vertex_attributes.is_empty() {
        println!("  {}Vertex attributes:{}", ANSI_CYAN, ANSI_RESET);
        for attr in &info.vertex_attributes {
            println!("    @location({}) {}", attr.location, attr.name);
        }
    }
}

fn format_resource_type(ty: &ResourceType) -> &'static str {
    match ty {
        ResourceType::Texture => "Texture",
        ResourceType::Sampler => "Sampler",
        ResourceType::UniformBuffer => "Uniform",
        ResourceType::StorageBuffer => "Storage",
    }
}

fn print_detailed_analysis(results: &[ValidationResult]) {
    println!();
    println!("{}Detailed Analysis{}", ANSI_BOLD, ANSI_RESET);
    println!("{}", "=".repeat(50));

    let mut all_bindings: HashSet<(u32, u32)> = HashSet::new();
    let mut binding_usage: HashMap<(u32, u32), Vec<String>> = HashMap::new();

    for result in results {
        if let Some(info) = result.wgsl_result.get_info() {
            for binding in &info.bindings {
                all_bindings.insert((binding.group, binding.binding));
                binding_usage.entry((binding.group, binding.binding))
                    .or_insert_with(Vec::new)
                    .push(result.shader_name.clone());
            }
        }
    }

    println!();
    println!("{}Common Binding Slots{}", ANSI_CYAN, ANSI_RESET);
    let mut sorted: Vec<_> = all_bindings.iter().collect();
    sorted.sort();

    for (group, binding) in sorted {
        let shaders = binding_usage.get(&(*group, *binding))
            .map(|v| v.join(", "))
            .unwrap_or_default();

        let count = binding_usage.get(&(*group, *binding)).map(|v| v.len()).unwrap_or(0);
        let display = if shaders.len() > 60 {
            format!("...(& {} shaders)", count)
        } else {
            shaders
        };
        println!("  [{}:{}] used in: {}", group, binding, display);
    }

    let mut all_structs: HashSet<String> = HashSet::new();
    let mut struct_usage: HashMap<String, Vec<String>> = HashMap::new();

    for result in results {
        if let Some(info) = result.wgsl_result.get_info() {
            for s in &info.uniform_structs {
                all_structs.insert(s.name.clone());
                struct_usage.entry(s.name.clone())
                    .or_insert_with(Vec::new)
                    .push(result.shader_name.clone());
            }
        }
    }

    println!();
    println!("{}Common Uniform Structs{}", ANSI_CYAN, ANSI_RESET);
    let mut sorted: Vec<_> = all_structs.iter().collect();
    sorted.sort();

    for name in sorted {
        let shaders = struct_usage.get(name)
            .map(|v| v.join(", "))
            .unwrap_or_default();

        let count = struct_usage.get(name).map(|v| v.len()).unwrap_or(0);
        let display = if shaders.len() > 60 {
            format!("...(& {} shaders)", count)
        } else {
            shaders
        };
        println!("  {} used in: {}", name, display);
    }
}
