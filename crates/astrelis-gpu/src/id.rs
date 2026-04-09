//! Typed resource handle markers and aliases.
//!
//! Each GPU resource type gets a unique marker so that [`Id<T>`] handles
//! cannot be accidentally mixed across resource kinds.

use astrelis_core::id::Id;

/// Marker for [`BufferId`].
pub struct BufferMarker;
/// Marker for [`TextureId`].
pub struct TextureMarker;
/// Marker for [`TextureViewId`].
pub struct TextureViewMarker;
/// Marker for [`SamplerId`].
pub struct SamplerMarker;
/// Marker for [`ShaderModuleId`].
pub struct ShaderModuleMarker;
/// Marker for [`BindGroupLayoutId`].
pub struct BindGroupLayoutMarker;
/// Marker for [`BindGroupId`].
pub struct BindGroupMarker;
/// Marker for [`PipelineLayoutId`].
pub struct PipelineLayoutMarker;
/// Marker for [`RenderPipelineId`].
pub struct RenderPipelineMarker;
/// Marker for [`ComputePipelineId`].
pub struct ComputePipelineMarker;

/// Handle to a GPU buffer.
pub type BufferId = Id<BufferMarker>;
/// Handle to a GPU texture.
pub type TextureId = Id<TextureMarker>;
/// Handle to a GPU texture view.
pub type TextureViewId = Id<TextureViewMarker>;
/// Handle to a GPU sampler.
pub type SamplerId = Id<SamplerMarker>;
/// Handle to a compiled shader module.
pub type ShaderModuleId = Id<ShaderModuleMarker>;
/// Handle to a bind group layout.
pub type BindGroupLayoutId = Id<BindGroupLayoutMarker>;
/// Handle to a bind group.
pub type BindGroupId = Id<BindGroupMarker>;
/// Handle to a pipeline layout.
pub type PipelineLayoutId = Id<PipelineLayoutMarker>;
/// Handle to a render pipeline.
pub type RenderPipelineId = Id<RenderPipelineMarker>;
/// Handle to a compute pipeline.
pub type ComputePipelineId = Id<ComputePipelineMarker>;
