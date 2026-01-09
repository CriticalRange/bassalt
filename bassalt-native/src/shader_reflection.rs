//! Shader reflection utilities for validation and testing
//!
//! Provides utilities to extract and compare shader information
//! including bindings, uniforms, vertex attributes, and struct layouts.

use std::collections::HashMap;
use naga::{Module, ShaderStage, Handle, Type, StructMember, Expression, GlobalVariable, StorageQualifier};

/// Information about a single binding (resource)
#[derive(Debug, Clone, PartialEq)]
pub struct BindingInfo {
    /// Binding slot number
    pub binding: u32,
    /// Bind group number (usually 0 in Bassalt)
    pub group: u32,
    /// Variable name in shader
    pub name: String,
    /// Type of resource
    pub resource_type: ResourceType,
    /// For textures: the dimension (2D, Cube, etc.)
    pub dimension: Option<TextureDimension>,
    /// For storage buffers: read/write mode
    pub access: Option<StorageAccess>,
}

/// Type of resource binding
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    Texture,
    Sampler,
    UniformBuffer,
    StorageBuffer { read_only: bool },
}

/// Texture dimension
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureDimension {
    D1,
    D2,
    D2Array,
    D3,
    Cube,
    CubeArray,
}

/// Storage buffer access mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageAccess {
    Read,
    Write,
    ReadWrite,
}

/// Information about a struct member (uniform fields)
#[derive(Debug, Clone)]
pub struct StructMemberInfo {
    pub name: String,
    pub offset: u32,
    pub ty: String,
    pub size: u32,
}

/// Information about a uniform buffer struct
#[derive(Debug, Clone)]
pub struct UniformStructInfo {
    pub name: String,
    pub size: u32,
    pub members: Vec<StructMemberInfo>,
    /// The binding this struct is attached to (if any)
    pub binding: Option<u32>,
}

/// Information about a vertex input
#[derive(Debug, Clone)]
pub struct VertexInputInfo {
    pub location: u32,
    pub name: String,
    pub ty: String,
}

/// Information about a vertex output
#[derive(Debug, Clone)]
pub struct VertexOutputInfo {
    pub location: u32,
    pub name: String,
    pub ty: String,
}

/// Complete reflection info for a shader module
#[derive(Debug, Clone)]
pub struct ShaderReflectionInfo {
    pub module_name: String,
    pub stage: ShaderStage,
    pub bindings: Vec<BindingInfo>,
    pub uniform_structs: Vec<UniformStructInfo>,
    pub vertex_inputs: Vec<VertexInputInfo>,
    pub vertex_outputs: Vec<VertexOutputInfo>,
}

impl ShaderReflectionInfo {
    pub fn new(module_name: String, stage: ShaderStage) -> Self {
        Self {
            module_name,
            stage,
            bindings: Vec::new(),
            uniform_structs: Vec::new(),
            vertex_inputs: Vec::new(),
            vertex_outputs: Vec::new(),
        }
    }

    /// Get bindings in order
    pub fn get_bindings_sorted(&self) -> Vec<&BindingInfo> {
        let mut bindings: Vec<_> = self.bindings.iter().collect();
        bindings.sort_by_key(|b| (b.group, b.binding));
        bindings
    }

    /// Get binding by slot
    pub fn get_binding(&self, binding: u32) -> Option<&BindingInfo> {
        self.bindings.iter().find(|b| b.binding == binding)
    }

    /// Get uniform struct by name
    pub fn get_uniform_struct(&self, name: &str) -> Option<&UniformStructInfo> {
        self.uniform_structs.iter().find(|s| s.name == name)
    }
}

/// Extract reflection information from a parsed naga Module
pub fn reflect_module(module: &Module, module_name: String) -> Result<ShaderReflectionInfo, String> {
    let mut info = ShaderReflectionInfo::new(module_name, ShaderStage::Vertex);

    // Determine shader stage from entry points
    let (entry_point, stage) = module.entry_points.iter()
        .next()
        .ok_or("No entry point found in module")?;

    info.stage = *stage;

    // Collect all types for later lookup
    let types = &module.types;

    // Process global variables (bindings)
    for (handle, var) in module.global_variables.iter() {
        if let Some(binding) = &var.binding {
            // Extract resource type from the type
            let ty = types.get_handle(var.ty)
                .ok_or("Type not found")?;

            let resource_type = match &ty.inner {
                Type::Image {
                    dim, arrayed, class, ..
                } => {
                    ResourceType::Texture
                }
                Type::Sampler { .. } => ResourceType::Sampler,
                Type::Struct { .. } => {
                    // Check if this is a uniform or storage buffer
                    match var.space {
                        naga::AddressSpace::Uniform => ResourceType::UniformBuffer,
                        naga::AddressSpace::Storage { .. } => {
                            let read_only = matches!(var.space, naga::AddressSpace::Storage { read: true });
                            ResourceType::StorageBuffer { read_only }
                        }
                        _ => continue,
                    }
                }
                _ => continue,
            };

            let binding_info = BindingInfo {
                binding: binding.binding,
                group: binding.group,
                name: var.name.clone().unwrap_or_else(|| format!("binding_{}", binding.binding)),
                resource_type,
                dimension: extract_texture_dim(ty),
                access: extract_storage_access(var),
            };

            info.bindings.push(binding_info);
        }
    }

    // Extract uniform struct definitions
    for (handle, ty) in module.types.iter() {
        if let Type::Struct { members, span } = &ty.inner {
            let struct_name = ty.name.clone().unwrap_or_else(|| format!("struct_{}", handle))?;

            let mut member_infos = Vec::new();
            for member in members {
                let member_ty = types.get_handle(member.ty)
                    .ok_or("Member type not found")?;

                let (ty_name, size) = get_type_name_and_size(member_ty, types);

                member_infos.push(StructMemberInfo {
                    name: member.name.clone().unwrap_or_else(|| format!("member_{}", member_infos.len())),
                    offset: member.offset as u32,
                    ty: ty_name,
                    size,
                });
            }

            info.uniform_structs.push(UniformStructInfo {
                name: struct_name.clone(),
                size: span as u32,
                members: member_infos,
                binding: find_binding_for_struct(&info, &struct_name),
            });
        }
    }

    // Extract function IO for vertex inputs/outputs
    if let Some(ep_func) = module.functions.get(entry_point.function) {
        // Look at function arguments for inputs
        for arg in &ep_func.arguments {
            let ty = types.get_handle(arg.ty)
                .ok_or("Argument type not found")?;

            let ty_name = get_type_name_and_size(ty, types).0;

            info.vertex_inputs.push(VertexInputInfo {
                location: arg.binding.as_ref()
                    .and_then(|b| b.location())
                    .unwrap_or(0),
                name: arg.name.clone().unwrap_or_else(|| format!("input_{}", info.vertex_inputs.len())),
                ty: ty_name,
            });
        }

        // Look at function return value for outputs
        if let Some(ref ret_ty) = ep_func.return_type {
            if let Type::Struct { members, .. } = &types.get_handle(*ret_ty).ok_or("Return type not found")?.inner {
                for member in members {
                    let member_ty = types.get_handle(member.ty)
                        .ok_or("Member type not found")?;

                    let ty_name = get_type_name_and_size(member_ty, types).0;

                    if let Some(location) = member.binding.as_ref().and_then(|b| b.location()) {
                        info.vertex_outputs.push(VertexOutputInfo {
                            location,
                            name: member.name.clone().unwrap_or_else(|| format!("output_{}", info.vertex_outputs.len())),
                            ty: ty_name,
                        });
                    }
                }
            }
        }
    }

    Ok(info)
}

fn extract_texture_dim(ty: &Type) -> Option<TextureDimension> {
    match &ty.inner {
        Type::Image { dim, arrayed, .. } => {
            match (dim, arrayed) {
                (naga::ImageDimension::D1, false) => Some(TextureDimension::D1),
                (naga::ImageDimension::D2, false) => Some(TextureDimension::D2),
                (naga::ImageDimension::D2, true) => Some(TextureDimension::D2Array),
                (naga::ImageDimension::D3, false) => Some(TextureDimension::D3),
                (naga::ImageDimension::Cube, false) => Some(TextureDimension::Cube),
                (naga::ImageDimension::Cube, true) => Some(TextureDimension::CubeArray),
                _ => None,
            }
        }
        _ => None,
    }
}

fn extract_storage_access(var: &GlobalVariable) -> Option<StorageAccess> {
    match var.space {
        naga::AddressSpace::Storage { read } => {
            if read {
                Some(StorageAccess::Read)
            } else {
                Some(StorageAccess::ReadWrite)
            }
        }
        _ => None,
    }
}

fn get_type_name_and_size(ty: &Type, types: &naga::UniqueArena<Type>) -> (String, u32) {
    match &ty.inner {
        Type::Scalar { kind, width } => {
            let name = format!("{:?}{}", kind, width);
            let size = width as u32 / 8;
            (name, size)
        }
        Type::Vector { size, kind, width } => {
            let name = format!("vec{}<{:?}{}>", size as u8, kind, width);
            let scalar_size = width as u32 / 8;
            let vec_size = scalar_size * size as u32;
            (name, vec_size)
        }
        Type::Matrix { columns, rows, width, .. } => {
            let name = format!("mat{}x{}<{}>", columns as u8, rows as u8, width);
            let scalar_size = width as u32 / 8;
            let mat_size = scalar_size * columns as u32 * rows as u32;
            (name, mat_size)
        }
        Type::Array { base, size, stride, .. } => {
            let (base_name, base_size) = get_type_name_and_size(types.get_handle(*base).unwrap(), types);
            let count = match size {
                naga::ArraySize::Constant(c) => c.get() as u32,
                naga::ArraySize::Dynamic => 1,
            };
            let total_size = base_size * count;
            (format!("array<{}, {}>", base_name, count), total_size)
        }
        Type::Struct { span, .. } => {
            (format!("struct"), *span as u32)
        }
        _ => ("unknown".to_string(), 0),
    }
}

fn find_binding_for_struct(info: &ShaderReflectionInfo, struct_name: &str) -> Option<u32> {
    // Find a uniform buffer binding that references this struct
    // This requires checking which global variable has this type
    // For now, return None as we'd need more context
    None
}

/// Compare two reflection infos and generate a report
pub struct ComparisonReport {
    pub shader_name: String,
    pub issues: Vec<ComparisonIssue>,
}

#[derive(Debug, Clone)]
pub enum ComparisonIssue {
    MissingBinding { slot: u32, expected_type: String },
    ExtraBinding { slot: u32, found_type: String },
    TypeMismatch { slot: u32, expected: String, found: String },
    MissingUniform { name: String },
    ExtraUniform { name: String },
    UniformSizeMismatch { name: String, expected: u32, found: u32 },
    MissingField { struct_name: String, field_name: String },
    ExtraField { struct_name: String, field_name: String },
    FieldOffsetMismatch { struct_name: String, field_name: String, expected: u32, found: u32 },
    FieldTypeMismatch { struct_name: String, field_name: String, expected: String, found: String },
    MissingVertexInput { location: u32 },
    ExtraVertexInput { location: u32 },
    VertexInputTypeMismatch { location: u32, expected: String, found: String },
}

pub fn compare_reflection_info(
    wgsl_info: &ShaderReflectionInfo,
    glsl_info: &ShaderReflectionInfo,
) -> ComparisonReport {
    let mut issues = Vec::new();

    // Compare bindings
    let wgsl_bindings: HashMap<u32, &BindingInfo> = wgsl_info.bindings.iter()
        .map(|b| (b.binding, b))
        .collect();

    let glsl_bindings: HashMap<u32, &BindingInfo> = glsl_info.bindings.iter()
        .map(|b| (b.binding, b))
        .collect();

    // Check for missing bindings
    for (&slot, glsl_binding) in &glsl_bindings {
        if let Some(wgsl_binding) = wgsl_bindings.get(&slot) {
            // Check type match
            if wgsl_binding.resource_type != glsl_binding.resource_type {
                issues.push(ComparisonIssue::TypeMismatch {
                    slot,
                    expected: format!("{:?}", glsl_binding.resource_type),
                    found: format!("{:?}", wgsl_binding.resource_type),
                });
            }
        } else {
            issues.push(ComparisonIssue::MissingBinding {
                slot,
                expected_type: format!("{:?}", glsl_binding.resource_type),
            });
        }
    }

    // Check for extra bindings
    for (&slot, wgsl_binding) in &wgsl_bindings {
        if !glsl_bindings.contains_key(&slot) {
            issues.push(ComparisonIssue::ExtraBinding {
                slot,
                found_type: format!("{:?}", wgsl_binding.resource_type),
            });
        }
    }

    // Compare uniform structs
    let wgsl_structs: HashMap<String, &UniformStructInfo> = wgsl_info.uniform_structs.iter()
        .map(|s| (s.name.clone(), s))
        .collect();

    let glsl_structs: HashMap<String, &UniformStructInfo> = glsl_info.uniform_structs.iter()
        .map(|s| (s.name.clone(), s))
        .collect();

    for (name, glsl_struct) in &glsl_structs {
        if let Some(wgsl_struct) = wgsl_structs.get(name) {
            // Check size
            if wgsl_struct.size != glsl_struct.size {
                issues.push(ComparisonIssue::UniformSizeMismatch {
                    name: name.clone(),
                    expected: glsl_struct.size,
                    found: wgsl_struct.size,
                });
            }

            // Check fields
            let wgsl_members: HashMap<String, &StructMemberInfo> = wgsl_struct.members.iter()
                .map(|m| (m.name.clone(), m))
                .collect();

            let glsl_members: HashMap<String, &StructMemberInfo> = glsl_struct.members.iter()
                .map(|m| (m.name.clone(), m))
                .collect();

            for (field_name, glsl_field) in &glsl_members {
                if let Some(wgsl_field) = wgsl_members.get(field_name) {
                    if wgsl_field.ty != glsl_field.ty {
                        issues.push(ComparisonIssue::FieldTypeMismatch {
                            struct_name: name.clone(),
                            field_name: field_name.clone(),
                            expected: glsl_field.ty.clone(),
                            found: wgsl_field.ty.clone(),
                        });
                    }
                    if wgsl_field.offset != glsl_field.offset {
                        issues.push(ComparisonIssue::FieldOffsetMismatch {
                            struct_name: name.clone(),
                            field_name: field_name.clone(),
                            expected: glsl_field.offset,
                            found: wgsl_field.offset,
                        });
                    }
                } else {
                    issues.push(ComparisonIssue::MissingField {
                        struct_name: name.clone(),
                        field_name: field_name.clone(),
                    });
                }
            }

            for field_name in wgsl_members.keys() {
                if !glsl_members.contains_key(field_name) {
                    issues.push(ComparisonIssue::ExtraField {
                        struct_name: name.clone(),
                        field_name: field_name.clone(),
                    });
                }
            }
        } else {
            issues.push(ComparisonIssue::MissingUniform { name: name.clone() });
        }
    }

    for name in wgsl_structs.keys() {
        if !glsl_structs.contains_key(name) {
            issues.push(ComparisonIssue::ExtraUniform { name: name.clone() });
        }
    }

    // Compare vertex inputs
    let wgsl_inputs: HashMap<u32, &VertexInputInfo> = wgsl_info.vertex_inputs.iter()
        .map(|i| (i.location, i))
        .collect();

    let glsl_inputs: HashMap<u32, &VertexInputInfo> = glsl_info.vertex_inputs.iter()
        .map(|i| (i.location, i))
        .collect();

    for (&loc, glsl_input) in &glsl_inputs {
        if let Some(wgsl_input) = wgsl_inputs.get(&loc) {
            if wgsl_input.ty != glsl_input.ty {
                issues.push(ComparisonIssue::VertexInputTypeMismatch {
                    location: loc,
                    expected: glsl_input.ty.clone(),
                    found: wgsl_input.ty.clone(),
                });
            }
        } else {
            issues.push(ComparisonIssue::MissingVertexInput { location: loc });
        }
    }

    for (&loc, _) in &wgsl_inputs {
        if !glsl_inputs.contains_key(&loc) {
            issues.push(ComparisonIssue::ExtraVertexInput { location: loc });
        }
    }

    ComparisonReport {
        shader_name: wgsl_info.module_name.clone(),
        issues,
    }
}

impl ComparisonIssue {
    pub fn severity(&self) -> IssueSeverity {
        match self {
            ComparisonIssue::MissingBinding { .. } => IssueSeverity::Error,
            ComparisonIssue::ExtraBinding { .. } => IssueSeverity::Warning,
            ComparisonIssue::TypeMismatch { .. } => IssueSeverity::Error,
            ComparisonIssue::MissingUniform { .. } => IssueSeverity::Warning,
            ComparisonIssue::ExtraUniform { .. } => IssueSeverity::Warning,
            ComparisonIssue::UniformSizeMismatch { .. } => IssueSeverity::Error,
            ComparisonIssue::MissingField { .. } => IssueSeverity::Error,
            ComparisonIssue::ExtraField { .. } => IssueSeverity::Warning,
            ComparisonIssue::FieldOffsetMismatch { .. } => IssueSeverity::Error,
            ComparisonIssue::FieldTypeMismatch { .. } => IssueSeverity::Error,
            ComparisonIssue::MissingVertexInput { .. } => IssueSeverity::Warning,
            ComparisonIssue::ExtraVertexInput { .. } => IssueSeverity::Warning,
            ComparisonIssue::VertexInputTypeMismatch { .. } => IssueSeverity::Error,
        }
    }

    pub fn description(&self) -> String {
        match self {
            ComparisonIssue::MissingBinding { slot, expected_type } => {
                format!("Binding slot {} ({}) missing from WGSL", slot, expected_type)
            }
            ComparisonIssue::ExtraBinding { slot, found_type } => {
                format!("WGSL has extra binding at slot {} ({}) not in GLSL", slot, found_type)
            }
            ComparisonIssue::TypeMismatch { slot, expected, found } => {
                format!("Binding slot {} type mismatch: expected {}, found {}", slot, expected, found)
            }
            ComparisonIssue::MissingUniform { name } => {
                format!("Uniform struct '{}' missing from WGSL", name)
            }
            ComparisonIssue::ExtraUniform { name } => {
                format!("WGSL has extra uniform struct '{}' not in GLSL", name)
            }
            ComparisonIssue::UniformSizeMismatch { name, expected, found } => {
                format!("Uniform '{}' size mismatch: expected {} bytes, found {} bytes", name, expected, found)
            }
            ComparisonIssue::MissingField { struct_name, field_name } => {
                format!("Field '{}' in struct '{}' missing from WGSL", field_name, struct_name)
            }
            ComparisonIssue::ExtraField { struct_name, field_name } => {
                format!("WGSL has extra field '{}' in struct '{}' not in GLSL", field_name, struct_name)
            }
            ComparisonIssue::FieldOffsetMismatch { struct_name, field_name, expected, found } => {
                format!("Field '{}.{}' offset mismatch: expected {}, found {}", struct_name, field_name, expected, found)
            }
            ComparisonIssue::FieldTypeMismatch { struct_name, field_name, expected, found } => {
                format!("Field '{}.{}' type mismatch: expected {}, found {}", struct_name, field_name, expected, found)
            }
            ComparisonIssue::MissingVertexInput { location } => {
                format!("Vertex input at location {} missing from WGSL", location)
            }
            ComparisonIssue::ExtraVertexInput { location } => {
                format!("WGSL has extra vertex input at location {} not in GLSL", location)
            }
            ComparisonIssue::VertexInputTypeMismatch { location, expected, found } => {
                format!("Vertex input at location {} type mismatch: expected {}, found {}", location, expected, found)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IssueSeverity {
    Warning,
    Error,
}
