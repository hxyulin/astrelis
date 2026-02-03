//! GPU state management for chart rendering.
//!
//! This module provides efficient GPU buffer management for charts:
//! - Instance buffers for line segments
//! - Partial buffer updates for streaming data
//! - Transform uniforms for pan/zoom
//!
//! # Architecture
//!
//! ```text
//! ChartGpuState
//!   ├── ChartTransform (uniform buffer)
//!   │     └── view_matrix, data_range, etc.
//!   ├── SeriesGpuBuffers (per series)
//!   │     ├── line_vertices
//!   │     ├── marker_vertices
//!   │     └── dirty_ranges
//!   └── grid_vertices
//! ```

use super::cache::ChartDirtyFlags;
use super::rect::Rect;
use super::streaming::PrepareResult;
use super::types::{Chart, DataPoint};
use astrelis_render::{wgpu, Color, GraphicsContext};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec2};
use std::sync::Arc;

/// GPU vertex for line rendering.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LineVertex {
    /// Position (x, y)
    pub position: [f32; 2],
    /// Color (r, g, b, a)
    pub color: [f32; 4],
    /// Line direction for thickness calculation
    pub direction: [f32; 2],
    /// Thickness
    pub thickness: f32,
    /// Padding for alignment
    _padding: f32,
}

impl LineVertex {
    /// Create a new line vertex.
    pub fn new(position: Vec2, color: Color, direction: Vec2, thickness: f32) -> Self {
        Self {
            position: position.to_array(),
            color: [color.r, color.g, color.b, color.a],
            direction: direction.to_array(),
            thickness,
            _padding: 0.0,
        }
    }
}

/// GPU vertex for marker rendering.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct MarkerVertex {
    /// Position (x, y)
    pub position: [f32; 2],
    /// Color (r, g, b, a)
    pub color: [f32; 4],
    /// Size
    pub size: f32,
    /// Shape type (0 = circle, 1 = square, etc.)
    pub shape: u32,
}

impl MarkerVertex {
    /// Create a new marker vertex.
    pub fn new(position: Vec2, color: Color, size: f32, shape: u32) -> Self {
        Self {
            position: position.to_array(),
            color: [color.r, color.g, color.b, color.a],
            size,
            shape,
        }
    }
}

/// Chart transform uniform data.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ChartTransform {
    /// View-projection matrix
    pub view_proj: [[f32; 4]; 4],
    /// Data range (x_min, x_max, y_min, y_max)
    pub data_range: [f32; 4],
    /// Plot area in screen coords (x, y, width, height)
    pub plot_area: [f32; 4],
    /// Viewport size
    pub viewport_size: [f32; 2],
    /// Padding
    _padding: [f32; 2],
}

impl Default for ChartTransform {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            data_range: [0.0, 1.0, 0.0, 1.0],
            plot_area: [0.0, 0.0, 1.0, 1.0],
            viewport_size: [800.0, 600.0],
            _padding: [0.0; 2],
        }
    }
}

impl ChartTransform {
    /// Create a transform from chart state.
    pub fn from_chart(chart: &Chart, bounds: &Rect, viewport_size: Vec2) -> Self {
        let (x_min, x_max) = chart.x_range();
        let (y_min, y_max) = chart.y_range();

        // Create orthographic projection
        let view_proj = Mat4::orthographic_rh(
            0.0,
            viewport_size.x,
            viewport_size.y,
            0.0,
            -1.0,
            1.0,
        );

        Self {
            view_proj: view_proj.to_cols_array_2d(),
            data_range: [x_min as f32, x_max as f32, y_min as f32, y_max as f32],
            plot_area: [bounds.x, bounds.y, bounds.width, bounds.height],
            viewport_size: viewport_size.to_array(),
            _padding: [0.0; 2],
        }
    }

    /// Convert data coordinates to normalized coordinates.
    pub fn data_to_normalized(&self, x: f64, y: f64) -> Vec2 {
        let x_norm = ((x as f32 - self.data_range[0]) / (self.data_range[1] - self.data_range[0]))
            .clamp(0.0, 1.0);
        let y_norm = ((y as f32 - self.data_range[2]) / (self.data_range[3] - self.data_range[2]))
            .clamp(0.0, 1.0);
        Vec2::new(x_norm, 1.0 - y_norm) // Flip Y for screen coords
    }

    /// Convert normalized coordinates to screen coordinates.
    pub fn normalized_to_screen(&self, normalized: Vec2) -> Vec2 {
        Vec2::new(
            self.plot_area[0] + normalized.x * self.plot_area[2],
            self.plot_area[1] + normalized.y * self.plot_area[3],
        )
    }

    /// Convert data coordinates to screen coordinates.
    pub fn data_to_screen(&self, x: f64, y: f64) -> Vec2 {
        let normalized = self.data_to_normalized(x, y);
        self.normalized_to_screen(normalized)
    }
}

/// Range of dirty data that needs GPU upload.
#[derive(Debug, Clone, Copy, Default)]
pub struct DirtyRange {
    /// Start index (inclusive)
    pub start: usize,
    /// End index (exclusive)
    pub end: usize,
}

impl DirtyRange {
    /// Create a new dirty range.
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Create a range covering everything.
    pub fn all(len: usize) -> Self {
        Self { start: 0, end: len }
    }

    /// Check if this range is empty.
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    /// Get the number of elements in the range.
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Merge with another range.
    pub fn merge(&self, other: &DirtyRange) -> DirtyRange {
        if self.is_empty() {
            *other
        } else if other.is_empty() {
            *self
        } else {
            DirtyRange {
                start: self.start.min(other.start),
                end: self.end.max(other.end),
            }
        }
    }
}

/// GPU buffers for a single series.
#[derive(Debug)]
#[derive(Default)]
pub struct SeriesGpuBuffers {
    /// Vertex buffer for line segments
    pub line_buffer: Option<wgpu::Buffer>,
    /// Current line vertex count
    pub line_vertex_count: usize,
    /// Vertex buffer for markers
    pub marker_buffer: Option<wgpu::Buffer>,
    /// Current marker vertex count
    pub marker_vertex_count: usize,
    /// Dirty range for partial updates
    pub dirty_range: Option<DirtyRange>,
    /// Data version when buffers were last updated
    pub data_version: u64,
}


/// GPU state for chart rendering.
///
/// Manages all GPU resources needed for efficient chart rendering:
/// - Transform uniform buffer
/// - Per-series vertex buffers
/// - Grid line buffers
pub struct ChartGpuState {
    /// Graphics context
    context: Arc<GraphicsContext>,
    /// Transform uniform buffer
    transform_buffer: wgpu::Buffer,
    /// Transform bind group layout
    transform_bind_group_layout: wgpu::BindGroupLayout,
    /// Transform bind group
    transform_bind_group: wgpu::BindGroup,
    /// Per-series GPU buffers
    series_buffers: Vec<SeriesGpuBuffers>,
    /// Grid line buffer
    grid_buffer: Option<wgpu::Buffer>,
    /// Grid vertex count
    grid_vertex_count: usize,
    /// Current transform data
    current_transform: ChartTransform,
    /// Version counter for tracking changes
    version: u64,
}

impl ChartGpuState {
    /// Create a new GPU state.
    pub fn new(context: Arc<GraphicsContext>) -> Self {
        let device = context.device();

        // Create transform uniform buffer
        let transform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Chart Transform Buffer"),
            size: std::mem::size_of::<ChartTransform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let transform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Chart Transform Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        // Create bind group
        let transform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Chart Transform Bind Group"),
            layout: &transform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: transform_buffer.as_entire_binding(),
            }],
        });

        Self {
            context,
            transform_buffer,
            transform_bind_group_layout,
            transform_bind_group,
            series_buffers: Vec::new(),
            grid_buffer: None,
            grid_vertex_count: 0,
            current_transform: ChartTransform::default(),
            version: 0,
        }
    }

    /// Update GPU state from chart and cache.
    pub fn update(
        &mut self,
        chart: &Chart,
        bounds: &Rect,
        viewport_size: Vec2,
        dirty_flags: ChartDirtyFlags,
    ) {
        // Update transform if view changed
        if dirty_flags.intersects(
            ChartDirtyFlags::VIEW_CHANGED
                | ChartDirtyFlags::BOUNDS_CHANGED
                | ChartDirtyFlags::AXES_CHANGED,
        ) {
            self.current_transform = ChartTransform::from_chart(chart, bounds, viewport_size);
            let queue = self.context.queue();
            queue.write_buffer(
                &self.transform_buffer,
                0,
                bytemuck::bytes_of(&self.current_transform),
            );
        }

        // Ensure we have enough series buffers
        while self.series_buffers.len() < chart.series.len() {
            self.series_buffers.push(SeriesGpuBuffers::default());
        }

        // Update series buffers
        let needs_data_update = dirty_flags.intersects(
            ChartDirtyFlags::DATA_CHANGED | ChartDirtyFlags::DATA_APPENDED,
        );

        if needs_data_update {
            // Clone transform to avoid borrow issues
            let transform = self.current_transform;
            for (idx, series) in chart.series.iter().enumerate() {
                self.update_series_buffer(idx, &series.data, &transform);
            }
        }

        self.version = self.version.wrapping_add(1);
    }

    /// Update GPU state from streaming chart prepare result.
    pub fn update_from_prepare_result(
        &mut self,
        chart: &Chart,
        bounds: &Rect,
        viewport_size: Vec2,
        result: &PrepareResult,
    ) {
        // Always update transform
        self.current_transform = ChartTransform::from_chart(chart, bounds, viewport_size);
        {
            let queue = self.context.queue();
            queue.write_buffer(
                &self.transform_buffer,
                0,
                bytemuck::bytes_of(&self.current_transform),
            );
        }

        // Ensure we have enough series buffers
        while self.series_buffers.len() < chart.series.len() {
            self.series_buffers.push(SeriesGpuBuffers::default());
        }

        // Clone transform to avoid borrow issues
        let transform = self.current_transform;

        // Update only series that changed
        for update in &result.series_updates {
            if update.full_rebuild || update.new_points > 0 {
                let series = &chart.series[update.index];
                self.update_series_buffer(update.index, &series.data, &transform);
            }
        }

        self.version = self.version.wrapping_add(1);
    }

    /// Update a single series buffer.
    fn update_series_buffer(
        &mut self,
        series_idx: usize,
        data: &[DataPoint],
        transform: &ChartTransform,
    ) {
        if data.len() < 2 {
            self.series_buffers[series_idx].line_vertex_count = 0;
            return;
        }

        // Generate line vertices
        let mut vertices = Vec::with_capacity(data.len() * 2);

        for i in 0..data.len() - 1 {
            let p0 = transform.data_to_screen(data[i].x, data[i].y);
            let p1 = transform.data_to_screen(data[i + 1].x, data[i + 1].y);

            let dir = (p1 - p0).normalize_or_zero();

            vertices.push(LineVertex::new(p0, Color::WHITE, dir, 1.0));
            vertices.push(LineVertex::new(p1, Color::WHITE, dir, 1.0));
        }

        let device = self.context.device();
        let queue = self.context.queue();

        let buffer_size = (vertices.len() * std::mem::size_of::<LineVertex>()) as u64;

        // Check if we need to recreate the buffer
        let needs_new_buffer = self.series_buffers[series_idx]
            .line_buffer
            .as_ref()
            .is_none_or(|b| b.size() < buffer_size);

        if needs_new_buffer {
            // Create new buffer with some extra capacity
            let capacity = (buffer_size * 3 / 2).max(4096);
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("Chart Series {} Line Buffer", series_idx)),
                size: capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.series_buffers[series_idx].line_buffer = Some(buffer);
        }

        // Write data
        if let Some(buffer) = &self.series_buffers[series_idx].line_buffer {
            queue.write_buffer(buffer, 0, bytemuck::cast_slice(&vertices));
        }

        self.series_buffers[series_idx].line_vertex_count = vertices.len();
    }

    /// Get the transform bind group.
    pub fn transform_bind_group(&self) -> &wgpu::BindGroup {
        &self.transform_bind_group
    }

    /// Get the transform bind group layout.
    pub fn transform_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.transform_bind_group_layout
    }

    /// Get the current transform.
    pub fn transform(&self) -> &ChartTransform {
        &self.current_transform
    }

    /// Get series buffers.
    pub fn series_buffers(&self) -> &[SeriesGpuBuffers] {
        &self.series_buffers
    }

    /// Get the version counter.
    pub fn version(&self) -> u64 {
        self.version
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chart_transform() {
        let transform = ChartTransform {
            data_range: [0.0, 100.0, 0.0, 100.0],
            plot_area: [50.0, 50.0, 400.0, 300.0],
            ..Default::default()
        };

        // Test data to normalized conversion
        let norm = transform.data_to_normalized(50.0, 50.0);
        assert!((norm.x - 0.5).abs() < 0.001);
        assert!((norm.y - 0.5).abs() < 0.001);

        // Test normalized to screen conversion
        let screen = transform.normalized_to_screen(Vec2::new(0.5, 0.5));
        assert!((screen.x - 250.0).abs() < 0.001);
        assert!((screen.y - 200.0).abs() < 0.001);
    }

    #[test]
    fn test_dirty_range() {
        let r1 = DirtyRange::new(10, 20);
        let r2 = DirtyRange::new(15, 30);

        let merged = r1.merge(&r2);
        assert_eq!(merged.start, 10);
        assert_eq!(merged.end, 30);
        assert_eq!(merged.len(), 20);
    }
}
