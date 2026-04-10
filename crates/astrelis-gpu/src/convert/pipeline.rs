//! Pipeline-related type conversions.

use crate::pipeline::*;

use super::types;

pub(crate) fn primitive_state(s: &PrimitiveState) -> wgpu::PrimitiveState {
    wgpu::PrimitiveState {
        topology: types::primitive_topology(s.topology),
        strip_index_format: s.strip_index_format.map(types::index_format),
        front_face: types::front_face(s.front_face),
        cull_mode: types::cull_mode(s.cull_mode),
        polygon_mode: types::polygon_mode(s.polygon_mode),
        unclipped_depth: s.unclipped_depth,
        conservative: false,
    }
}

pub(crate) fn depth_stencil_state(s: &DepthStencilState) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: types::texture_format(s.format),
        depth_write_enabled: Some(s.depth_write_enabled),
        depth_compare: Some(types::compare_function(s.depth_compare)),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

pub(crate) fn multisample_state(s: &MultisampleState) -> wgpu::MultisampleState {
    wgpu::MultisampleState {
        count: s.count,
        mask: s.mask,
        alpha_to_coverage_enabled: s.alpha_to_coverage_enabled,
    }
}

pub(crate) fn color_target_state(s: &ColorTargetState) -> wgpu::ColorTargetState {
    wgpu::ColorTargetState {
        format: types::texture_format(s.format),
        blend: s.blend.as_ref().map(types::blend_state),
        write_mask: types::color_writes(s.write_mask),
    }
}

pub(crate) fn vertex_attribute(a: &VertexAttribute) -> wgpu::VertexAttribute {
    wgpu::VertexAttribute {
        format: types::vertex_format(a.format),
        offset: a.offset,
        shader_location: a.shader_location,
    }
}
