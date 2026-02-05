//! Example demonstrating the Image widget with a procedurally generated texture.
//!
//! This example creates a checkerboard pattern in memory and displays it
//! using the Image widget in the UI system.

use astrelis_core::logging;
use astrelis_render::{GraphicsContext, RenderWindow, RenderWindowBuilder, wgpu};
use astrelis_ui::{Color, FlexDirection, ImageFit, ImageTexture, ImageUV, UiSystem};
use astrelis_winit::{
    WindowId,
    app::run_app,
    window::{Window, WindowDescriptor, WinitPhysicalSize},
};
use std::collections::HashMap;
use std::sync::Arc;

struct App {
    context: Arc<GraphicsContext>,
    windows: HashMap<WindowId, RenderWindow>,
    ui: UiSystem,
    texture: ImageTexture,
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let graphics_ctx =
            GraphicsContext::new_owned_sync().expect("Failed to create graphics context");
        let mut windows = HashMap::new();

        let scale = Window::platform_dpi() as f32;
        let window = ctx
            .create_window(WindowDescriptor {
                title: "Astrelis UI - Image Widget Example".to_string(),
                size: Some(WinitPhysicalSize::new(900.0 * scale, 700.0 * scale)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let renderable_window = RenderWindowBuilder::new()
            .color_format(wgpu::TextureFormat::Bgra8UnormSrgb)
            .with_depth_default()
            .build(window, graphics_ctx.clone())
            .expect("Failed to create render window");

        let window_id = renderable_window.id();
        windows.insert(window_id, renderable_window);

        // Create a procedural checkerboard texture
        let texture = create_checkerboard_texture(&graphics_ctx, 256, 256, 32);

        // Get the first window for from_window
        let first_window = windows.get(&window_id).expect("Window must exist");
        let mut ui = UiSystem::from_window(graphics_ctx.clone(), first_window);

        // Build the UI with multiple image widgets showing different features
        build_image_demo(&mut ui, texture.clone());

        Box::new(App {
            context: graphics_ctx,
            windows,
            ui,
            texture,
        })
    });
}

/// Create a procedural checkerboard texture.
fn create_checkerboard_texture(
    context: &GraphicsContext,
    width: u32,
    height: u32,
    cell_size: u32,
) -> ImageTexture {
    // Generate checkerboard pattern
    let mut pixels = vec![0u8; (width * height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let cell_x = x / cell_size;
            let cell_y = y / cell_size;
            let is_white = (cell_x + cell_y) % 2 == 0;

            let idx = ((y * width + x) * 4) as usize;
            if is_white {
                pixels[idx] = 240; // R
                pixels[idx + 1] = 240; // G
                pixels[idx + 2] = 240; // B
                pixels[idx + 3] = 255; // A
            } else {
                pixels[idx] = 60; // R
                pixels[idx + 1] = 60; // G
                pixels[idx + 2] = 80; // B
                pixels[idx + 3] = 255; // A
            }
        }
    }

    // Create WGPU texture
    let texture = context.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("Checkerboard Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    // Upload pixel data
    context.queue().write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    // Create texture view
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    Arc::new(view)
}

fn build_image_demo(ui: &mut UiSystem, texture: ImageTexture) {
    let theme = ui.theme().clone();
    let colors = &theme.colors;

    ui.build(|root| {
        root.container()
            .width(900.0)
            .height(700.0)
            .padding(20.0)
            .background_color(colors.background)
            .flex_direction(FlexDirection::Column)
            .gap(20.0)
            .child(|ui| {
                ui.text("Image Widget Examples")
                    .size(28.0)
                    .color(colors.text_primary)
                    .build()
            })
            .child(|ui| {
                // Row of images with different fit modes
                ui.row()
                    .gap(20.0)
                    .child(|ui| {
                        ui.container()
                            .width(200.0)
                            .height(200.0)
                            .background_color(colors.surface)
                            .border_radius(8.0)
                            .flex_direction(FlexDirection::Column)
                            .padding(10.0)
                            .gap(5.0)
                            .child(|ui| {
                                ui.text("ImageFit::Fill")
                                    .size(12.0)
                                    .color(colors.text_secondary)
                                    .build()
                            })
                            .child(|ui| {
                                ui.image(texture.clone())
                                    .fit(ImageFit::Fill)
                                    .width(180.0)
                                    .height(150.0)
                                    .build()
                            })
                            .build()
                    })
                    .child(|ui| {
                        ui.container()
                            .width(200.0)
                            .height(200.0)
                            .background_color(colors.surface)
                            .border_radius(8.0)
                            .flex_direction(FlexDirection::Column)
                            .padding(10.0)
                            .gap(5.0)
                            .child(|ui| {
                                ui.text("ImageFit::Contain")
                                    .size(12.0)
                                    .color(colors.text_secondary)
                                    .build()
                            })
                            .child(|ui| {
                                ui.image(texture.clone())
                                    .fit(ImageFit::Contain)
                                    .width(180.0)
                                    .height(150.0)
                                    .build()
                            })
                            .build()
                    })
                    .child(|ui| {
                        ui.container()
                            .width(200.0)
                            .height(200.0)
                            .background_color(colors.surface)
                            .border_radius(8.0)
                            .flex_direction(FlexDirection::Column)
                            .padding(10.0)
                            .gap(5.0)
                            .child(|ui| {
                                ui.text("With Red Tint")
                                    .size(12.0)
                                    .color(colors.text_secondary)
                                    .build()
                            })
                            .child(|ui| {
                                // KEEP: Image tint color is content-specific
                                ui.image(texture.clone())
                                    .tint(Color::from_rgb_u8(255, 128, 128))
                                    .width(180.0)
                                    .height(150.0)
                                    .build()
                            })
                            .build()
                    })
                    .child(|ui| {
                        ui.container()
                            .width(200.0)
                            .height(200.0)
                            .background_color(colors.surface)
                            .border_radius(8.0)
                            .flex_direction(FlexDirection::Column)
                            .padding(10.0)
                            .gap(5.0)
                            .child(|ui| {
                                ui.text("With Green Tint")
                                    .size(12.0)
                                    .color(colors.text_secondary)
                                    .build()
                            })
                            .child(|ui| {
                                // KEEP: Image tint color is content-specific
                                ui.image(texture.clone())
                                    .tint(Color::from_rgb_u8(128, 255, 128))
                                    .width(180.0)
                                    .height(150.0)
                                    .build()
                            })
                            .build()
                    })
                    .build()
            })
            .child(|ui| {
                // Row showing UV/sprite sheet usage
                ui.row()
                    .gap(20.0)
                    .child(|ui| {
                        ui.container()
                            .width(200.0)
                            .height(200.0)
                            .background_color(colors.surface)
                            .border_radius(8.0)
                            .flex_direction(FlexDirection::Column)
                            .padding(10.0)
                            .gap(5.0)
                            .child(|ui| {
                                ui.text("UV: Top-Left Quarter")
                                    .size(12.0)
                                    .color(colors.text_secondary)
                                    .build()
                            })
                            .child(|ui| {
                                ui.image(texture.clone())
                                    .uv(ImageUV::new(0.0, 0.0, 0.5, 0.5))
                                    .width(180.0)
                                    .height(150.0)
                                    .build()
                            })
                            .build()
                    })
                    .child(|ui| {
                        ui.container()
                            .width(200.0)
                            .height(200.0)
                            .background_color(colors.surface)
                            .border_radius(8.0)
                            .flex_direction(FlexDirection::Column)
                            .padding(10.0)
                            .gap(5.0)
                            .child(|ui| {
                                ui.text("UV: Bottom-Right")
                                    .size(12.0)
                                    .color(colors.text_secondary)
                                    .build()
                            })
                            .child(|ui| {
                                ui.image(texture.clone())
                                    .uv(ImageUV::new(0.5, 0.5, 1.0, 1.0))
                                    .width(180.0)
                                    .height(150.0)
                                    .build()
                            })
                            .build()
                    })
                    .child(|ui| {
                        ui.container()
                            .width(200.0)
                            .height(200.0)
                            .background_color(colors.surface)
                            .border_radius(8.0)
                            .flex_direction(FlexDirection::Column)
                            .padding(10.0)
                            .gap(5.0)
                            .child(|ui| {
                                ui.text("Rounded Corners")
                                    .size(12.0)
                                    .color(colors.text_secondary)
                                    .build()
                            })
                            .child(|ui| {
                                ui.image(texture.clone())
                                    .border_radius(20.0)
                                    .width(180.0)
                                    .height(150.0)
                                    .build()
                            })
                            .build()
                    })
                    .child(|ui| {
                        ui.container()
                            .width(200.0)
                            .height(200.0)
                            .background_color(colors.surface)
                            .border_radius(8.0)
                            .flex_direction(FlexDirection::Column)
                            .padding(10.0)
                            .gap(5.0)
                            .child(|ui| {
                                ui.text("Rounded + Tint")
                                    .size(12.0)
                                    .color(colors.text_secondary)
                                    .build()
                            })
                            .child(|ui| {
                                // KEEP: Image tint color is content-specific
                                ui.image(texture.clone())
                                    .border_radius(30.0)
                                    .tint(Color::from_rgb_u8(128, 200, 255))
                                    .width(180.0)
                                    .height(150.0)
                                    .build()
                            })
                            .build()
                    })
                    .build()
            })
            .child(|ui| {
                ui.text("The Image widget supports textures, UV coordinates (for sprite sheets),")
                    .size(14.0)
                    .color(colors.text_secondary)
                    .build()
            })
            .child(|ui| {
                ui.text("tint colors (multiplied with texture), and rounded corners via SDF.")
                    .size(14.0)
                    .color(colors.text_secondary)
                    .build()
            })
            .build();
    });
}

impl astrelis_winit::app::App for App {
    fn update(
        &mut self,
        _ctx: &mut astrelis_winit::app::AppCtx,
        _time: &astrelis_winit::FrameTime,
    ) {
        // No updates needed for this static demo
    }

    fn render(
        &mut self,
        _ctx: &mut astrelis_winit::app::AppCtx,
        window_id: WindowId,
        events: &mut astrelis_winit::event::EventBatch,
    ) {
        let Some(window) = self.windows.get_mut(&window_id) else {
            return;
        };

        // Handle resize
        events.dispatch(|event| {
            if let astrelis_winit::event::Event::WindowResized(size) = event {
                window.resized(*size);
                self.ui.set_viewport(window.viewport());
                astrelis_winit::event::HandleStatus::consumed()
            } else {
                astrelis_winit::event::HandleStatus::ignored()
            }
        });

        let bg = self.ui.theme().colors.background;
        let Some(frame) = window.begin_frame() else {
            return; // Surface not available
        };

        {
            let mut pass = frame
                .render_pass()
                .clear_color(bg)
                .with_window_depth()
                .clear_depth(0.0)
                .label("UI")
                .build();

            self.ui.render(pass.wgpu_pass());
        }
        // Frame auto-submits on drop
    }
}
