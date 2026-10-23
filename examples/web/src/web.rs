use astrelis::{
    FrameError, GraphicsContext, LineDraw, Painter, Path, PathDraw, PathOptions, PreparedPath,
    PreparedText, Rect, RenderTarget, SurfaceOptions, TextBuffer, TextDraw, TextRasterOptions,
    TextStyle, TextSystem, TextureBinding, TextureBindingOptions, TextureDraw, TextureOptions,
    Transform2D, wgpu,
};
use wasm_bindgen::prelude::*;

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

/// GPU resources retained between browser callbacks. JavaScript owns scheduling.
#[wasm_bindgen]
pub struct WebApp {
    canvas: web_sys::HtmlCanvasElement,
    graphics: GraphicsContext,
    target: RenderTarget<'static>,
    painter: Painter,
    fonts: TextSystem,
    text: TextBuffer,
    prepared_text: Option<PreparedText>,
    text_scale: f32,
    path: PreparedPath,
    image: TextureBinding,
}

#[wasm_bindgen]
impl WebApp {
    /// Adapter and device requests yield to the browser; never block_on on wasm.
    pub async fn create(canvas: web_sys::HtmlCanvasElement) -> Result<WebApp, JsValue> {
        let (graphics, target) = GraphicsContext::with_surface(
            wgpu::SurfaceTarget::Canvas(canvas.clone()),
            SurfaceOptions::new(canvas.width(), canvas.height()),
        )
        .await
        .map_err(js_error)?;
        let mut painter = Painter::new(&graphics);
        let mut fonts = TextSystem::new();
        fonts
            .load_font(include_bytes!(
                "../../../crates/astrelis/tests/fonts/SourceSans3-Regular.otf"
            ))
            .map_err(js_error)?;
        let mut text = TextBuffer::new();
        text.set_text(
            "Astrelis on the web\nShapes, paths, images and embedded text",
            TextStyle::new()
                .family("Source Sans 3")
                .font_size(22.)
                .line_height(32.),
        )
        .map_err(js_error)?;
        let mut builder = Path::builder();
        builder
            .move_to([0., 70.])
            .cubic_to([30., -20.], [90., 130.], [140., 10.]);
        let path = painter
            .prepare_path(
                &builder.build().map_err(js_error)?,
                PathOptions::new().stroke(astrelis::PathStroke::new(6.)),
            )
            .map_err(js_error)?;
        let texture = graphics
            .create_texture(TextureOptions::new(2, 2))
            .map_err(js_error)?;
        texture
            .write(&[
                255, 100, 40, 255, 40, 180, 255, 255, 40, 180, 255, 255, 255, 100, 40, 255,
            ])
            .map_err(js_error)?;
        let image = painter
            .create_image_binding(texture.view(), TextureBindingOptions::new())
            .map_err(js_error)?;
        painter.prepare_for_target(&target).map_err(js_error)?;
        painter
            .prepare_image(&image, &target.render_format())
            .map_err(js_error)?;
        Ok(Self {
            canvas,
            graphics,
            target,
            painter,
            fonts,
            text,
            prepared_text: None,
            text_scale: 0.,
            path,
            image,
        })
    }

    /// Physical backing dimensions; zero size suspends acquisition without drawing.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), JsValue> {
        self.target.resize(width, height).map_err(js_error)
    }

    /// Switches attachment pipelines as well as target storage.
    pub fn set_msaa(&mut self, enabled: bool) -> Result<(), JsValue> {
        let count = if enabled { 4 } else { 1 };
        self.target.set_sample_count(count).map_err(js_error)?;
        self.painter
            .prepare_for_target(&self.target)
            .map_err(js_error)?;
        self.painter
            .prepare_image(&self.image, &self.target.render_format())
            .map_err(js_error)
    }

    /// Drains asynchronous GPU diagnostics without acquiring or recording a frame.
    pub fn check_errors(&self) -> Result<(), JsValue> {
        if self.graphics.is_device_lost() {
            return Err(js_error("GPU device lost; reload to rebuild GPU resources"));
        }
        if let Some(error) = self.graphics.take_gpu_events().first() {
            return Err(js_error(error));
        }
        Ok(())
    }

    /// Draws in CSS pixels, scaled once for the physical framebuffer.
    /// Returns false when acquisition should be retried on a later animation frame.
    pub fn draw(&mut self, scale: f32, pointer_x: f32, pointer_y: f32) -> Result<bool, JsValue> {
        if !scale.is_finite() || scale <= 0. {
            return Err(js_error("device pixel ratio must be positive and finite"));
        }
        self.check_errors()?;
        if self.text_scale != scale {
            self.prepared_text = None;
            let layout = self.text.layout(&mut self.fonts).map_err(js_error)?;
            self.prepared_text = Some(
                self.painter
                    .prepare_text(&layout, TextRasterOptions::new().raster_scale(scale))
                    .map_err(js_error)?,
            );
            self.text_scale = scale;
        }
        let mut frame = match self.target.begin_frame() {
            Ok(frame) => frame,
            Err(FrameError::Retry | FrameError::Suspended) => return Ok(false),
            Err(FrameError::SurfaceLost) => {
                let [width, height] = self.target.size();
                let samples = self.target.sample_count();
                self.target = self
                    .graphics
                    .create_surface(
                        wgpu::SurfaceTarget::Canvas(self.canvas.clone()),
                        SurfaceOptions::new(width, height).sample_count(samples),
                    )
                    .map_err(js_error)?;
                self.painter
                    .prepare_for_target(&self.target)
                    .map_err(js_error)?;
                self.painter
                    .prepare_image(&self.image, &self.target.render_format())
                    .map_err(js_error)?;
                return Ok(false);
            }
            Err(error) => return Err(js_error(error)),
        };
        {
            let mut pass = frame
                .render_pass()
                .clear_color(wgpu::Color {
                    r: 0.015,
                    g: 0.025,
                    b: 0.045,
                    a: 1.,
                })
                .begin()
                .map_err(js_error)?;
            let mut paint = self.painter.begin(&mut pass).map_err(js_error)?;
            let mut paint = paint
                .transformed(Transform2D::scale(scale, scale))
                .map_err(js_error)?;
            paint
                .fill_rounded_rect(Rect::new(20., 20., 440., 300.), 20., [0.04, 0.07, 0.12, 1.])
                .map_err(js_error)?;
            if let Some(text) = &self.prepared_text {
                paint
                    .draw_text(text, TextDraw::new([40., 40.]).color([0.9, 0.95, 1., 1.]))
                    .map_err(js_error)?;
            }
            paint
                .fill_rounded_rect(Rect::new(40., 140., 100., 80.), 14., [0.05, 0.7, 0.45, 1.])
                .map_err(js_error)?;
            paint
                .draw_image(
                    &self.image,
                    TextureDraw::new(Rect::new(160., 140., 80., 80.)),
                )
                .map_err(js_error)?;
            {
                let mut local = paint
                    .transformed(Transform2D::translation(270., 140.))
                    .map_err(js_error)?;
                local
                    .draw_path(&self.path, PathDraw::new([1., 0.5, 0.15, 1.]))
                    .map_err(js_error)?;
            }
            paint
                .draw_line(LineDraw::new([40., 250.], [420., 250.], [0.2, 0.55, 1., 1.]).width(3.))
                .map_err(js_error)?;
            paint
                .fill_ellipse(
                    Rect::new(pointer_x - 10., pointer_y - 10., 20., 20.),
                    [1., 0.3, 0.5, 0.9],
                )
                .map_err(js_error)?;
        }
        frame.finish().map_err(js_error)?;
        Ok(true)
    }
}
