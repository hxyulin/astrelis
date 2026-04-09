//! Render and compute pipeline descriptors.

use crate::bind_group::ShaderStages;
use crate::id::{BindGroupLayoutId, PipelineLayoutId, ShaderModuleId};
use crate::types::{
    BlendState, ColorWrites, CompareFunction, CullMode, FrontFace, IndexFormat, PrimitiveTopology,
    PolygonMode, TextureFormat, VertexFormat, VertexStepMode,
};

/// A single vertex attribute within a vertex buffer layout.
#[derive(Clone, Copy, Debug)]
pub struct VertexAttribute {
    /// Data format of this attribute.
    pub format: VertexFormat,
    /// Byte offset from the start of the vertex.
    pub offset: u64,
    /// Shader location (`@location(N)` in WGSL).
    pub shader_location: u32,
}

/// Describes how vertex data is laid out in a buffer.
#[derive(Clone, Debug)]
pub struct VertexBufferLayout<'a> {
    /// Byte stride between consecutive elements.
    pub array_stride: u64,
    /// Whether to advance per-vertex or per-instance.
    pub step_mode: VertexStepMode,
    /// Vertex attributes within this buffer.
    pub attributes: &'a [VertexAttribute],
}

/// Describes the vertex shader stage of a render pipeline.
#[derive(Clone, Debug)]
pub struct VertexState<'a> {
    /// Shader module containing the vertex entry point.
    pub module: ShaderModuleId,
    /// Entry point function name.
    pub entry_point: &'a str,
    /// Vertex buffer layouts.
    pub buffers: &'a [VertexBufferLayout<'a>],
}

/// Describes a single color target in the fragment stage.
#[derive(Clone, Debug)]
pub struct ColorTargetState {
    /// Output texture format.
    pub format: TextureFormat,
    /// Blend state. `None` = no blending.
    pub blend: Option<BlendState>,
    /// Which color channels to write.
    pub write_mask: ColorWrites,
}

/// Describes the fragment shader stage of a render pipeline.
#[derive(Clone, Debug)]
pub struct FragmentState<'a> {
    /// Shader module containing the fragment entry point.
    pub module: ShaderModuleId,
    /// Entry point function name.
    pub entry_point: &'a str,
    /// Color targets this fragment shader writes to.
    pub targets: &'a [ColorTargetState],
}

/// Depth-stencil attachment state.
#[derive(Clone, Copy, Debug)]
pub struct DepthStencilState {
    /// Depth/stencil texture format.
    pub format: TextureFormat,
    /// Whether the depth buffer is written.
    pub depth_write_enabled: bool,
    /// Depth comparison function.
    pub depth_compare: CompareFunction,
}

/// Primitive assembly and rasterization state.
#[derive(Clone, Copy, Debug)]
pub struct PrimitiveState {
    /// Primitive topology.
    pub topology: PrimitiveTopology,
    /// Index format for strip topologies. Required for strip topologies.
    pub strip_index_format: Option<IndexFormat>,
    /// Front-face winding order.
    pub front_face: FrontFace,
    /// Face culling mode.
    pub cull_mode: CullMode,
    /// Polygon rasterization mode.
    pub polygon_mode: PolygonMode,
    /// If `true`, depth values are clamped instead of clipped.
    pub unclipped_depth: bool,
}

impl Default for PrimitiveState {
    fn default() -> Self {
        Self {
            topology: PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: FrontFace::Ccw,
            cull_mode: CullMode::None,
            polygon_mode: PolygonMode::Fill,
            unclipped_depth: false,
        }
    }
}

/// Multisample rasterization state.
#[derive(Clone, Copy, Debug)]
pub struct MultisampleState {
    /// Number of samples. 1 = no multisampling.
    pub count: u32,
    /// Bitmask controlling which samples are active.
    pub mask: u64,
    /// If `true`, alpha values affect sample coverage.
    pub alpha_to_coverage_enabled: bool,
}

impl Default for MultisampleState {
    fn default() -> Self {
        Self {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        }
    }
}

/// A push constant range.
#[derive(Clone, Debug)]
pub struct PushConstantRange {
    /// Which shader stages can access this range.
    pub stages: ShaderStages,
    /// Byte range within the push constant block.
    pub range: std::ops::Range<u32>,
}

/// Describes a pipeline layout.
#[derive(Clone, Debug)]
pub struct PipelineLayoutDescriptor<'a> {
    /// Debug label.
    pub label: Option<&'a str>,
    /// Bind group layouts used by this pipeline.
    pub bind_group_layouts: &'a [BindGroupLayoutId],
    /// Push constant ranges.
    pub push_constant_ranges: &'a [PushConstantRange],
}

/// Describes a render pipeline.
#[derive(Clone, Debug)]
pub struct RenderPipelineDescriptor<'a> {
    /// Debug label.
    pub label: Option<&'a str>,
    /// Pipeline layout. `None` = auto-derive from shader.
    pub layout: Option<PipelineLayoutId>,
    /// Vertex stage configuration.
    pub vertex: VertexState<'a>,
    /// Primitive state.
    pub primitive: PrimitiveState,
    /// Depth/stencil state. `None` = no depth/stencil.
    pub depth_stencil: Option<DepthStencilState>,
    /// Multisample state.
    pub multisample: MultisampleState,
    /// Fragment stage configuration. `None` = vertex-only pipeline.
    pub fragment: Option<FragmentState<'a>>,
}

/// Describes a compute pipeline.
#[derive(Clone, Debug)]
pub struct ComputePipelineDescriptor<'a> {
    /// Debug label.
    pub label: Option<&'a str>,
    /// Pipeline layout. `None` = auto-derive from shader.
    pub layout: Option<PipelineLayoutId>,
    /// Shader module containing the compute entry point.
    pub module: ShaderModuleId,
    /// Entry point function name.
    pub entry_point: &'a str,
}
