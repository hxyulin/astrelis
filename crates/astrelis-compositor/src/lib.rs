//! Ordered composition of backend-independent UI and application scene passes.

#![warn(missing_docs)]

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
};

use astrelis_core::{
    color::Color,
    geometry::{Physical, Point, Rect, Size},
    math::Vec2,
};
use astrelis_gpu as gpu;
use astrelis_paint::{CompositorMarker, CompositorViewId, DisplayList, ExternalImage};
use astrelis_paint_gpu::{
    RenderStats as PaintStats, RenderTarget as PaintTarget, Renderer as PaintRenderer,
};
use astrelis_render::{CompositedRenderTarget, RenderTarget};

/// Destination selected for one registered scene callback.
#[derive(Clone, Debug)]
pub enum ViewRenderTarget {
    /// The scene writes into a scissored region of the shared frame attachment.
    Direct(CompositedRenderTarget),
    /// The scene writes into a compositor-managed texture which UI then samples.
    Texture(RenderTarget),
}

/// Per-view composition policy supplied alongside its callback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewOptions {
    /// Linear background painted before the scene.
    pub clear_color: Color,
}

impl Default for ViewOptions {
    fn default() -> Self {
        Self {
            clear_color: Color::TRANSPARENT,
        }
    }
}

/// Aggregate statistics for one composed frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompositionStats {
    /// UI layers recorded, including empty layers needed to resolve MSAA.
    pub ui_layers: u32,
    /// Scene views rendered directly into the frame.
    pub direct_views: u32,
    /// Scene views rendered through managed fallback textures.
    pub texture_views: u32,
    /// Accumulated UI painter statistics.
    pub paint: PaintStats,
}

/// Composition failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompositionError(String);

impl fmt::Display for CompositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl Error for CompositionError {}

struct Fallback {
    _texture: gpu::Texture,
    view: gpu::TextureView,
    image: ExternalImage,
    allocation: Size<Physical, u32>,
}

/// Device-bound UI/scene compositor.
pub struct Compositor {
    device: gpu::Device,
    paint: PaintRenderer,
    fallbacks: HashMap<CompositorViewId, Fallback>,
}

impl Compositor {
    /// Creates a compositor around an existing GPU paint renderer.
    pub fn new(device: gpu::Device, paint: PaintRenderer) -> Self {
        Self {
            device,
            paint,
            fallbacks: HashMap::new(),
        }
    }

    /// Mutable access to the embedded GPU painter for image registration and caches.
    pub fn paint_mut(&mut self) -> &mut PaintRenderer {
        &mut self.paint
    }

    /// Composes one display list and invokes each unique scene callback in paint order.
    pub fn render<E>(
        &mut self,
        encoder: &mut gpu::CommandEncoder,
        list: &DisplayList,
        target: PaintTarget,
        mut view_options: impl FnMut(CompositorViewId) -> ViewOptions,
        mut render_view: impl FnMut(
            CompositorViewId,
            &mut gpu::CommandEncoder,
            ViewRenderTarget,
        ) -> Result<(), E>,
    ) -> Result<CompositionStats, CompositionError>
    where
        E: fmt::Display,
    {
        astrelis_profiling::profile_scope!("compositor.render");
        let plan = list.composition_plan();
        let mut unique = HashSet::new();
        for marker in &plan.markers {
            if !unique.insert(marker.id) {
                return Err(CompositionError(format!(
                    "duplicate compositor view id {}",
                    marker.id.get()
                )));
            }
        }
        let mut stats = CompositionStats::default();
        for (index, layer) in plan.layers.iter().enumerate() {
            let last = index + 1 == plan.layers.len();
            let paint = if plan.markers.is_empty() {
                self.paint.render(encoder, layer, target.clone())
            } else if index == 0 {
                self.paint
                    .render_first_layer(encoder, layer, target.clone())
            } else if last {
                self.paint
                    .render_final_layer(encoder, layer, target.clone())
            } else {
                self.paint.render_layer(encoder, layer, target.clone())
            }
            .map_err(|error| CompositionError(error.to_string()))?;
            add_paint_stats(&mut stats.paint, paint);
            stats.ui_layers += 1;
            let Some(marker) = plan.markers.get(index) else {
                continue;
            };
            let transformed = transformed_rect(marker, target.scale_factor);
            if transformed.size.width <= 0.0 || transformed.size.height <= 0.0 {
                continue;
            }
            let options = view_options(marker.id);
            if let Some((viewport, scissor)) =
                direct_region(marker, target.size, target.scale_factor)
            {
                let clear = list.compositor_clear_layer(index, options.clear_color);
                self.paint
                    .render_layer(encoder, &clear, target.clone())
                    .map_err(|error| CompositionError(error.to_string()))?;
                let view = self
                    .paint
                    .compositor_color_view(&target)
                    .map_err(|error| CompositionError(error.to_string()))?;
                let direct = CompositedRenderTarget {
                    view,
                    size: target.size,
                    viewport,
                    scissor,
                    scale_factor: target.scale_factor,
                    clear_color: options.clear_color,
                };
                render_view(marker.id, encoder, ViewRenderTarget::Direct(direct))
                    .map_err(|error| CompositionError(error.to_string()))?;
                stats.direct_views += 1;
            } else {
                let desired = transformed_size(marker, target.scale_factor);
                self.ensure_fallback(marker.id, desired, target.format)?;
                let fallback = self.fallbacks.get(&marker.id).expect("fallback exists");
                render_view(
                    marker.id,
                    encoder,
                    ViewRenderTarget::Texture(RenderTarget {
                        view: fallback.view.clone(),
                        allocation_size: fallback.allocation,
                        render_size: desired,
                        scale_factor: target.scale_factor,
                        clear_color: options.clear_color,
                    }),
                )
                .map_err(|error| CompositionError(error.to_string()))?;
                let fallback_layer = list
                    .compositor_fallback_layer(index, fallback.image.clone(), desired)
                    .map_err(|error| CompositionError(error.to_string()))?;
                self.paint
                    .render_layer(encoder, &fallback_layer, target.clone())
                    .map_err(|error| CompositionError(error.to_string()))?;
                stats.texture_views += 1;
            }
        }
        Ok(stats)
    }

    fn ensure_fallback(
        &mut self,
        id: CompositorViewId,
        desired: Size<Physical, u32>,
        format: gpu::TextureFormat,
    ) -> Result<(), CompositionError> {
        let bucket = Size::new(
            desired.width.max(1).div_ceil(64) * 64,
            desired.height.max(1).div_ceil(64) * 64,
        );
        let allocation = self.fallbacks.get(&id).map_or(bucket, |current| {
            let fits = desired.width <= current.allocation.width
                && desired.height <= current.allocation.height;
            let should_shrink = (desired.width as f32) < current.allocation.width as f32 * 0.75
                || (desired.height as f32) < current.allocation.height as f32 * 0.75;
            if fits && !should_shrink {
                current.allocation
            } else {
                bucket
            }
        });
        if self
            .fallbacks
            .get(&id)
            .is_some_and(|value| value.allocation == allocation)
        {
            return Ok(());
        }
        if let Some(old) = self.fallbacks.remove(&id) {
            self.paint.unregister_external_image(&old.image);
        }
        let texture = self.device.create_texture(gpu::TextureDescriptor {
            label: Some("compositor render-view fallback".into()),
            size: gpu::Extent3d::d2(allocation.width, allocation.height),
            mip_level_count: 1,
            sample_count: 1,
            dimension: gpu::TextureDimension::D2,
            format,
            usage: gpu::TextureUsages::RENDER_ATTACHMENT | gpu::TextureUsages::TEXTURE_BINDING,
        });
        let view = texture.create_view(Default::default());
        let image =
            ExternalImage::new(allocation).map_err(|error| CompositionError(error.to_string()))?;
        self.paint
            .register_external_image(&image, view.clone())
            .map_err(|error| CompositionError(error.to_string()))?;
        self.fallbacks.insert(
            id,
            Fallback {
                _texture: texture,
                view,
                image,
                allocation,
            },
        );
        Ok(())
    }
}

fn direct_region(
    marker: &CompositorMarker,
    frame: Size<Physical, u32>,
    scale: f32,
) -> Option<(Rect<Physical, u32>, Rect<Physical, u32>)> {
    if !marker.prefer_direct || marker.has_complex_clip {
        return None;
    }
    let columns = marker.transform.to_cols_array();
    if columns[0] <= 0.0
        || columns[3] <= 0.0
        || columns[1].abs() > f32::EPSILON
        || columns[2].abs() > f32::EPSILON
    {
        return None;
    }
    let rect = transformed_rect(marker, scale);
    if rect.origin.x < 0.0
        || rect.origin.y < 0.0
        || rect.origin.x + rect.size.width > frame.width as f32
        || rect.origin.y + rect.size.height > frame.height as f32
    {
        return None;
    }
    let destination_edges = [
        rect.origin.x,
        rect.origin.y,
        rect.origin.x + rect.size.width,
        rect.origin.y + rect.size.height,
    ];
    if destination_edges
        .iter()
        .any(|value| value.fract().abs() > f32::EPSILON)
    {
        return None;
    }
    let viewport = Rect::new(
        Point::new(rect.origin.x as u32, rect.origin.y as u32),
        Size::new(rect.size.width as u32, rect.size.height as u32),
    );
    let mut x0 = rect.origin.x.max(0.0).ceil() as u32;
    let mut y0 = rect.origin.y.max(0.0).ceil() as u32;
    let mut x1 = (rect.origin.x + rect.size.width)
        .min(frame.width as f32)
        .floor() as u32;
    let mut y1 = (rect.origin.y + rect.size.height)
        .min(frame.height as f32)
        .floor() as u32;
    for (clip, transform) in &marker.rectangular_clips {
        let clip = transform_rect(*clip, *transform, scale);
        let edges = [
            clip.origin.x,
            clip.origin.y,
            clip.origin.x + clip.size.width,
            clip.origin.y + clip.size.height,
        ];
        if edges.iter().any(|value| value.fract().abs() > f32::EPSILON) {
            return None;
        }
        x0 = x0.max(clip.origin.x.ceil().max(0.0) as u32);
        y0 = y0.max(clip.origin.y.ceil().max(0.0) as u32);
        x1 = x1.min((clip.origin.x + clip.size.width).floor().max(0.0) as u32);
        y1 = y1.min((clip.origin.y + clip.size.height).floor().max(0.0) as u32);
    }
    (x1 > x0 && y1 > y0).then(|| {
        (
            viewport,
            Rect::new(Point::new(x0, y0), Size::new(x1 - x0, y1 - y0)),
        )
    })
}

fn transformed_rect(marker: &CompositorMarker, scale: f32) -> Rect<Physical> {
    transform_rect(marker.destination, marker.transform, scale)
}
fn transform_rect(
    rect: astrelis_core::geometry::LogicalRect,
    transform: astrelis_core::math::Affine2,
    scale: f32,
) -> Rect<Physical> {
    let a = transform.transform_point2(Vec2::new(rect.origin.x, rect.origin.y)) * scale;
    let b = transform.transform_point2(Vec2::new(rect.max_x(), rect.max_y())) * scale;
    Rect::from_xywh(
        a.x.min(b.x),
        a.y.min(b.y),
        (b.x - a.x).abs(),
        (b.y - a.y).abs(),
    )
}
fn transformed_size(marker: &CompositorMarker, scale: f32) -> Size<Physical, u32> {
    let rect = transformed_rect(marker, scale);
    Size::new(
        rect.size.width.ceil().max(1.0) as u32,
        rect.size.height.ceil().max(1.0) as u32,
    )
}
fn add_paint_stats(total: &mut PaintStats, value: PaintStats) {
    total.draws += value.draws;
    total.triangles += value.triangles;
    total.mesh_cache_hits += value.mesh_cache_hits;
    total.mesh_cache_misses += value.mesh_cache_misses;
    total.image_cache_hits += value.image_cache_hits;
    total.image_cache_misses += value.image_cache_misses;
    total.gradient_cache_hits += value.gradient_cache_hits;
    total.gradient_cache_misses += value.gradient_cache_misses;
    total.glyph_cache_hits += value.glyph_cache_hits;
    total.glyph_cache_misses += value.glyph_cache_misses;
    total.glyph_uploads += value.glyph_uploads;
}

#[cfg(test)]
mod tests {
    use super::*;
    use astrelis_core::{geometry::Rect, math::Affine2};

    fn marker() -> CompositorMarker {
        CompositorMarker {
            id: CompositorViewId::new(),
            destination: Rect::from_xywh(10.0, 20.0, 100.0, 50.0),
            transform: Affine2::IDENTITY,
            rectangular_clips: Vec::new(),
            has_complex_clip: false,
            prefer_direct: true,
        }
    }

    #[test]
    fn exact_rectangles_select_direct_regions() {
        assert_eq!(
            direct_region(&marker(), Size::new(200, 100), 1.0),
            Some((
                Rect::from_xywh(10, 20, 100, 50),
                Rect::from_xywh(10, 20, 100, 50)
            ))
        );
    }

    #[test]
    fn complex_clips_and_rotation_select_fallback() {
        let mut value = marker();
        value.has_complex_clip = true;
        assert!(direct_region(&value, Size::new(200, 100), 1.0).is_none());
        value.has_complex_clip = false;
        value.transform = Affine2::from_angle(0.1);
        assert!(direct_region(&value, Size::new(200, 100), 1.0).is_none());
    }
}
