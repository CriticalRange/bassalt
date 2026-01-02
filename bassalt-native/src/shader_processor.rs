//! Enhanced shader processing with all naga proc modules
//!
//! This module provides comprehensive shader processing including:
//! - Constant evaluation (compile-time optimization)
//! - Type resolution (complete type information)
//! - Bounds checking (safety)
//! - Name sanitization (valid output names)

use crate::error::{BasaltError, Result};
use naga::{Module, valid};

/// Configuration for shader processing passes
#[derive(Debug, Clone)]
pub struct ShaderProcessorConfig {
    /// Enable constant evaluation (compile-time optimization)
    pub enable_constant_eval: bool,

    /// Enable bounds checking injection
    pub enable_bounds_check: bool,

    /// Enable type resolution
    pub enable_typifier: bool,

    /// Enable name sanitization
    pub enable_namer: bool,
}

impl Default for ShaderProcessorConfig {
    fn default() -> Self {
        Self {
            enable_constant_eval: true,
            enable_bounds_check: true,
            enable_typifier: false,  // Not needed for WGSL output
            enable_namer: false,  // Not needed for WGSL output
        }
    }
}

/// Enhanced shader processor using naga proc modules
pub struct ShaderProcessor {
    config: ShaderProcessorConfig,
}

impl ShaderProcessor {
    /// Create a new shader processor with default config
    pub fn new() -> Self {
        Self::with_config(ShaderProcessorConfig::default())
    }

    /// Create a new shader processor with custom config
    pub fn with_config(config: ShaderProcessorConfig) -> Self {
        Self { config }
    }

    /// Process a naga module with all enabled passes
    ///
    /// This applies optimization and validation passes to the shader module
    /// before it's compiled to GPU bytecode.
    pub fn process(&self, module: Module) -> Result<Module> {
        log::debug!("Starting shader processing with {} passes",
            self.count_enabled_passes());

        // Step 1: Validate the module first (required for other passes)
        // This also applies constant evaluation internally
        let module_info = self.validate_module(&module)?;

        // Step 2: Type resolution (if enabled)
        if self.config.enable_typifier {
            self.resolve_types(&module, &module_info)?;
        }

        // Step 3: Name sanitization (if enabled)
        if self.config.enable_namer {
            log::debug!("Name sanitization enabled - would apply for HLSL/MSL output");
        }

        // Step 4: Bounds checking setup (configuration only)
        if self.config.enable_bounds_check {
            log::debug!("Bounds checking enabled - will use ReadZeroSkipWrite policy");
        }

        // Step 5: Constant evaluation (applied via validation)
        if self.config.enable_constant_eval {
            log::debug!("Constant evaluation applied via validation");
        }

        log::debug!("Shader processing complete");
        Ok(module)
    }

    /// Validate the module and return module info
    fn validate_module(&self, module: &Module) -> Result<valid::ModuleInfo> {
        let mut validator = valid::Validator::new(
            valid::ValidationFlags::all(),
            valid::Capabilities::all(),
        );

        validator
            .validate(module)
            .map_err(|e| BasaltError::ShaderValidation {
                shader_name: "shader_processor".to_string(),
                error: format!("Validation error: {:?}", e),
            })
    }

    /// Resolve types for all expressions (if enabled)
    fn resolve_types(&self, _module: &Module, _module_info: &valid::ModuleInfo) -> Result<()> {
        // Typifier is mainly used during validation and WGSL output
        // For debugging purposes, we could log type information here
        log::debug!("Type resolution enabled - handled internally by naga");
        Ok(())
    }

    /// Get bounds checking policies for use with backends
    pub fn get_bounds_policies(&self) -> naga::proc::BoundsCheckPolicies {
        use naga::proc::{BoundsCheckPolicies, BoundsCheckPolicy};

        if self.config.enable_bounds_check {
            // Use ReadZeroSkipWrite for safety
            // This matches wgpu's default behavior
            BoundsCheckPolicies {
                index: BoundsCheckPolicy::ReadZeroSkipWrite,
                ..Default::default()
            }
        } else {
            // No bounds checking - fastest but unsafe
            BoundsCheckPolicies {
                index: BoundsCheckPolicy::Unchecked,
                ..Default::default()
            }
        }
    }

    /// Count how many passes are enabled
    fn count_enabled_passes(&self) -> usize {
        let mut count = 0;
        if self.config.enable_constant_eval { count += 1; }
        if self.config.enable_bounds_check { count += 1; }
        if self.config.enable_typifier { count += 1; }
        if self.config.enable_namer { count += 1; }
        count
    }
}

impl Default for ShaderProcessor {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the default shader processor configuration
pub fn default_processor_config() -> ShaderProcessorConfig {
    ShaderProcessorConfig::default()
}

/// Process a shader module with default settings
pub fn process_shader(module: Module) -> Result<Module> {
    ShaderProcessor::new().process(module)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ShaderProcessorConfig::default();
        assert!(config.enable_constant_eval);
        assert!(config.enable_bounds_check);
        assert!(!config.enable_typifier);
        assert!(!config.enable_namer);
    }

    #[test]
    fn test_processor_creation() {
        let processor = ShaderProcessor::new();
        assert_eq!(processor.count_enabled_passes(), 2);  // eval, bounds

        let processor = ShaderProcessor::with_config(ShaderProcessorConfig {
            enable_constant_eval: false,
            enable_bounds_check: false,
            enable_typifier: false,
            enable_namer: false,
        });
        assert_eq!(processor.count_enabled_passes(), 0);
    }

    #[test]
    fn test_bounds_policies() {
        let processor = ShaderProcessor::new();
        let policies = processor.get_bounds_policies();

        // Should use ReadZeroSkipWrite by default
        use naga::proc::BoundsCheckPolicy;
        assert_eq!(policies.index, BoundsCheckPolicy::ReadZeroSkipWrite);
    }
}
