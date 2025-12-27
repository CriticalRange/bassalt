//! Render pipeline management

/// Descriptor for creating a render pipeline
#[derive(Debug, Clone)]
pub struct RenderPipelineDescriptor {
    pub vertex_shader: String,
    pub fragment_shader: Option<String>,
    pub vertex_format: u32,
    pub primitive_topology: u32,
    pub depth_test_enabled: bool,
    pub depth_write_enabled: bool,
    pub depth_compare: u32,
    pub blend_enabled: bool,
    pub blend_color_factor: u32,
    pub blend_alpha_factor: u32,
}

impl Default for RenderPipelineDescriptor {
    fn default() -> Self {
        Self {
            vertex_shader: String::new(),
            fragment_shader: None,
            vertex_format: 0,
            primitive_topology: 3, // TriangleList
            depth_test_enabled: false,
            depth_write_enabled: false,
            depth_compare: 1, // Less
            blend_enabled: false,
            blend_color_factor: 1, // One
            blend_alpha_factor: 0, // Zero
        }
    }
}
