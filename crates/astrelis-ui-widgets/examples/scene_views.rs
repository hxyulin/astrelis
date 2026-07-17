//! Two independently animated scene renderers composited through retained render views.

#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use std::{sync::Arc, time::Instant};

use astrelis_core::{
    color::Color,
    geometry::Size,
    math::{Affine2, Mat4, Quat, Vec2, Vec3},
};
use astrelis_gpu::{
    CompositeAlphaMode, DeviceDescriptor, PresentMode, RequestAdapterOptions, SurfaceConfiguration,
    SurfaceFrameStatus, SurfaceTarget, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages,
};
use astrelis_paint::ExternalImage;
use astrelis_paint_gpu::{
    RenderTarget as PaintTarget, Renderer as PaintRenderer, RendererOptions as PaintOptions,
};
use astrelis_platform::{
    Application, PlatformContext, Window, WindowAttributes, WindowEvent, WindowId,
};
use astrelis_render::RenderTarget;
use astrelis_render_2d::{
    Camera2D, DrawList2D, Renderer2D, SpriteDraw, TextureOptions as TextureOptions2D,
};
use astrelis_render_3d::{
    Camera3D, DrawList3D, Lighting, MaterialDescriptor, MeshDraw, Renderer3D, cube,
};
use astrelis_text::{FontDatabase, FontFamily};
use astrelis_ui_core::{ElementHandle, LayoutStyle, Length, Theme, Ui};
use astrelis_ui_widgets::{
    RenderView, RenderViewContent, RenderViewResizePolicy, render_view_snapshot,
};

const NOTO_SANS: &[u8] = include_bytes!("../../astrelis-ui-core/assets/NotoSans.ttf");

struct SceneTexture {
    _texture: astrelis_gpu::Texture,
    view: astrelis_gpu::TextureView,
    image: ExternalImage,
    allocation: Size<astrelis_core::geometry::Physical, u32>,
}

struct GpuState {
    surface: astrelis_gpu::Surface,
    device: astrelis_gpu::Device,
    queue: astrelis_gpu::Queue,
    configuration: SurfaceConfiguration,
    paint: PaintRenderer,
    renderer_2d: Renderer2D,
    renderer_3d: Renderer3D,
    scene_2d: Option<SceneTexture>,
    scene_3d: Option<SceneTexture>,
    sprite: astrelis_render_2d::TextureHandle,
    cube: astrelis_render_3d::MeshHandle,
    material: astrelis_render_3d::MaterialHandle,
}

struct Demo {
    instance: astrelis_gpu::Instance,
    window: Option<Window>,
    gpu: Option<GpuState>,
    ui: Ui<()>,
    view_2d: ElementHandle<RenderView<()>>,
    view_3d: ElementHandle<RenderView<()>>,
    started: Instant,
}

impl Demo {
    fn new() -> Self {
        let mut fonts = FontDatabase::empty();
        fonts
            .register_font(Arc::<[u8]>::from(NOTO_SANS))
            .expect("font");
        let mut ui = Ui::new(
            fonts,
            Theme {
                font_families: vec![FontFamily::Named("Noto Sans".into())],
                ..Default::default()
            },
        );
        let root = ui.root();
        let column = ui.add_column(root).expect("column");
        ui.add_label(
            column,
            "Milestone 14 — 2D and 3D renderers sharing one device",
        )
        .expect("label");
        let row = ui.add_row(column).expect("row");
        ui.set_layout(
            row,
            LayoutStyle {
                grow: 1.0,
                ..Default::default()
            },
        )
        .expect("row layout");
        let view_2d = ui
            .add_widget(row, RenderView::new("Animated 2D scene", |_| ()))
            .expect("2D view");
        let view_3d = ui
            .add_widget(row, RenderView::new("Animated 3D scene", |_| ()))
            .expect("3D view");
        for view in [view_2d, view_3d] {
            ui.set_layout(
                view,
                LayoutStyle {
                    width: Length::Percent(0.5),
                    height: Length::Px(430.0),
                    grow: 1.0,
                    ..Default::default()
                },
            )
            .expect("view layout");
        }
        Self {
            instance: astrelis_gpu_wgpu::create_instance(Default::default()),
            window: None,
            gpu: None,
            ui,
            view_2d,
            view_3d,
            started: Instant::now(),
        }
    }

    fn configure(&mut self, width: u32, height: u32) {
        let Some(gpu) = &mut self.gpu else { return };
        if width == 0 || height == 0 {
            return;
        }
        gpu.configuration.width = width;
        gpu.configuration.height = height;
        gpu.surface
            .configure(&gpu.device, gpu.configuration.clone())
            .expect("configure");
        let scale = self
            .window
            .as_ref()
            .map_or(1.0, |window| window.scale_factor() as f32);
        self.ui.set_viewport(
            Size::new(width as f32 / scale, height as f32 / scale),
            scale,
        );
    }

    fn ensure_scene(
        &mut self,
        which_2d: bool,
    ) -> Option<Size<astrelis_core::geometry::Physical, u32>> {
        let handle = if which_2d { self.view_2d } else { self.view_3d };
        let snapshot = render_view_snapshot(&mut self.ui, handle).ok()?;
        if !snapshot.should_render {
            return None;
        }
        let gpu = self.gpu.as_mut()?;
        let slot = if which_2d {
            &mut gpu.scene_2d
        } else {
            &mut gpu.scene_3d
        };
        let allocation = RenderViewResizePolicy::default().allocation(
            slot.as_ref().map(|scene| scene.allocation),
            snapshot.desired_physical_size,
            true,
        )?;
        if slot.as_ref().map(|scene| scene.allocation) != Some(allocation) {
            if let Some(old) = slot.take() {
                gpu.paint.unregister_external_image(&old.image);
            }
            let texture = gpu.device.create_texture(TextureDescriptor {
                label: Some("scene view allocation".into()),
                size: astrelis_gpu::Extent3d::d2(allocation.width, allocation.height),
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            });
            let view = texture.create_view(Default::default());
            let image = ExternalImage::new(allocation).expect("image token");
            gpu.paint
                .register_external_image(&image, view.clone())
                .expect("register scene image");
            *slot = Some(SceneTexture {
                _texture: texture,
                view,
                image,
                allocation,
            });
        }
        let image = slot.as_ref().unwrap().image.clone();
        self.ui
            .update_widget(handle, |view| {
                view.set_content(RenderViewContent::Ready {
                    image,
                    source_extent: snapshot.desired_physical_size,
                })
            })
            .expect("update view");
        Some(snapshot.desired_physical_size)
    }

    fn redraw(&mut self) {
        let size_2d = self.ensure_scene(true);
        let size_3d = self.ensure_scene(false);
        let list = self.ui.display_list().expect("display list");
        let Some(gpu) = &mut self.gpu else { return };
        let frame = match gpu.surface.acquire().expect("acquire") {
            SurfaceFrameStatus::Ready(frame) | SurfaceFrameStatus::Suboptimal(frame) => frame,
            SurfaceFrameStatus::Outdated | SurfaceFrameStatus::Lost => {
                gpu.surface
                    .configure(&gpu.device, gpu.configuration.clone())
                    .expect("recover");
                return;
            }
            SurfaceFrameStatus::Timeout | SurfaceFrameStatus::Occluded => return,
            _ => return,
        };
        let time = self.started.elapsed().as_secs_f32();
        let mut encoder = gpu.device.create_command_encoder(Default::default());
        if let (Some(render_size), Some(scene)) = (size_2d, &gpu.scene_2d) {
            let camera = Camera2D::default();
            let mut draws = DrawList2D::new();
            draws.draw_sprite(SpriteDraw {
                texture: gpu.sprite,
                source: None,
                transform: Affine2::from_scale_angle_translation(
                    Vec2::splat(3.0),
                    time,
                    Vec2::ZERO,
                ),
                size: Vec2::splat(32.0),
                pivot: Vec2::splat(0.5),
                tint: Color::WHITE,
                layer: 0,
            });
            gpu.renderer_2d
                .render(
                    &mut encoder,
                    &RenderTarget {
                        view: scene.view.clone(),
                        allocation_size: scene.allocation,
                        render_size,
                        scale_factor: self
                            .window
                            .as_ref()
                            .map_or(1.0, |window| window.scale_factor() as f32),
                        clear_color: Color::rgb(0.03, 0.08, 0.16),
                    },
                    &camera,
                    &draws,
                )
                .expect("2D render");
        }
        if let (Some(render_size), Some(scene)) = (size_3d, &gpu.scene_3d) {
            let mut camera = Camera3D {
                position: Vec3::new(time.sin() * 4.0, 2.5, time.cos() * 4.0),
                ..Default::default()
            };
            camera.look_at(Vec3::ZERO, Vec3::Y);
            let mut draws = DrawList3D::new();
            draws.draw_mesh(MeshDraw {
                mesh: gpu.cube,
                material: gpu.material,
                transform: Mat4::from_quat(Quat::from_rotation_y(time)),
                tint: Color::WHITE,
            });
            draws.draw_grid(5, 0.5, Color::new(0.3, 0.4, 0.6, 0.7));
            draws.draw_axes(Mat4::IDENTITY, 1.4);
            gpu.renderer_3d
                .render(
                    &mut encoder,
                    &RenderTarget {
                        view: scene.view.clone(),
                        allocation_size: scene.allocation,
                        render_size,
                        scale_factor: 1.0,
                        clear_color: Color::rgb(0.025, 0.035, 0.08),
                    },
                    &camera,
                    &Lighting::default(),
                    &draws,
                )
                .expect("3D render");
        }
        gpu.paint
            .render(
                &mut encoder,
                &list,
                PaintTarget {
                    view: frame.texture().create_view(Default::default()),
                    format: gpu.configuration.format,
                    size: Size::new(gpu.configuration.width, gpu.configuration.height),
                    scale_factor: self
                        .window
                        .as_ref()
                        .map_or(1.0, |window| window.scale_factor() as f32),
                    clear_color: Color::BLACK,
                },
            )
            .expect("paint UI");
        gpu.queue
            .submit([encoder.finish().expect("finish")])
            .expect("submit");
        frame.present().expect("present");
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

impl Application for Demo {
    type UserEvent = ();
    fn resumed(&mut self, context: &mut PlatformContext<'_, ()>) {
        if self.window.is_some() {
            return;
        }
        let window = context
            .create_window(WindowAttributes {
                title: "Astrelis scene RenderViews".into(),
                inner_size: Some(Size::new(1100.0, 620.0)),
                ..Default::default()
            })
            .expect("window");
        let surface = self
            .instance
            .create_surface(SurfaceTarget::new(window.clone()))
            .expect("surface");
        let adapter = pollster::block_on(self.instance.request_adapter(RequestAdapterOptions {
            compatible_surface: Some(surface.clone()),
            ..Default::default()
        }))
        .expect("adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(DeviceDescriptor::default()))
                .expect("device");
        let capabilities = surface.capabilities(&adapter).expect("capabilities");
        let size = window.inner_size().expect("size");
        let configuration = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: capabilities.formats[0],
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: PresentMode::Fifo,
            alpha_mode: capabilities
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(CompositeAlphaMode::Opaque),
            desired_maximum_frame_latency: 2,
        };
        surface
            .configure(&device, configuration.clone())
            .expect("configure");
        let paint = PaintRenderer::new(device.clone(), queue.clone(), PaintOptions::default())
            .expect("paint");
        let mut renderer_2d = Renderer2D::new(device.clone(), queue.clone(), Default::default())
            .expect("2D renderer");
        let sprite = renderer_2d
            .create_texture_rgba8(
                Size::new(2, 2),
                &[
                    255, 70, 80, 255, 80, 220, 255, 255, 255, 220, 70, 255, 120, 80, 255, 255,
                ],
                TextureOptions2D::default(),
            )
            .expect("sprite");
        let mut renderer_3d = Renderer3D::new(device.clone(), queue.clone(), Default::default())
            .expect("3D renderer");
        let cube = renderer_3d.create_mesh(&cube(1.7)).expect("cube");
        let material = renderer_3d
            .create_material(MaterialDescriptor {
                base_color: Color::rgb(0.2, 0.7, 1.0),
                ..Default::default()
            })
            .expect("material");
        self.window = Some(window);
        self.gpu = Some(GpuState {
            surface,
            device,
            queue,
            configuration: configuration.clone(),
            paint,
            renderer_2d,
            renderer_3d,
            scene_2d: None,
            scene_3d: None,
            sprite,
            cube,
            material,
        });
        self.configure(configuration.width, configuration.height);
        self.window.as_ref().unwrap().request_redraw();
    }
    fn window_event(
        &mut self,
        context: &mut PlatformContext<'_, ()>,
        id: WindowId,
        event: WindowEvent,
    ) {
        if matches!(event, WindowEvent::CloseRequested) {
            self.gpu = None;
            self.window = None;
            context.exit();
            return;
        }
        match event {
            WindowEvent::Resized(size) => self.configure(size.width, size.height),
            WindowEvent::ScaleFactorChanged { inner_size, .. } => {
                self.configure(inner_size.width, inner_size.height)
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
        if let Some(window) = &self.window {
            let update = self
                .ui
                .handle_window_event(window, &context.clipboard(), &event)
                .expect("UI event");
            if update.redraw {
                window.request_redraw();
            }
        }
        let _ = id;
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), astrelis_platform::PlatformError> {
    astrelis_platform_winit::run(Demo::new())
}

#[cfg(target_arch = "wasm32")]
fn main() {}
