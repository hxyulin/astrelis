//! Material system for high-level shader parameter management.
//!
//! Provides a declarative API for managing shader parameters, textures, and pipeline state.
//!
//! # Example
//!
//! ```ignore
//! use astrelis_render::*;
//! use glam::{Vec2, Vec3, Mat4};
//!
//! let mut material = Material::new(shader, &renderer);
//!
//! // Set parameters
//! material.set_parameter("color", MaterialParameter::Color(Color::RED));
//! material.set_parameter("time", MaterialParameter::Float(1.5));
//! material.set_parameter("view_proj", MaterialParameter::Matrix4(view_proj_matrix));
//!
//! // Set textures
//! material.set_texture("albedo_texture", texture_handle);
//!
//! // Apply material to render pass
//! material.bind(&mut pass);
//! ```

use crate::{Color, GraphicsContext};
use ahash::HashMap;
use glam::{Mat4, Vec2, Vec3, Vec4};
use std::sync::Arc;

/// A material parameter value that can be bound to a shader.
#[derive(Debug, Clone)]
pub enum MaterialParameter {
    /// Single float value
    Float(f32),
    /// 2D vector
    Vec2(Vec2),
    /// 3D vector
    Vec3(Vec3),
    /// 4D vector
    Vec4(Vec4),
    /// RGBA color
    Color(Color),
    /// 4x4 matrix
    Matrix4(Mat4),
    /// Array of floats
    FloatArray(Vec<f32>),
    /// Array of Vec2
    Vec2Array(Vec<Vec2>),
    /// Array of Vec3
    Vec3Array(Vec<Vec3>),
    /// Array of Vec4
    Vec4Array(Vec<Vec4>),
}

impl MaterialParameter {
    /// Convert parameter to bytes for GPU upload.
    pub fn as_bytes(&self) -> Vec<u8> {
        match self {
            MaterialParameter::Float(v) => bytemuck::bytes_of(v).to_vec(),
            MaterialParameter::Vec2(v) => bytemuck::bytes_of(v).to_vec(),
            MaterialParameter::Vec3(v) => {
                // Pad Vec3 to 16 bytes for alignment
                let mut bytes = Vec::with_capacity(16);
                bytes.extend_from_slice(bytemuck::bytes_of(v));
                bytes.extend_from_slice(&[0u8; 4]); // padding
                bytes
            }
            MaterialParameter::Vec4(v) => bytemuck::bytes_of(v).to_vec(),
            MaterialParameter::Color(c) => bytemuck::bytes_of(c).to_vec(),
            MaterialParameter::Matrix4(m) => bytemuck::bytes_of(m).to_vec(),
            MaterialParameter::FloatArray(arr) => bytemuck::cast_slice(arr).to_vec(),
            MaterialParameter::Vec2Array(arr) => bytemuck::cast_slice(arr).to_vec(),
            MaterialParameter::Vec3Array(arr) => {
                // Each Vec3 needs padding to 16 bytes
                let mut bytes = Vec::with_capacity(arr.len() * 16);
                for v in arr {
                    bytes.extend_from_slice(bytemuck::bytes_of(v));
                    bytes.extend_from_slice(&[0u8; 4]); // padding
                }
                bytes
            }
            MaterialParameter::Vec4Array(arr) => bytemuck::cast_slice(arr).to_vec(),
        }
    }

    /// Get the size of the parameter in bytes (including padding).
    pub fn size(&self) -> u64 {
        match self {
            MaterialParameter::Float(_) => 4,
            MaterialParameter::Vec2(_) => 8,
            MaterialParameter::Vec3(_) => 16, // Padded
            MaterialParameter::Vec4(_) => 16,
            MaterialParameter::Color(_) => 16,
            MaterialParameter::Matrix4(_) => 64,
            MaterialParameter::FloatArray(arr) => (arr.len() * 4) as u64,
            MaterialParameter::Vec2Array(arr) => (arr.len() * 8) as u64,
            MaterialParameter::Vec3Array(arr) => (arr.len() * 16) as u64, // Padded
            MaterialParameter::Vec4Array(arr) => (arr.len() * 16) as u64,
        }
    }
}

/// Texture binding information for a material.
#[derive(Debug, Clone)]
pub struct MaterialTexture {
    /// The texture to bind
    pub texture: wgpu::Texture,
    /// The texture view
    pub view: wgpu::TextureView,
    /// Optional sampler (if None, a default linear sampler will be used)
    pub sampler: Option<wgpu::Sampler>,
}

/// Pipeline state configuration for a material.
#[derive(Debug, Clone)]
pub struct PipelineState {
    /// Primitive topology (default: TriangleList)
    pub topology: wgpu::PrimitiveTopology,
    /// Cull mode (default: Some(Back))
    pub cull_mode: Option<wgpu::Face>,
    /// Front face winding (default: Ccw)
    pub front_face: wgpu::FrontFace,
    /// Polygon mode (default: Fill)
    pub polygon_mode: wgpu::PolygonMode,
    /// Depth test enabled (default: false)
    pub depth_test: bool,
    /// Depth write enabled (default: false)
    pub depth_write: bool,
    /// Blend mode (default: None - opaque)
    pub blend: Option<wgpu::BlendState>,
}

impl Default for PipelineState {
    fn default() -> Self {
        Self {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: Some(wgpu::Face::Back),
            front_face: wgpu::FrontFace::Ccw,
            polygon_mode: wgpu::PolygonMode::Fill,
            depth_test: false,
            depth_write: false,
            blend: None,
        }
    }
}

/// A material manages shader parameters, textures, and pipeline state.
pub struct Material {
    /// The shader module
    shader: Arc<wgpu::ShaderModule>,
    /// Named parameters
    parameters: HashMap<String, MaterialParameter>,
    /// Named textures
    textures: HashMap<String, MaterialTexture>,
    /// Pipeline state
    pipeline_state: PipelineState,
    /// Graphics context reference
    context: Arc<GraphicsContext>,
    /// Cached uniform buffer
    uniform_buffer: Option<wgpu::Buffer>,
    /// Cached bind group layout
    bind_group_layout: Option<wgpu::BindGroupLayout>,
    /// Cached bind group
    bind_group: Option<wgpu::BindGroup>,
    /// Dirty flag - set to true when parameters/textures change
    dirty: bool,
}

impl Material {
    /// Create a new material with a shader.
    pub fn new(shader: Arc<wgpu::ShaderModule>, context: Arc<GraphicsContext>) -> Self {
        Self {
            shader,
            parameters: HashMap::default(),
            textures: HashMap::default(),
            pipeline_state: PipelineState::default(),
            context,
            uniform_buffer: None,
            bind_group_layout: None,
            bind_group: None,
            dirty: true,
        }
    }

    /// Create a material from a shader source string.
    pub fn from_source(
        source: &str,
        label: Option<&str>,
        context: Arc<GraphicsContext>,
    ) -> Self {
        let shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label,
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
        Self::new(Arc::new(shader), context)
    }

    /// Set a parameter by name.
    pub fn set_parameter(&mut self, name: impl Into<String>, value: MaterialParameter) {
        self.parameters.insert(name.into(), value);
        self.dirty = true;
    }

    /// Get a parameter by name.
    pub fn get_parameter(&self, name: &str) -> Option<&MaterialParameter> {
        self.parameters.get(name)
    }

    /// Set a texture by name.
    pub fn set_texture(&mut self, name: impl Into<String>, texture: MaterialTexture) {
        self.textures.insert(name.into(), texture);
        self.dirty = true;
    }

    /// Get a texture by name.
    pub fn get_texture(&self, name: &str) -> Option<&MaterialTexture> {
        self.textures.get(name)
    }

    /// Set the pipeline state.
    pub fn set_pipeline_state(&mut self, state: PipelineState) {
        self.pipeline_state = state;
    }

    /// Get the pipeline state.
    pub fn pipeline_state(&self) -> &PipelineState {
        &self.pipeline_state
    }

    /// Get the shader module.
    pub fn shader(&self) -> &wgpu::ShaderModule {
        &self.shader
    }

    /// Update GPU resources if dirty.
    fn update_resources(&mut self) {
        if !self.dirty {
            return;
        }

        // Calculate total uniform buffer size
        let mut uniform_size = 0u64;
        for param in self.parameters.values() {
            uniform_size += param.size();
            // Add padding for alignment
            if uniform_size % 16 != 0 {
                uniform_size += 16 - (uniform_size % 16);
            }
        }

        // Create or update uniform buffer
        if uniform_size > 0 {
            let mut uniform_data = Vec::new();
            for param in self.parameters.values() {
                uniform_data.extend_from_slice(&param.as_bytes());
                // Add padding for alignment
                let current_size = uniform_data.len() as u64;
                if current_size % 16 != 0 {
                    let padding = 16 - (current_size % 16);
                    uniform_data.extend(vec![0u8; padding as usize]);
                }
            }

            if let Some(buffer) = &self.uniform_buffer {
                // Update existing buffer
                self.context.queue.write_buffer(buffer, 0, &uniform_data);
            } else {
                // Create new buffer
                let buffer = self
                    .context
                    .device
                    .create_buffer(&wgpu::BufferDescriptor {
                        label: Some("Material Uniform Buffer"),
                        size: uniform_size,
                        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                self.context.queue.write_buffer(&buffer, 0, &uniform_data);
                self.uniform_buffer = Some(buffer);
            }
        }

        // Rebuild bind group layout and bind group
        self.rebuild_bind_groups();

        self.dirty = false;
    }

    /// Rebuild bind group layout and bind group.
    fn rebuild_bind_groups(&mut self) {
        let mut layout_entries = Vec::new();
        let mut bind_entries = Vec::new();
        let mut binding = 0u32;

        // Add uniform buffer binding if present
        if self.uniform_buffer.is_some() {
            layout_entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });

            bind_entries.push(wgpu::BindGroupEntry {
                binding,
                resource: self.uniform_buffer.as_ref().unwrap().as_entire_binding(),
            });

            binding += 1;
        }

        // Add texture bindings
        for texture in self.textures.values() {
            // Texture binding
            layout_entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });

            bind_entries.push(wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::TextureView(&texture.view),
            });

            binding += 1;

            // Sampler binding
            layout_entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            });

            // Use provided sampler or create default
            let sampler = if let Some(ref s) = texture.sampler {
                s
            } else {
                // Create a default linear sampler (this should be cached in practice)
                // For now, we'll use a temporary one
                // TODO: Add sampler cache to Material or GraphicsContext
                unimplemented!("Default sampler not yet implemented - please provide sampler")
            };

            bind_entries.push(wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::Sampler(sampler),
            });

            binding += 1;
        }

        // Create bind group layout
        let layout = self
            .context
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Material Bind Group Layout"),
                entries: &layout_entries,
            });

        // Create bind group
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Material Bind Group"),
                layout: &layout,
                entries: &bind_entries,
            });

        self.bind_group_layout = Some(layout);
        self.bind_group = Some(bind_group);
    }

    /// Bind this material's resources to a render pass.
    ///
    /// This will update GPU resources if needed and set the bind group.
    ///
    /// # Arguments
    ///
    /// * `pass` - The render pass to bind to
    /// * `bind_group_index` - The bind group index (default is usually 0)
    pub fn bind<'a>(&'a mut self, pass: &mut wgpu::RenderPass<'a>, bind_group_index: u32) {
        self.update_resources();

        if let Some(ref bind_group) = self.bind_group {
            pass.set_bind_group(bind_group_index, bind_group, &[]);
        }
    }

    /// Get the bind group layout (creates it if needed).
    ///
    /// This is useful when creating render pipelines.
    pub fn bind_group_layout(&mut self) -> &wgpu::BindGroupLayout {
        if self.dirty || self.bind_group_layout.is_none() {
            self.update_resources();
        }
        self.bind_group_layout
            .as_ref()
            .expect("Bind group layout should be created")
    }
}

/// Builder for creating materials with a fluent API.
pub struct MaterialBuilder {
    shader: Option<Arc<wgpu::ShaderModule>>,
    parameters: HashMap<String, MaterialParameter>,
    textures: HashMap<String, MaterialTexture>,
    pipeline_state: PipelineState,
    context: Arc<GraphicsContext>,
}

impl MaterialBuilder {
    /// Create a new material builder.
    pub fn new(context: Arc<GraphicsContext>) -> Self {
        Self {
            shader: None,
            parameters: HashMap::default(),
            textures: HashMap::default(),
            pipeline_state: PipelineState::default(),
            context,
        }
    }

    /// Set the shader from a module.
    pub fn shader(mut self, shader: Arc<wgpu::ShaderModule>) -> Self {
        self.shader = Some(shader);
        self
    }

    /// Set the shader from source code.
    pub fn shader_source(mut self, source: &str, label: Option<&str>) -> Self {
        let shader = self
            .context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label,
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
        self.shader = Some(Arc::new(shader));
        self
    }

    /// Set a parameter.
    pub fn parameter(mut self, name: impl Into<String>, value: MaterialParameter) -> Self {
        self.parameters.insert(name.into(), value);
        self
    }

    /// Set a texture.
    pub fn texture(mut self, name: impl Into<String>, texture: MaterialTexture) -> Self {
        self.textures.insert(name.into(), texture);
        self
    }

    /// Set the pipeline state.
    pub fn pipeline_state(mut self, state: PipelineState) -> Self {
        self.pipeline_state = state;
        self
    }

    /// Build the material.
    pub fn build(self) -> Material {
        let shader = self.shader.expect("Shader is required");
        let mut material = Material::new(shader, self.context);
        material.parameters = self.parameters;
        material.textures = self.textures;
        material.pipeline_state = self.pipeline_state;
        material.dirty = true;
        material
    }
}
