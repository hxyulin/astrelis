//! Backend-neutral GPU vocabulary.

use std::{fmt, num::NonZeroU64, ops::Range};

use bitflags::bitflags;

/// Identifier shared by a device and all resources created from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DeviceId(pub(crate) u64);

/// Physical graphics API used by a backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GraphicsApi {
    /// Vulkan.
    Vulkan,
    /// Metal.
    Metal,
    /// Direct3D 12.
    Dx12,
    /// OpenGL or OpenGL ES.
    Gl,
    /// Browser WebGPU.
    WebGpu,
    /// A backend not represented by this version of Astrelis.
    Other,
}

/// Broad hardware class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DeviceType {
    /// Integrated GPU.
    Integrated,
    /// Discrete GPU.
    Discrete,
    /// Virtual GPU.
    Virtual,
    /// CPU or software renderer.
    Cpu,
    /// Unknown hardware class.
    Other,
}

/// Adapter metadata suitable for diagnostics and selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterInfo {
    /// Human-readable adapter name.
    pub name: String,
    /// Driver name.
    pub driver: String,
    /// Driver-specific information.
    pub driver_info: String,
    /// Numeric vendor identifier where available.
    pub vendor: u32,
    /// Numeric device identifier where available.
    pub device: u32,
    /// Hardware class.
    pub device_type: DeviceType,
    /// Underlying graphics API.
    pub api: GraphicsApi,
}

/// Adapter power preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PowerPreference {
    /// Let the backend choose.
    #[default]
    None,
    /// Prefer lower power consumption.
    LowPower,
    /// Prefer higher performance.
    HighPerformance,
}

bitflags! {
    /// Optional GPU functionality.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Features: u64 {
        /// Timestamp queries at render/compute pass boundaries.
        const TIMESTAMP_QUERY = 1 << 0;
        /// Timestamp writes directly on command encoders.
        const TIMESTAMP_QUERY_INSIDE_ENCODERS = 1 << 1;
        /// BC compressed textures.
        const TEXTURE_COMPRESSION_BC = 1 << 2;
        /// ETC2 compressed textures.
        const TEXTURE_COMPRESSION_ETC2 = 1 << 3;
        /// ASTC compressed textures.
        const TEXTURE_COMPRESSION_ASTC = 1 << 4;
        /// Indirect draw count commands.
        const MULTI_DRAW_INDIRECT_COUNT = 1 << 5;
        /// Polygon line mode.
        const POLYGON_MODE_LINE = 1 << 6;
    }
}

/// Focused device limits used by the first rendering milestones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Maximum 1D/2D texture dimension.
    pub max_texture_dimension_2d: u32,
    /// Maximum bind groups per pipeline.
    pub max_bind_groups: u32,
    /// Maximum vertex buffers.
    pub max_vertex_buffers: u32,
    /// Maximum buffer size.
    pub max_buffer_size: u64,
    /// Required alignment for uniform buffer offsets.
    pub min_uniform_buffer_offset_alignment: u32,
    /// Required alignment for storage buffer offsets.
    pub min_storage_buffer_offset_alignment: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_texture_dimension_2d: 8_192,
            max_bind_groups: 4,
            max_vertex_buffers: 8,
            max_buffer_size: 256 << 20,
            min_uniform_buffer_offset_alignment: 256,
            min_storage_buffer_offset_alignment: 256,
        }
    }
}

/// Device capabilities actually enabled after negotiation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceCapabilities {
    /// Enabled optional features.
    pub features: Features,
    /// Enabled limits.
    pub limits: Limits,
    /// Underlying graphics API.
    pub api: GraphicsApi,
    /// Whether timestamp results are known to be reliable.
    pub reliable_timestamps: bool,
}

/// Adapter selection request.
#[derive(Clone, Debug, Default)]
pub struct RequestAdapterOptions {
    /// Power preference.
    pub power_preference: PowerPreference,
    /// Require a software/fallback adapter.
    pub force_fallback_adapter: bool,
    /// Surface with which the adapter must be compatible.
    pub compatible_surface: Option<crate::Surface>,
}

/// Logical device request.
#[derive(Clone, Debug, Default)]
pub struct DeviceDescriptor {
    /// Optional debug label.
    pub label: Option<String>,
    /// Features without which creation must fail.
    pub required_features: Features,
    /// Features enabled only when the adapter supports them.
    pub optional_features: Features,
    /// Minimum required limits.
    pub required_limits: Limits,
}

bitflags! {
    /// Permitted buffer uses.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct BufferUsages: u32 {
        /// Buffer can be mapped for CPU reads.
        const MAP_READ = 1 << 0;
        /// Buffer can be mapped for CPU writes.
        const MAP_WRITE = 1 << 1;
        /// Copy source.
        const COPY_SRC = 1 << 2;
        /// Copy destination.
        const COPY_DST = 1 << 3;
        /// Index buffer.
        const INDEX = 1 << 4;
        /// Vertex buffer.
        const VERTEX = 1 << 5;
        /// Uniform buffer.
        const UNIFORM = 1 << 6;
        /// Storage buffer.
        const STORAGE = 1 << 7;
        /// Indirect arguments.
        const INDIRECT = 1 << 8;
        /// Query resolve destination.
        const QUERY_RESOLVE = 1 << 9;
    }
}

/// Buffer creation settings.
#[derive(Clone, Debug)]
pub struct BufferDescriptor {
    /// Optional debug label.
    pub label: Option<String>,
    /// Size in bytes.
    pub size: u64,
    /// Permitted uses.
    pub usage: BufferUsages,
    /// Whether the buffer begins mapped.
    pub mapped_at_creation: bool,
}

/// CPU mapping access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapMode {
    /// Read mapping.
    Read,
    /// Write mapping.
    Write,
}

bitflags! {
    /// Permitted texture uses.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct TextureUsages: u32 {
        /// Copy source.
        const COPY_SRC = 1 << 0;
        /// Copy destination.
        const COPY_DST = 1 << 1;
        /// Shader sampled texture.
        const TEXTURE_BINDING = 1 << 2;
        /// Shader storage texture.
        const STORAGE_BINDING = 1 << 3;
        /// Render or depth attachment.
        const RENDER_ATTACHMENT = 1 << 4;
    }
}

/// Texture dimensionality.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextureDimension {
    /// One-dimensional texture.
    D1,
    /// Two-dimensional texture.
    #[default]
    D2,
    /// Three-dimensional texture.
    D3,
}

/// Texture or surface pixel format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TextureFormat {
    /// 8-bit normalized red.
    R8Unorm,
    /// 8-bit normalized RGBA.
    Rgba8Unorm,
    /// 8-bit normalized sRGB RGBA.
    Rgba8UnormSrgb,
    /// 8-bit normalized BGRA.
    Bgra8Unorm,
    /// 8-bit normalized sRGB BGRA.
    Bgra8UnormSrgb,
    /// 16-bit floating-point RGBA.
    Rgba16Float,
    /// 32-bit floating-point red.
    R32Float,
    /// 32-bit unsigned integer red.
    R32Uint,
    /// 16-bit depth.
    Depth16Unorm,
    /// 24-bit depth plus 8-bit stencil.
    Depth24PlusStencil8,
    /// 32-bit floating-point depth.
    Depth32Float,
}

/// Three-dimensional texel extent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Extent3d {
    /// Width in texels.
    pub width: u32,
    /// Height in texels.
    pub height: u32,
    /// Depth or array layers.
    pub depth_or_array_layers: u32,
}

/// Origin of a texture copy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Origin3d {
    /// X coordinate.
    pub x: u32,
    /// Y coordinate.
    pub y: u32,
    /// Z coordinate or array layer.
    pub z: u32,
}

/// Texture participating in a copy operation.
#[derive(Clone, Debug)]
pub struct TextureCopy {
    /// Texture resource.
    pub texture: crate::Texture,
    /// Mip level.
    pub mip_level: u32,
    /// Copy origin.
    pub origin: Origin3d,
}

/// Buffer layout for a texture copy.
#[derive(Clone, Debug)]
pub struct BufferTextureCopy {
    /// Buffer resource.
    pub buffer: crate::Buffer,
    /// Starting byte offset.
    pub offset: u64,
    /// Bytes per image row.
    pub bytes_per_row: Option<u32>,
    /// Rows per image.
    pub rows_per_image: Option<u32>,
}

/// CPU byte layout used when uploading texture data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextureDataLayout {
    /// Starting byte offset.
    pub offset: u64,
    /// Bytes per image row.
    pub bytes_per_row: Option<u32>,
    /// Rows per image.
    pub rows_per_image: Option<u32>,
}

impl Extent3d {
    /// Creates a two-dimensional extent with one layer.
    pub const fn d2(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            depth_or_array_layers: 1,
        }
    }
}

/// Texture creation settings.
#[derive(Clone, Debug)]
pub struct TextureDescriptor {
    /// Optional debug label.
    pub label: Option<String>,
    /// Texture extent.
    pub size: Extent3d,
    /// Mip level count.
    pub mip_level_count: u32,
    /// Sample count.
    pub sample_count: u32,
    /// Dimensionality.
    pub dimension: TextureDimension,
    /// Pixel format.
    pub format: TextureFormat,
    /// Permitted uses.
    pub usage: TextureUsages,
}

/// Texture view creation settings.
#[derive(Clone, Debug, Default)]
pub struct TextureViewDescriptor {
    /// Optional debug label.
    pub label: Option<String>,
    /// Format reinterpretation.
    pub format: Option<TextureFormat>,
    /// First mip level.
    pub base_mip_level: u32,
    /// Number of mip levels, or all remaining levels.
    pub mip_level_count: Option<u32>,
    /// First array layer.
    pub base_array_layer: u32,
    /// Number of array layers, or all remaining layers.
    pub array_layer_count: Option<u32>,
}

/// Texture address mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AddressMode {
    /// Clamp to the edge texel.
    #[default]
    ClampToEdge,
    /// Repeat.
    Repeat,
    /// Mirrored repeat.
    MirrorRepeat,
}

/// Texture filter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FilterMode {
    /// Nearest neighbor.
    #[default]
    Nearest,
    /// Linear filtering.
    Linear,
}

/// Sampler creation settings.
#[derive(Clone, Debug)]
pub struct SamplerDescriptor {
    /// Optional debug label.
    pub label: Option<String>,
    /// U address mode.
    pub address_mode_u: AddressMode,
    /// V address mode.
    pub address_mode_v: AddressMode,
    /// W address mode.
    pub address_mode_w: AddressMode,
    /// Magnification filter.
    pub mag_filter: FilterMode,
    /// Minification filter.
    pub min_filter: FilterMode,
    /// Mipmap filter.
    pub mipmap_filter: FilterMode,
    /// Minimum level of detail.
    pub lod_min_clamp: f32,
    /// Maximum level of detail.
    pub lod_max_clamp: f32,
}

impl Default for SamplerDescriptor {
    fn default() -> Self {
        Self {
            label: None,
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 32.0,
        }
    }
}

/// Shader creation settings.
#[derive(Clone, Debug)]
pub struct ShaderModuleDescriptor {
    /// Optional debug label.
    pub label: Option<String>,
    /// WGSL source.
    pub wgsl: String,
}

bitflags! {
    /// Shader stages visible to a binding.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct ShaderStages: u32 {
        /// Vertex shader.
        const VERTEX = 1 << 0;
        /// Fragment shader.
        const FRAGMENT = 1 << 1;
        /// Compute shader.
        const COMPUTE = 1 << 2;
    }
}

/// Buffer binding category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BufferBindingType {
    /// Uniform buffer.
    Uniform,
    /// Read-only storage buffer.
    ReadOnlyStorage,
    /// Read-write storage buffer.
    Storage,
}

/// Sampler binding category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SamplerBindingType {
    /// Filtering sampler.
    Filtering,
    /// Non-filtering sampler.
    NonFiltering,
    /// Comparison sampler.
    Comparison,
}

/// Sampled texture scalar category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureSampleType {
    /// Filterable floating-point texture.
    Float,
    /// Non-filterable floating-point texture.
    UnfilterableFloat,
    /// Signed integer texture.
    Sint,
    /// Unsigned integer texture.
    Uint,
    /// Depth texture.
    Depth,
}

/// Texture shape as seen by a shader.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextureViewDimension {
    /// One-dimensional texture.
    D1,
    /// Two-dimensional texture.
    #[default]
    D2,
    /// Two-dimensional array texture.
    D2Array,
    /// Cube texture.
    Cube,
    /// Cube array texture.
    CubeArray,
    /// Three-dimensional texture.
    D3,
}

/// Bind-group layout entry category.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingType {
    /// Buffer binding.
    Buffer {
        /// Buffer category.
        ty: BufferBindingType,
        /// Whether dynamic offsets are accepted.
        has_dynamic_offset: bool,
        /// Minimum bound size.
        min_binding_size: Option<NonZeroU64>,
    },
    /// Sampler binding.
    Sampler(SamplerBindingType),
    /// Sampled texture binding.
    Texture {
        /// Sample scalar category.
        sample_type: TextureSampleType,
        /// Shader view dimension.
        view_dimension: TextureViewDimension,
        /// Whether the texture is multisampled.
        multisampled: bool,
    },
}

/// One bind-group layout slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindGroupLayoutEntry {
    /// Shader binding number.
    pub binding: u32,
    /// Visible shader stages.
    pub visibility: ShaderStages,
    /// Resource category.
    pub ty: BindingType,
}

/// Bind-group layout creation settings.
#[derive(Clone, Debug)]
pub struct BindGroupLayoutDescriptor {
    /// Optional debug label.
    pub label: Option<String>,
    /// Binding slots.
    pub entries: Vec<BindGroupLayoutEntry>,
}

/// Pipeline layout creation settings.
#[derive(Clone, Debug)]
pub struct PipelineLayoutDescriptor {
    /// Optional debug label.
    pub label: Option<String>,
    /// Bind-group layouts by group index.
    pub bind_group_layouts: Vec<crate::BindGroupLayout>,
}

/// Buffer resource bound to a shader.
#[derive(Clone, Debug)]
pub struct BufferBinding {
    /// Buffer.
    pub buffer: crate::Buffer,
    /// Byte offset.
    pub offset: u64,
    /// Optional bound size.
    pub size: Option<NonZeroU64>,
}

/// Resource used by one bind-group entry.
#[derive(Clone, Debug)]
pub enum BindingResource {
    /// Buffer range.
    Buffer(BufferBinding),
    /// Sampler.
    Sampler(crate::Sampler),
    /// Texture view.
    TextureView(crate::TextureView),
}

/// One populated bind-group slot.
#[derive(Clone, Debug)]
pub struct BindGroupEntry {
    /// Shader binding number.
    pub binding: u32,
    /// Bound resource.
    pub resource: BindingResource,
}

/// Bind-group creation settings.
#[derive(Clone, Debug)]
pub struct BindGroupDescriptor {
    /// Optional debug label.
    pub label: Option<String>,
    /// Compatible layout.
    pub layout: crate::BindGroupLayout,
    /// Populated entries.
    pub entries: Vec<BindGroupEntry>,
}

/// Vertex input scalar/vector format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum VertexFormat {
    /// Two 32-bit floats.
    Float32x2,
    /// Three 32-bit floats.
    Float32x3,
    /// Four 32-bit floats.
    Float32x4,
    /// Four normalized unsigned bytes.
    Unorm8x4,
}

/// Vertex buffer advancement mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum VertexStepMode {
    /// Advance per vertex.
    #[default]
    Vertex,
    /// Advance per instance.
    Instance,
}

/// One shader vertex input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VertexAttribute {
    /// Byte offset within a vertex.
    pub offset: u64,
    /// Shader location.
    pub shader_location: u32,
    /// Input format.
    pub format: VertexFormat,
}

/// One vertex buffer slot layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VertexBufferLayout {
    /// Byte stride between elements.
    pub array_stride: u64,
    /// Per-vertex or per-instance advancement.
    pub step_mode: VertexStepMode,
    /// Shader attributes sourced from this buffer.
    pub attributes: Vec<VertexAttribute>,
}

/// Primitive topology.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PrimitiveTopology {
    /// Independent points.
    PointList,
    /// Independent lines.
    LineList,
    /// Connected line strip.
    LineStrip,
    /// Independent triangles.
    #[default]
    TriangleList,
    /// Connected triangle strip.
    TriangleStrip,
}

/// Triangle winding used to identify front faces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FrontFace {
    /// Counter-clockwise winding.
    #[default]
    Ccw,
    /// Clockwise winding.
    Cw,
}

/// Face culling selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    /// Front-facing triangles.
    Front,
    /// Back-facing triangles.
    Back,
}

/// Primitive assembly and rasterization settings.
#[derive(Clone, Debug)]
pub struct PrimitiveState {
    /// Primitive topology.
    pub topology: PrimitiveTopology,
    /// Front-face winding.
    pub front_face: FrontFace,
    /// Optional culled face.
    pub cull_mode: Option<Face>,
}

impl Default for PrimitiveState {
    fn default() -> Self {
        Self {
            topology: PrimitiveTopology::TriangleList,
            front_face: FrontFace::Ccw,
            cull_mode: None,
        }
    }
}

bitflags! {
    /// Enabled color channels for a render target.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct ColorWrites: u32 {
        /// Red channel.
        const RED = 1 << 0;
        /// Green channel.
        const GREEN = 1 << 1;
        /// Blue channel.
        const BLUE = 1 << 2;
        /// Alpha channel.
        const ALPHA = 1 << 3;
        /// All channels.
        const ALL = Self::RED.bits() | Self::GREEN.bits() | Self::BLUE.bits() | Self::ALPHA.bits();
    }
}

/// Fragment render target state.
#[derive(Clone, Debug)]
pub struct ColorTargetState {
    /// Target format.
    pub format: TextureFormat,
    /// Optional color blending.
    pub blend: Option<BlendState>,
    /// Writable color channels.
    pub write_mask: ColorWrites,
}

/// Blend multiplier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlendFactor {
    /// Zero.
    Zero,
    /// One.
    One,
    /// Source alpha.
    SrcAlpha,
    /// One minus source alpha.
    OneMinusSrcAlpha,
    /// Destination alpha.
    DstAlpha,
    /// One minus destination alpha.
    OneMinusDstAlpha,
}

/// Blend arithmetic operation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BlendOperation {
    /// Add source and destination terms.
    #[default]
    Add,
    /// Subtract the destination term from the source term.
    Subtract,
    /// Subtract the source term from the destination term.
    ReverseSubtract,
    /// Select the smaller term.
    Min,
    /// Select the larger term.
    Max,
}

/// Blend settings for one color component group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlendComponent {
    /// Source multiplier.
    pub src_factor: BlendFactor,
    /// Destination multiplier.
    pub dst_factor: BlendFactor,
    /// Arithmetic operation.
    pub operation: BlendOperation,
}

impl BlendComponent {
    /// Replaces the destination with the source.
    pub const REPLACE: Self = Self {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::Zero,
        operation: BlendOperation::Add,
    };

    /// Premultiplied source-over blending.
    pub const PREMULTIPLIED_ALPHA: Self = Self {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::OneMinusSrcAlpha,
        operation: BlendOperation::Add,
    };
}

/// Color and alpha blending state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlendState {
    /// RGB blending.
    pub color: BlendComponent,
    /// Alpha blending.
    pub alpha: BlendComponent,
}

impl BlendState {
    /// Premultiplied source-over blending.
    pub const PREMULTIPLIED_ALPHA: Self = Self {
        color: BlendComponent::PREMULTIPLIED_ALPHA,
        alpha: BlendComponent::PREMULTIPLIED_ALPHA,
    };
}

/// Depth/stencil comparison function.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CompareFunction {
    /// Never pass.
    Never,
    /// Pass when less.
    Less,
    /// Pass when equal.
    Equal,
    /// Pass when less or equal.
    LessEqual,
    /// Pass when greater.
    Greater,
    /// Pass when not equal.
    NotEqual,
    /// Pass when greater or equal.
    GreaterEqual,
    /// Always pass.
    #[default]
    Always,
}

/// Operation applied to a stencil value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StencilOperation {
    /// Preserve the current value.
    #[default]
    Keep,
    /// Replace with zero.
    Zero,
    /// Replace with the stencil reference.
    Replace,
    /// Increment and clamp.
    IncrementClamp,
    /// Decrement and clamp.
    DecrementClamp,
    /// Invert all bits.
    Invert,
    /// Increment and wrap.
    IncrementWrap,
    /// Decrement and wrap.
    DecrementWrap,
}

/// Stencil behavior for one face orientation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StencilFaceState {
    /// Comparison against the stencil reference.
    pub compare: CompareFunction,
    /// Operation when stencil comparison fails.
    pub fail_op: StencilOperation,
    /// Operation when depth comparison fails.
    pub depth_fail_op: StencilOperation,
    /// Operation when both comparisons pass.
    pub pass_op: StencilOperation,
}

impl StencilFaceState {
    /// Ignore stencil and preserve its value.
    pub const IGNORE: Self = Self {
        compare: CompareFunction::Always,
        fail_op: StencilOperation::Keep,
        depth_fail_op: StencilOperation::Keep,
        pass_op: StencilOperation::Keep,
    };
}

/// Stencil pipeline state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StencilState {
    /// Front-facing triangle behavior.
    pub front: StencilFaceState,
    /// Back-facing triangle behavior.
    pub back: StencilFaceState,
    /// Mask applied when comparing values.
    pub read_mask: u32,
    /// Mask applied when writing values.
    pub write_mask: u32,
}

/// Depth/stencil pipeline state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthStencilState {
    /// Attachment format.
    pub format: TextureFormat,
    /// Whether depth writes are enabled.
    pub depth_write_enabled: bool,
    /// Depth comparison.
    pub depth_compare: CompareFunction,
    /// Stencil behavior.
    pub stencil: StencilState,
    /// Depth bias constant.
    pub bias_constant: i32,
    /// Depth bias slope scale.
    pub bias_slope_scale: f32,
    /// Depth bias clamp.
    pub bias_clamp: f32,
}

/// Multisampling pipeline state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MultisampleState {
    /// Rasterization sample count.
    pub count: u32,
    /// Active sample mask.
    pub mask: u64,
    /// Enables alpha-to-coverage.
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

/// Vertex shader pipeline state.
#[derive(Clone, Debug)]
pub struct VertexState {
    /// Shader module.
    pub module: crate::ShaderModule,
    /// Entry point.
    pub entry_point: String,
    /// Vertex buffer layouts by slot.
    pub buffers: Vec<VertexBufferLayout>,
}

/// Fragment shader pipeline state.
#[derive(Clone, Debug)]
pub struct FragmentState {
    /// Shader module.
    pub module: crate::ShaderModule,
    /// Entry point.
    pub entry_point: String,
    /// Color targets by attachment slot.
    pub targets: Vec<Option<ColorTargetState>>,
}

/// Render pipeline creation settings.
#[derive(Clone, Debug)]
pub struct RenderPipelineDescriptor {
    /// Optional debug label.
    pub label: Option<String>,
    /// Explicit pipeline layout, or automatic reflection when omitted.
    pub layout: Option<crate::PipelineLayout>,
    /// Vertex stage.
    pub vertex: VertexState,
    /// Primitive state.
    pub primitive: PrimitiveState,
    /// Optional depth/stencil state.
    pub depth_stencil: Option<DepthStencilState>,
    /// Multisampling state.
    pub multisample: MultisampleState,
    /// Optional fragment stage.
    pub fragment: Option<FragmentState>,
}

/// Compute pipeline creation settings.
#[derive(Clone, Debug)]
pub struct ComputePipelineDescriptor {
    /// Optional debug label.
    pub label: Option<String>,
    /// Explicit pipeline layout, or automatic reflection when omitted.
    pub layout: Option<crate::PipelineLayout>,
    /// Compute shader.
    pub module: crate::ShaderModule,
    /// Entry point.
    pub entry_point: String,
}

/// RGBA render color in linear space.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Color {
    /// Red component.
    pub r: f64,
    /// Green component.
    pub g: f64,
    /// Blue component.
    pub b: f64,
    /// Alpha component.
    pub a: f64,
}

/// Attachment load behavior.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LoadOp {
    /// Preserve previous contents.
    Load,
    /// Clear to a color.
    Clear(Color),
}

/// Attachment store behavior.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StoreOp {
    /// Store results.
    #[default]
    Store,
    /// Discard results.
    Discard,
}

/// Color attachment description.
#[derive(Clone, Debug)]
pub struct RenderPassColorAttachment {
    /// Render target view.
    pub view: crate::TextureView,
    /// Optional multisample resolve target.
    pub resolve_target: Option<crate::TextureView>,
    /// Load operation.
    pub load: LoadOp,
    /// Store operation.
    pub store: StoreOp,
}

/// Load/store operations for an attachment aspect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttachmentOperations<T> {
    /// Load behavior.
    pub load: LoadOpValue<T>,
    /// Store behavior.
    pub store: StoreOp,
}

/// Generic attachment load behavior.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LoadOpValue<T> {
    /// Preserve previous contents.
    Load,
    /// Clear to a value.
    Clear(T),
}

/// Depth/stencil attachment description.
#[derive(Clone, Debug)]
pub struct RenderPassDepthStencilAttachment {
    /// Attachment view.
    pub view: crate::TextureView,
    /// Optional depth operations.
    pub depth_ops: Option<AttachmentOperations<f32>>,
    /// Optional stencil operations.
    pub stencil_ops: Option<AttachmentOperations<u32>>,
}

/// Render pass creation settings.
#[derive(Clone, Debug, Default)]
pub struct RenderPassDescriptor {
    /// Optional debug label.
    pub label: Option<String>,
    /// Color attachments.
    pub color_attachments: Vec<Option<RenderPassColorAttachment>>,
    /// Optional depth/stencil attachment.
    pub depth_stencil_attachment: Option<RenderPassDepthStencilAttachment>,
    /// Optional beginning/end timestamps.
    pub timestamp_writes: Option<RenderPassTimestampWrites>,
}

/// Index buffer element format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IndexFormat {
    /// 16-bit unsigned indices.
    Uint16,
    /// 32-bit unsigned indices.
    Uint32,
}

/// Compute pass creation settings.
#[derive(Clone, Debug, Default)]
pub struct ComputePassDescriptor {
    /// Optional debug label.
    pub label: Option<String>,
}

/// Command encoder creation settings.
#[derive(Clone, Debug, Default)]
pub struct CommandEncoderDescriptor {
    /// Optional debug label.
    pub label: Option<String>,
}

/// Query set kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QueryType {
    /// GPU timestamp queries.
    Timestamp,
}

/// Query set creation settings.
#[derive(Clone, Debug)]
pub struct QuerySetDescriptor {
    /// Optional debug label.
    pub label: Option<String>,
    /// Query type.
    pub query_type: QueryType,
    /// Number of query slots.
    pub count: u32,
}

/// Timestamp writes associated with a render pass.
#[derive(Clone, Debug)]
pub struct RenderPassTimestampWrites {
    /// Destination query set.
    pub query_set: crate::QuerySet,
    /// Optional query written at the beginning of the pass.
    pub beginning_of_pass_write_index: Option<u32>,
    /// Optional query written at the end of the pass.
    pub end_of_pass_write_index: Option<u32>,
}

/// Surface presentation mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PresentMode {
    /// Vsync-backed first-in-first-out presentation.
    #[default]
    Fifo,
    /// Low-latency mailbox presentation.
    Mailbox,
    /// Immediate, potentially tearing presentation.
    Immediate,
}

/// Surface alpha composition mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CompositeAlphaMode {
    /// Backend-selected mode.
    #[default]
    Auto,
    /// Opaque surface.
    Opaque,
    /// Premultiplied alpha.
    PreMultiplied,
    /// Straight alpha.
    PostMultiplied,
}

/// Surface capabilities for an adapter.
#[derive(Clone, Debug, Default)]
pub struct SurfaceCapabilities {
    /// Supported formats.
    pub formats: Vec<TextureFormat>,
    /// Supported present modes.
    pub present_modes: Vec<PresentMode>,
    /// Supported alpha modes.
    pub alpha_modes: Vec<CompositeAlphaMode>,
}

/// Configured surface behavior.
#[derive(Clone, Debug)]
pub struct SurfaceConfiguration {
    /// Texture usage for acquired frames.
    pub usage: TextureUsages,
    /// Frame format.
    pub format: TextureFormat,
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
    /// Presentation mode.
    pub present_mode: PresentMode,
    /// Alpha composition mode.
    pub alpha_mode: CompositeAlphaMode,
    /// Maximum queued frames when supported.
    pub desired_maximum_frame_latency: u32,
}

/// Surface acquisition result.
#[derive(Debug)]
#[non_exhaustive]
pub enum SurfaceFrameStatus {
    /// A normal frame.
    Ready(crate::SurfaceFrame),
    /// A renderable frame whose configuration should be refreshed.
    Suboptimal(crate::SurfaceFrame),
    /// Acquisition timed out.
    Timeout,
    /// Surface is fully occluded.
    Occluded,
    /// Surface must be reconfigured.
    Outdated,
    /// Surface or device was lost.
    Lost,
}

/// Device polling behavior.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PollMode {
    /// Process already completed work without blocking.
    #[default]
    Poll,
    /// Block until all submitted work completes.
    Wait,
}

/// Kind of asynchronous device error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DeviceErrorKind {
    /// API validation failure.
    Validation,
    /// Device ran out of memory.
    OutOfMemory,
    /// Backend-internal failure.
    Internal,
    /// Device was lost.
    DeviceLost,
}

/// Asynchronous device error event.
#[derive(Clone, Debug)]
pub struct DeviceError {
    /// Device that reported the event.
    pub device: DeviceId,
    /// Error category.
    pub kind: DeviceErrorKind,
    /// Backend diagnostic.
    pub message: String,
}

/// Backend-neutral GPU operation error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuError {
    message: String,
}

impl GpuError {
    /// Creates an error with a diagnostic message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for GpuError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for GpuError {}

/// A byte range within a buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BufferRange {
    /// Start offset.
    pub offset: u64,
    /// Optional non-zero length; `None` means the rest of the buffer.
    pub size: Option<NonZeroU64>,
}

impl BufferRange {
    /// Creates a range from standard range bounds.
    pub fn from_range(range: Range<u64>) -> Result<Self, GpuError> {
        let size = range
            .end
            .checked_sub(range.start)
            .and_then(NonZeroU64::new)
            .ok_or_else(|| GpuError::new("buffer range must be non-empty and ordered"))?;
        Ok(Self {
            offset: range.start,
            size: Some(size),
        })
    }
}
