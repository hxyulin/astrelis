use std::{borrow::Cow, collections::HashMap, num::NonZeroU64, path::PathBuf};

use wgpu::{DepthStencilState, MultisampleState, VertexBufferLayout};

use crate::{
    alloc::{IndexSlot, SparseSet},
    graphics::Material,
};

#[repr(transparent)]
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, bytemuck::NoUninit)]
pub struct ShaderHandle(IndexSlot);

pub enum ShaderSource {
    File(PathBuf),
    String(Cow<'static, str>),
}

impl ShaderSource {
    pub fn as_str(&self) -> Cow<'_, str> {
        // TODO: Error handling
        match self {
            Self::String(str) => Cow::Borrowed(str),
            Self::File(path) => Cow::Owned(std::fs::read_to_string(path).unwrap()),
        }
    }
}

pub struct Shader {
    module: Option<ShaderModule>,
    source: ShaderSource,
    bind_groups: Vec<BindGroup>,
}

pub struct ShaderResources<'a> {
    pub vertex_buffers: &'a [VertexBufferLayout<'a>],
    pub targets: &'a [Option<wgpu::ColorTargetState>],
    pub resources: HashMap<UniformType, wgpu::BindingResource<'a>>,
    pub depth_stencil: Option<DepthStencilState>,
    pub multisample: MultisampleState,
}

impl Default for ShaderResources<'_> {
    fn default() -> Self {
        Self {
            vertex_buffers: &[],
            targets: &[],
            resources: HashMap::new(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
        }
    }
}

pub struct PipelineCacheEntry {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_groups: Vec<(u32, wgpu::BindGroup)>,
}

impl Shader {
    pub fn new(source: ShaderSource, bind_groups: Vec<BindGroup>) -> Self {
        // For now, all entrypoints must be named vs_main and fs_main
        Self {
            module: None,
            bind_groups,
            source,
        }
    }

    pub fn bind_group_layouts(&self, device: &wgpu::Device) -> Vec<wgpu::BindGroupLayout> {
        let mut out = Vec::with_capacity(self.bind_groups.len());
        for group in &self.bind_groups {
            let mut entries = Vec::with_capacity(group.entries.len());
            for (idx, entry) in group.entries.iter().enumerate() {
                entries.push(wgpu::BindGroupLayoutEntry {
                    binding: idx as u32,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: entry.ty.clone().into(),
                    // Don't support arrays yet
                    count: None,
                });
            }

            let desc = wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &entries,
            };
            out.push(device.create_bind_group_layout(&desc));
        }
        out
    }

    pub fn get_or_create_module(&mut self, device: &wgpu::Device) -> &ShaderModule {
        if self.module.is_none() {
            self.module = Some(ShaderModule {
                module: device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: None,
                    source: wgpu::ShaderSource::Wgsl(self.source.as_str()),
                }),
            });
        }

        self.module.as_ref().unwrap()
    }

    pub fn create_pipeline(
        &mut self,
        device: &wgpu::Device,
        mut resources: ShaderResources,
    ) -> PipelineCacheEntry {
        let bind_layouts = self.bind_group_layouts(device);
        let bind_groups = bind_layouts
            .iter()
            .enumerate()
            .map(|(idx, bl)| {
                let mut entries = Vec::new();
                for entry in &self.bind_groups[idx].entries {
                    let ty = match &entry.ty {
                        BindingType::Buffer { ty, size: _ } => match ty {
                            BufferBindingType::Uniform { ty } => ty,
                        },
                        BindingType::Texture => continue,
                    };

                    if let Some(resource) = resources.resources.remove(ty) {
                        entries.push(wgpu::BindGroupEntry {
                            binding: idx as u32,
                            resource,
                        });
                    }
                }

                (
                    idx as u32,
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None,
                        layout: bl,
                        entries: entries.as_slice(),
                    }),
                )
            })
            .collect::<Vec<_>>();

        let bind_group_layouts = bind_layouts.iter().by_ref().collect::<Vec<_>>();

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: bind_group_layouts.as_slice(),
            push_constant_ranges: &[],
        });

        let module = &self.get_or_create_module(device).module;

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            cache: None,
            vertex: wgpu::VertexState {
                module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: resources.vertex_buffers,
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: resources.targets,
            }),
            depth_stencil: resources.depth_stencil,
            multisample: resources.multisample,
            // TODO: Make these an engine feature as well
            multiview: None,
            primitive: wgpu::PrimitiveState::default(),
        });

        PipelineCacheEntry {
            pipeline,
            bind_groups,
        }
    }
}

pub struct ShaderModule {
    module: wgpu::ShaderModule,
}

pub struct BindGroup {
    entries: Vec<BindGroupEntry>,
}

#[derive(Debug, Clone)]
pub enum BindingType {
    Texture,
    Buffer { ty: BufferBindingType, size: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BuiltinUniform {
    Material,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UniformType {
    Builtin(BuiltinUniform),
    Custom(),
}

#[derive(Debug, Clone)]
pub enum BufferBindingType {
    Uniform { ty: UniformType },
}

impl Into<wgpu::BufferBindingType> for BufferBindingType {
    fn into(self) -> wgpu::BufferBindingType {
        match self {
            Self::Uniform { .. } => wgpu::BufferBindingType::Uniform,
        }
    }
}

impl Into<wgpu::BindingType> for BindingType {
    fn into(self) -> wgpu::BindingType {
        match self {
            Self::Buffer { ty, size } => wgpu::BindingType::Buffer {
                ty: ty.into(),
                has_dynamic_offset: false,
                min_binding_size: NonZeroU64::new(size as u64),
            },
            _ => unimplemented!(),
        }
    }
}

pub struct BindGroupEntry {
    pub ty: BindingType,
}

impl ShaderModule {}

pub fn material_shader() -> Shader {
    let source = ShaderSource::String(Cow::Borrowed(include_str!("renderer/material.wgsl")));
    let bind_groups = vec![BindGroup {
        entries: vec![BindGroupEntry {
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform {
                    ty: UniformType::Builtin(BuiltinUniform::Material),
                },
                size: std::mem::size_of::<Material>(),
            },
        }],
    }];
    Shader::new(source, bind_groups)
}

pub struct ShaderManager {
    shaders: SparseSet<Shader>,
}

impl ShaderManager {
    pub fn new() -> Self {
        Self {
            shaders: SparseSet::new(),
        }
    }

    pub fn create_shader(&mut self, shader: Shader) -> ShaderHandle {
        ShaderHandle(self.shaders.push(shader))
    }

    pub fn get_shader(&self, handle: ShaderHandle) -> &Shader {
        self.shaders.get(handle.0)
    }

    pub fn get_shader_mut(&mut self, handle: ShaderHandle) -> &mut Shader {
        self.shaders.get_mut(handle.0)
    }
}

pub struct PipelineCache {
    pipeline_cache: HashMap<ShaderHandle, PipelineCacheEntry>,
}

impl PipelineCache {
    pub fn new() -> Self {
        Self {
            pipeline_cache: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.pipeline_cache.clear();
    }

    pub fn get_or_create_pipeline<F>(
        &mut self,
        handle: ShaderHandle,
        init_fn: F,
    ) -> &PipelineCacheEntry
    where
        F: FnOnce() -> PipelineCacheEntry,
    {
        if !self.pipeline_cache.contains_key(&handle) {
            self.pipeline_cache.insert(handle.clone(), init_fn());
        }
        self.pipeline_cache.get(&handle).unwrap()
    }
}

pub struct BufferLayout {
    pub attributes: Vec<wgpu::VertexAttribute>,
    pub size: u64,
}

impl BufferLayout {
    pub fn get_wgpu(&self, step_mode: wgpu::VertexStepMode) -> wgpu::VertexBufferLayout<'_> {
        wgpu::VertexBufferLayout {
            array_stride: self.size,
            step_mode,
            attributes: self.attributes.as_slice(),
        }
    }
}

pub trait ShaderBufferCompatible {
    fn buffer_layout(base_position: u32) -> BufferLayout;
}

pub trait AsVertexFormat {
    fn vertex_format() -> wgpu::VertexFormat;
}

impl AsVertexFormat for f32 {
    fn vertex_format() -> wgpu::VertexFormat {
        wgpu::VertexFormat::Float32
    }
}

impl AsVertexFormat for glam::Vec2 {
    fn vertex_format() -> wgpu::VertexFormat {
        wgpu::VertexFormat::Float32x2
    }
}

impl AsVertexFormat for glam::Vec3 {
    fn vertex_format() -> wgpu::VertexFormat {
        wgpu::VertexFormat::Float32x3
    }
}

impl AsVertexFormat for glam::Vec4 {
    fn vertex_format() -> wgpu::VertexFormat {
        wgpu::VertexFormat::Float32x4
    }
}
