//! Shader module types.

/// Source code for a shader module.
#[derive(Clone, Debug)]
pub enum ShaderSource<'a> {
    /// WGSL source code.
    Wgsl(&'a str),
    /// Pre-compiled SPIR-V binary (requires backend support).
    SpirV(&'a [u32]),
}

/// Describes a shader module to be created.
#[derive(Clone, Debug)]
pub struct ShaderModuleDescriptor<'a> {
    /// Debug label.
    pub label: Option<&'a str>,
    /// Shader source code.
    pub source: ShaderSource<'a>,
}
