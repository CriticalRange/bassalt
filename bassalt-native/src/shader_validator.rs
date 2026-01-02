//! Comprehensive shader validation and analysis
//!
//! Provides detailed shader quality checks beyond basic syntax validation:
//! - Complexity metrics
//! - Resource usage analysis
//! - Performance issue detection

use crate::error::{BasaltError, Result};
use naga::{Module, valid};

/// Detailed validation report for a shader
#[derive(Debug, Clone)]
pub struct ShaderValidationReport {
    /// Shader name/identifier
    pub shader_name: String,

    /// Basic validation passed
    pub is_valid: bool,

    /// Warnings found (non-fatal issues)
    pub warnings: Vec<String>,

    /// Metrics about the shader
    pub metrics: ShaderMetrics,
}

/// Metrics about shader complexity and resource usage
#[derive(Debug, Clone, Default)]
pub struct ShaderMetrics {
    /// Number of functions
    pub function_count: usize,

    /// Number of global variables
    pub global_count: usize,

    /// Number of entry points
    pub entry_point_count: usize,

    /// Total instruction count (approximate)
    pub instruction_count: usize,

    /// Control flow complexity score
    pub complexity_score: u32,
}

/// Enhanced shader validator
pub struct ShaderValidator {
    /// Complexity threshold for warnings
    complexity_threshold: u32,
}

impl ShaderValidator {
    /// Create a new shader validator with default settings
    pub fn new() -> Self {
        Self {
            complexity_threshold: 1000,
        }
    }

    /// Validate a shader module and generate detailed report
    pub fn validate_and_analyze(&self, module: &Module, shader_name: &str) -> Result<ShaderValidationReport> {
        let mut warnings = Vec::new();

        // Step 1: Basic validation
        let _module_info = self.basic_validate(module, shader_name)?;

        // Step 2: Compute metrics
        let metrics = self.compute_metrics(module);

        // Step 3: Check complexity
        self.check_complexity(&metrics, &mut warnings);

        // Step 4: Detect potential issues
        self.detect_issues(module, &metrics, &mut warnings);

        Ok(ShaderValidationReport {
            shader_name: shader_name.to_string(),
            is_valid: true,
            warnings,
            metrics,
        })
    }

    /// Perform basic naga validation
    fn basic_validate(&self, module: &Module, shader_name: &str) -> Result<valid::ModuleInfo> {
        let mut validator = valid::Validator::new(
            valid::ValidationFlags::all(),  // Enable all validation passes
            valid::Capabilities::all(),     // All capabilities
        );

        validator.validate(module).map_err(|e| BasaltError::ShaderValidation {
            shader_name: shader_name.to_string(),
            error: format!("Validation failed: {:?}", e),
        })
    }

    /// Compute shader metrics
    fn compute_metrics(&self, module: &Module) -> ShaderMetrics {
        ShaderMetrics {
            function_count: module.functions.len(),
            global_count: module.global_variables.len(),
            entry_point_count: module.entry_points.len(),
            instruction_count: module.functions.iter()
                .map(|(_, f)| f.body.len())
                .sum(),
            complexity_score: self.compute_complexity_score(module),
        }
    }

    /// Check shader complexity
    fn check_complexity(&self, metrics: &ShaderMetrics, warnings: &mut Vec<String>) {
        if metrics.complexity_score > self.complexity_threshold {
            warnings.push(format!(
                "High complexity score: {} (threshold: {})",
                metrics.complexity_score, self.complexity_threshold
            ));
        }

        if metrics.function_count > 20 {
            warnings.push(format!("Many functions: {}", metrics.function_count));
        }

        if metrics.instruction_count > 1000 {
            warnings.push(format!("Large shader: {} instructions", metrics.instruction_count));
        }
    }

    /// Detect common shader issues
    fn detect_issues(&self, module: &Module, metrics: &ShaderMetrics, warnings: &mut Vec<String>) {
        // Check for high global variable count
        if metrics.global_count > 32 {
            warnings.push(format!(
                "Many global variables: {} (may impact performance)",
                metrics.global_count
            ));
        }

        // Check for very large functions
        for (_, function) in module.functions.iter() {
            if function.body.len() > 200 {
                if let Some(name) = &function.name {
                    warnings.push(format!(
                        "Large function '{}' has {} statements",
                        name,
                        function.body.len()
                    ));
                }
            }
        }

        // Check entry point counts
        if metrics.entry_point_count > 4 {
            warnings.push(format!(
                "Many entry points: {} (unusual for single shader)",
                metrics.entry_point_count
            ));
        }
    }

    /// Compute a complexity score for the shader
    fn compute_complexity_score(&self, module: &Module) -> u32 {
        let mut score = 0u32;

        for (_, function) in module.functions.iter() {
            // Base score per function
            score += 10;

            // Score based on function size
            score += function.body.len() as u32;

            // Count control flow statements (simplified)
            for statement in function.body.iter() {
                match statement {
                    naga::Statement::If { .. } => score += 10,
                    naga::Statement::Loop { .. } => score += 20,
                    naga::Statement::Switch { .. } => score += 15,
                    naga::Statement::Return { .. } => score += 1,
                    naga::Statement::Kill => score += 5,
                    naga::Statement::Call { .. } => score += 5,
                    _ => {}
                }
            }
        }

        score
    }
}

impl Default for ShaderValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function to validate a shader and get a report
pub fn validate_shader(module: &Module, shader_name: &str) -> Result<ShaderValidationReport> {
    ShaderValidator::new().validate_and_analyze(module, shader_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_creation() {
        let validator = ShaderValidator::new();
        assert_eq!(validator.complexity_threshold, 1000);
    }
}
