use crate::{
    Error, GraphicsContext, LineDraw, LineRenderer, PreparedText, Rect, RenderFormat, RenderPass,
    ShapeDraw, ShapeRenderer, Stroke, TextDraw, TextLayout, TextPreparation, TextRenderError,
    TextRenderer, TextureBinding, TextureBindingOptions, TextureDraw, TextureRenderer, Transform2D,
};

/// Reusable 2D drawing resources composed from independent primitive/image/text renderers.
///
/// Create once per device and retain across frames. [`Self::begin`] borrows an
/// application-owned pass; it never acquires a frame, clears attachments, submits,
/// or presents. Calls record immediately in caller order. There is no pending
/// command list and no implicit sorting/batching. Use explicit batch methods when
/// consecutive compatible draws can be instanced. The underlying renderers remain
/// independently usable, and custom GPU work fits through [`PaintSession::pass`].
///
/// Colors, coordinates, transforms, caps, coverage, and errors follow [`ShapeDraw`],
/// [`LineDraw`], [`TextureDraw`], and [`TextDraw`]. Text shaping and rasterization
/// are explicit: prepare a [`TextLayout`] before [`Self::prepare_text`], retain the
/// returned [`PreparedText`], and reuse it across sessions. Prepare variants before
/// first use to avoid pipeline creation while recording. After warm-up, a fixed batch workload reuses
/// GPU pages and CPU scratch; transformed batches retain capacity up to their peak.
///
/// ```no_run
/// use astrelis::{Painter, PreparedText, RenderPass, TextDraw, TextLayout,
///     TextRasterOptions, TextRenderError, Transform2D};
/// // Call when layout/content/DPI changes, before entering a painting session.
/// fn prepare(painter: &mut Painter, layout: &TextLayout, dpi: f32)
///     -> Result<PreparedText, TextRenderError>
/// {
///     painter.prepare_text(layout, TextRasterOptions::new().scale_factor(dpi))
/// }
/// fn paint(painter: &mut Painter, pass: &mut RenderPass<'_>, text: &PreparedText)
///     -> Result<(), Box<dyn std::error::Error>>
/// {
///     let mut paint = painter.begin(pass)?;
///     let mut local = paint.transformed(Transform2D::translation(20., 30.))?;
///     local.draw_text(text, TextDraw::default().color([0.8, 0.9, 1., 1.]))?;
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct Painter {
    graphics: GraphicsContext,
    shapes: ShapeRenderer,
    lines: LineRenderer,
    textures: TextureRenderer,
    text: TextRenderer,
    shape_scratch: Vec<ShapeDraw>,
    line_scratch: Vec<LineDraw>,
    image_scratch: Vec<TextureDraw>,
}
impl Painter {
    /// Creates independent shape, line, image, and text renderers with empty pipeline caches.
    pub fn new(graphics: &GraphicsContext) -> Self {
        Self {
            graphics: graphics.clone(),
            shapes: ShapeRenderer::new(graphics),
            lines: LineRenderer::new(graphics),
            textures: TextureRenderer::new(graphics),
            text: TextRenderer::new(graphics),
            shape_scratch: Vec::new(),
            line_scratch: Vec::new(),
            image_scratch: Vec::new(),
        }
    }
    /// Prepares default primitive and text pipelines for this attachment format.
    /// Does not shape text, rasterize glyphs, or upload prepared text geometry.
    /// Image variants also depend on a source binding; use [`Self::prepare_image`].
    pub fn prepare(&mut self, format: &RenderFormat) -> Result<(), Error> {
        self.shapes.prepare(format)?;
        self.lines.prepare(format)?;
        self.text.prepare_pipeline(format)
    }
    /// Prepares primitive and text pipelines for a surface, rejecting a foreign device.
    pub fn prepare_for_target(&mut self, target: &crate::RenderTarget<'_>) -> Result<(), Error> {
        if !target.graphics().same_device(&self.graphics) {
            return Err(Error::DeviceMismatch);
        }
        self.prepare(&target.render_format())
    }
    /// Prepares a default image variant for the source's alpha/filtering and target format.
    pub fn prepare_image(
        &mut self,
        image: &TextureBinding,
        format: &RenderFormat,
    ) -> Result<(), Error> {
        self.textures.prepare(image, format)
    }
    /// Creates a reusable snapshot image/sampler binding. Placement remains per draw.
    pub fn create_image_binding(
        &self,
        view: &wgpu::TextureView,
        options: TextureBindingOptions,
    ) -> Result<TextureBinding, Error> {
        self.textures.create_binding(view, options)
    }
    /// Creates a live premultiplied framebuffer binding which follows storage replacement.
    pub fn create_sampled_binding(
        &self,
        source: &crate::SampledColor,
    ) -> Result<TextureBinding, Error> {
        self.textures.create_sampled_binding(source)
    }
    /// Borrows the underlying renderer for direct preparation, drawing, or scopes.
    pub fn shapes(&mut self) -> &mut ShapeRenderer {
        &mut self.shapes
    }
    /// Borrows the underlying renderer for direct preparation, drawing, or scopes.
    pub fn lines(&mut self) -> &mut LineRenderer {
        &mut self.lines
    }
    /// Borrows the underlying renderer for custom samplers/materials, prepared data,
    /// live sources, direct drawing, or scopes outside an active painting session.
    pub fn textures(&mut self) -> &mut TextureRenderer {
        &mut self.textures
    }
    /// Borrows the text renderer for pipeline preparation, cache statistics, cache
    /// clearing, or direct drawing outside an active painting session. Prepared texts
    /// from any TextRenderer on this device are also accepted by the session.
    /// To configure atlas budgets, replace this renderer with
    /// [`TextRenderer::with_options`] before preparing text; retained resources keep
    /// their original allocation leases.
    pub fn text(&mut self) -> &mut TextRenderer {
        &mut self.text
    }
    /// Generates missing glyph images and uploads immutable text geometry outside painting.
    /// Pass `TextRasterOptions` for coverage or `MtsdfOptions` for scalable outline fill.
    /// The layout retains its fonts; Painter never borrows or owns a TextSystem.
    /// Raster scale converts layout units to physical pixels exactly once. Retain the
    /// result until content, layout, or preparation settings change; placement/color/opacity
    /// remain per draw. Cache pressure and unsupported glyphs follow
    /// [`TextRenderer::prepare_text`]. This does not prepare an attachment pipeline.
    pub fn prepare_text(
        &mut self,
        layout: &TextLayout,
        preparation: impl Into<TextPreparation>,
    ) -> Result<PreparedText, TextRenderError> {
        self.text.prepare_text(layout, preparation)
    }
    /// Starts immediate painting with identity transform on an existing color pass.
    /// Device/output checks happen here; individual draws validate geometry and resources.
    /// Dropping the session leaves the pass open with its current raster settings.
    pub fn begin<'draw, 'frame>(
        &'draw mut self,
        pass: &'draw mut RenderPass<'frame>,
    ) -> Result<PaintSession<'draw, 'frame>, Error> {
        if !pass.same_device(&self.graphics) {
            return Err(Error::DeviceMismatch);
        }
        pass.single_color_format()?;
        Ok(PaintSession {
            painter: self,
            pass,
            transform: Transform2D::IDENTITY,
        })
    }
}

/// Immediate 2D painting into an exclusively borrowed application pass.
///
/// Geometry retains its own units. The session transform is applied after each
/// draw's transform, in those same units, before viewport conversion. Use pixels
/// plus an explicit scale for application logical coordinates; normalized draws
/// still interpret translations/widths in viewport fractions. [`Self::transformed`]
/// lends a child with additional local geometry transformation, preserving the
/// parent's transform without a mutable save/restore stack. It does not save or
/// restore pass clipping, viewport, or application bindings.
///
/// Pass access records in place with no flush required. Following paint calls
/// restore the state their renderer needs. Application groups and chosen wrapped
/// raster settings remain caller-controlled. Painter does not own stencil clipping
/// policy, layout, windows, text shaping, paths, or a retained display list.
/// The exclusive pass borrow prevents overlapping direct use:
///
/// ```compile_fail
/// use astrelis::{Error, Painter, Rect, RenderPass};
/// fn overlap(painter: &mut Painter, pass: &mut RenderPass<'_>) -> Result<(), Error> {
///     let mut paint = painter.begin(pass)?;
///     pass.set_stencil_reference(1);
///     paint.fill_rect(Rect::new(0.,0.,10.,10.), [1.;4])?;
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct PaintSession<'draw, 'frame> {
    painter: &'draw mut Painter,
    pass: &'draw mut RenderPass<'frame>,
    transform: Transform2D,
}
impl<'frame> PaintSession<'_, 'frame> {
    /// Returns the transform applied after each draw's own transform.
    pub fn transform(&self) -> Transform2D {
        self.transform
    }
    /// Lends a child with `local` applied before the parent's transform. The parent
    /// becomes usable again when the child drops, with its transform unchanged.
    /// Nonfinite or overflowing composition rejects the child without changing state.
    pub fn transformed(
        &mut self,
        local: impl Into<Transform2D>,
    ) -> Result<PaintSession<'_, 'frame>, Error> {
        let local = local.into();
        let transform = local.then(self.transform);
        if !local.valid() || !transform.valid() {
            return Err(Error::InvalidTransform2D);
        }
        Ok(PaintSession {
            painter: self.painter,
            pass: self.pass,
            transform,
        })
    }
    /// Records a filled rectangle in pixels, using linear straight RGBA.
    #[inline]
    pub fn fill_rect(&mut self, rect: Rect, color: [f32; 4]) -> Result<(), Error> {
        self.draw_shape(ShapeDraw::rect(rect, color))
    }
    /// Records a uniformly rounded rectangle; the radius is clamped by its renderer.
    #[inline]
    pub fn fill_rounded_rect(
        &mut self,
        rect: Rect,
        radius: f32,
        color: [f32; 4],
    ) -> Result<(), Error> {
        self.draw_shape(ShapeDraw::rounded_rect(rect, radius, color))
    }
    /// Records an ellipse inscribed in the pixel rectangle.
    #[inline]
    pub fn fill_ellipse(&mut self, rect: Rect, color: [f32; 4]) -> Result<(), Error> {
        self.draw_shape(ShapeDraw::ellipse(rect, color))
    }
    /// Records a sharp-corner rectangle outline in pixels. Width transforms with geometry.
    #[inline]
    pub fn stroke_rect(
        &mut self,
        rect: Rect,
        stroke: Stroke,
        color: [f32; 4],
    ) -> Result<(), Error> {
        self.draw_shape(ShapeDraw::rect(rect, color).stroke(stroke))
    }
    /// Records a rounded rectangle outline with an offset, uniformly rounded boundary.
    #[inline]
    pub fn stroke_rounded_rect(
        &mut self,
        rect: Rect,
        radius: f32,
        stroke: Stroke,
        color: [f32; 4],
    ) -> Result<(), Error> {
        self.draw_shape(ShapeDraw::rounded_rect(rect, radius, color).stroke(stroke))
    }
    /// Records an ellipse outline using a distance offset of the original curve.
    #[inline]
    pub fn stroke_ellipse(
        &mut self,
        rect: Rect,
        stroke: Stroke,
        color: [f32; 4],
    ) -> Result<(), Error> {
        self.draw_shape(ShapeDraw::ellipse(rect, color).stroke(stroke))
    }
    /// Records explicit primitive geometry/space/coverage and its per-draw transform.
    #[inline]
    pub fn draw_shape(&mut self, mut draw: ShapeDraw) -> Result<(), Error> {
        if self.transform != Transform2D::IDENTITY {
            draw.transform = draw.transform.then(self.transform);
        }
        self.painter.shapes.draw(self.pass, draw)
    }
    /// Validates a whole slice before recording ordered primitive instance batches.
    /// Transformed batches reuse Painter-owned CPU scratch; no sorting occurs.
    pub fn draw_shapes(&mut self, draws: &[ShapeDraw]) -> Result<(), Error> {
        if self.transform == Transform2D::IDENTITY {
            return self.painter.shapes.draw_many(self.pass, draws);
        }
        self.painter.shape_scratch.clear();
        self.painter
            .shape_scratch
            .extend(draws.iter().map(|d| ShapeDraw {
                transform: d.transform.then(self.transform),
                ..*d
            }));
        self.painter
            .shapes
            .draw_many(self.pass, &self.painter.shape_scratch)
    }
    /// Records an independent segment with explicit width/caps/coverage and transform.
    #[inline]
    pub fn draw_line(&mut self, mut draw: LineDraw) -> Result<(), Error> {
        if self.transform != Transform2D::IDENTITY {
            draw.transform = draw.transform.then(self.transform);
        }
        self.painter.lines.draw(self.pass, draw)
    }
    /// Validates a whole slice before recording ordered line instance batches.
    pub fn draw_lines(&mut self, draws: &[LineDraw]) -> Result<(), Error> {
        if self.transform == Transform2D::IDENTITY {
            return self.painter.lines.draw_many(self.pass, draws);
        }
        self.painter.line_scratch.clear();
        self.painter
            .line_scratch
            .extend(draws.iter().map(|d| LineDraw {
                transform: d.transform.then(self.transform),
                ..*d
            }));
        self.painter
            .lines
            .draw_many(self.pass, &self.painter.line_scratch)
    }
    /// Records an image with explicit placement, UVs, tint, and transform.
    /// The binding is reusable and can track a live framebuffer source.
    #[inline]
    pub fn draw_image(
        &mut self,
        image: &TextureBinding,
        mut draw: TextureDraw,
    ) -> Result<(), Error> {
        if self.transform != Transform2D::IDENTITY {
            draw.transform = Transform2D::from(draw.transform)
                .then(self.transform)
                .to_array();
        }
        self.painter.textures.draw(self.pass, image, draw)
    }
    /// Records retained text immediately in caller order with shapes, images, lines,
    /// and custom pass work. No shaping, rasterization, atlas writes, or glyph geometry
    /// uploads occur here. The draw's origin and transform use physical pixels relative
    /// to the viewport; the session transform is applied after the draw's transform.
    /// DPI has already been applied during preparation: scaling a logical-coordinate
    /// session also scales these physical glyphs. Color tints coverage glyphs; intrinsic
    /// color glyphs retain their RGB. Color alpha and opacity affect both kinds.
    /// Invalid draws record no text commands and leave the session usable.
    #[inline]
    pub fn draw_text(
        &mut self,
        text: &PreparedText,
        mut draw: TextDraw,
    ) -> Result<(), TextRenderError> {
        if self.transform != Transform2D::IDENTITY {
            draw.transform = draw.transform.then(self.transform);
        }
        self.painter.text.draw(self.pass, text, draw)
    }
    /// Validates a whole slice, then instances ordered placements of one image.
    pub fn draw_images(
        &mut self,
        image: &TextureBinding,
        draws: &[TextureDraw],
    ) -> Result<(), Error> {
        if self.transform == Transform2D::IDENTITY {
            return self.painter.textures.draw_many(self.pass, image, draws);
        }
        self.painter.image_scratch.clear();
        self.painter.image_scratch.extend(
            draws
                .iter()
                .map(|d| d.transform_2d(Transform2D::from(d.transform).then(self.transform))),
        );
        self.painter
            .textures
            .draw_many(self.pass, image, &self.painter.image_scratch)
    }
    /// Borrows the existing pass for wrapped clipping/viewport changes, application
    /// bindings, other renderers, or raw commands. Geometry transforms affect only
    /// Painter draws; this access forwards the pass without applying a transform.
    pub fn pass(&mut self) -> &mut RenderPass<'frame> {
        self.pass
    }
}

#[cfg(test)]
#[path = "painter_tests.rs"]
mod tests;
