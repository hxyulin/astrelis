use crate::framebuffer::tests::pixels;
use crate::*;
fn target(g: &GraphicsContext, count: u32, depth: bool) -> Framebuffer {
    let mut o = FramebufferOptions::new(64, 64).sample_count(count).usage(
        wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
    );
    if depth {
        o = o.depth_stencil(wgpu::TextureFormat::Depth24PlusStencil8);
    }
    g.create_framebuffer(o).unwrap()
}
fn pixel(p: &[u8], x: usize, y: usize) -> [u8; 4] {
    p[(y * 64 + x) * 4..(y * 64 + x + 1) * 4]
        .try_into()
        .unwrap()
}
#[test]
fn attachment_effects_follow_submission_order_and_load_dependencies_are_rechecked() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let scope = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
        let mut shared = target(&g, 1, true);
        let mut other = target(&g, 1, false);
        {
            let mut f = shared.begin_frame().unwrap();
            drop(f.render_pass().begin().unwrap());
            f.finish().unwrap();
        }
        let mut a = other.begin_frame().unwrap();
        drop(a.render_to(&mut shared).begin().unwrap());
        {
            let mut b = shared.begin_frame().unwrap();
            drop(
                b.render_pass()
                    .depth_ops(Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Discard,
                    }))
                    .begin()
                    .unwrap(),
            );
            b.finish().unwrap();
        }
        a.finish().unwrap();
        {
            let mut f = shared.begin_frame().unwrap();
            drop(f.render_pass().load_all().begin().unwrap());
            f.finish().unwrap();
        }
        // A load recorded before another recording discards storage must not submit afterward.
        let mut a = other.begin_frame().unwrap();
        drop(a.render_to(&mut shared).load_all().begin().unwrap());
        {
            let mut b = shared.begin_frame().unwrap();
            drop(
                b.render_pass()
                    .depth_ops(Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Discard,
                    }))
                    .begin()
                    .unwrap(),
            );
            b.finish().unwrap();
        }
        assert!(matches!(a.finish(), Err(Error::UninitializedDepth)));
        // Dropping a recording never commits its discard.
        {
            let mut f = shared.begin_frame().unwrap();
            drop(f.render_pass().begin().unwrap());
            f.finish().unwrap();
        }
        {
            let mut f = shared.begin_frame().unwrap();
            drop(
                f.render_pass()
                    .depth_ops(Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Discard,
                    }))
                    .begin()
                    .unwrap(),
            );
        }
        {
            let mut f = shared.begin_frame().unwrap();
            drop(f.render_pass().load_all().begin().unwrap());
            f.finish().unwrap();
        }
        assert!(scope.pop().await.is_none());
    });
}
#[test]
fn msaa_can_resolve_only_last_pass_and_discard_samples_independently() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let scope = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
        let mut t = target(&g, 4, true);
        let bytes = pixels(&g, &mut t, |f| {
            drop(
                f.render_pass()
                    .clear_color(wgpu::Color::RED)
                    .resolve(false)
                    .begin()
                    .unwrap(),
            );
            let p = f
                .render_pass()
                .load_all()
                .color_ops(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Discard,
                })
                .without_depth_stencil()
                .begin()
                .unwrap();
            assert_eq!(p.depth_stencil_format(), None);
            drop(p);
        });
        assert_eq!(pixel(&bytes, 32, 32), [255, 0, 0, 255]);
        {
            let mut f = t.begin_frame().unwrap();
            assert!(matches!(
                f.render_pass().load_color().begin(),
                Err(Error::UninitializedFramebuffer)
            ));
        }
        // Resolve output is initialized independently of discarded sample storage.
        let colors = [Some(
            RenderColorAttachment::new(t.color_view().unwrap(), t.format(), t.size()).load(),
        )];
        let mut f = t.begin_frame().unwrap();
        drop(
            f.begin_render_pass(&RenderPassDescriptor {
                colors: &colors,
                ..Default::default()
            })
            .unwrap(),
        );
        f.finish().unwrap();
        assert!(scope.pop().await.is_none());
    });
}
#[test]
fn managed_custom_passes_support_depth_only_mrt_and_imported_mips() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let scope = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
        let mut a = target(&g, 1, true);
        let mut b = target(&g, 1, false);
        let mut renderer = MeshRenderer::new(&g);
        let mesh = g
            .create_mesh(
                &[
                    Vertex::new([-1., -1., 0.], [1.; 4]),
                    Vertex::new([1., -1., 0.], [1.; 4]),
                    Vertex::new([0., 1., 0.], [1.; 4]),
                ],
                &[0, 1, 2],
            )
            .unwrap();
        let bytes = pixels(&g, &mut a, |f| {
            let depth = f.depth_stencil_attachment().unwrap().unwrap();
            {
                let mut p = f
                    .begin_render_pass(&RenderPassDescriptor {
                        depth_stencil: Some(&depth),
                        ..Default::default()
                    })
                    .unwrap();
                assert_eq!(p.format(), None);
                assert!(matches!(
                    renderer.draw(&mut p, &mesh),
                    Err(Error::ExpectedSingleColor)
                ));
            }
            let mut ca = f.color_attachment().unwrap();
            ca.ops.load = wgpu::LoadOp::Clear(wgpu::Color::RED);
            let mut cb = b.color_attachment().unwrap();
            cb.ops.load = wgpu::LoadOp::Clear(wgpu::Color::GREEN);
            let colors = [Some(ca), Some(cb)];
            let depth = depth.load();
            let mut p = f
                .begin_render_pass(&RenderPassDescriptor {
                    colors: &colors,
                    depth_stencil: Some(&depth),
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(p.render_format().colors.len(), 2);
            assert!(matches!(
                renderer.draw(&mut p, &mesh),
                Err(Error::ExpectedSingleColor)
            ));
        });
        assert_eq!(pixel(&bytes, 32, 32), [255, 0, 0, 255]);
        let green = pixels(&g, &mut b, |f| {
            drop(f.render_pass().load_all().begin().unwrap())
        });
        assert_eq!(pixel(&green, 32, 32), [0, 255, 0, 255]);
        let raw = g.device().create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 128,
                height: 128,
                depth_or_array_layers: 2,
            },
            mip_level_count: 2,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = raw.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_mip_level: 1,
            mip_level_count: Some(1),
            base_array_layer: 1,
            array_layer_count: Some(1),
            ..Default::default()
        });
        let colors = [Some(RenderColorAttachment::new(
            &view,
            raw.format(),
            [64, 64],
        ))];
        let mut f = a.begin_frame().unwrap();
        let mut p = f
            .begin_render_pass(&RenderPassDescriptor {
                colors: &colors,
                ..Default::default()
            })
            .unwrap();
        renderer.draw(&mut p, &mesh).unwrap();
        drop(p);
        f.finish().unwrap();
        assert!(scope.pop().await.is_none());
    });
}
#[test]
fn custom_copy_initializes_managed_color_for_later_loads() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let scope = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
        let mut source = target(&g, 1, false);
        let mut dest = g
            .create_framebuffer(FramebufferOptions::new(64, 64).usage(
                wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC,
            ))
            .unwrap();
        {
            let mut f = source.begin_frame().unwrap();
            drop(
                f.render_pass()
                    .clear_color(wgpu::Color::GREEN)
                    .begin()
                    .unwrap(),
            );
            f.finish().unwrap();
        }
        let texture = source.color_texture().unwrap().clone();
        let mut f = source.begin_frame().unwrap();
        f.write_framebuffer_color(&mut dest, |encoder, view| {
            encoder.copy_texture_to_texture(
                texture.as_image_copy(),
                view.texture().as_image_copy(),
                texture.size(),
            )
        })
        .unwrap();
        drop(f.render_to(&mut dest).load_all().begin().unwrap());
        f.finish().unwrap();
        let p = pixels(&g, &mut dest, |f| {
            drop(f.render_pass().load_all().begin().unwrap())
        });
        assert_eq!(pixel(&p, 32, 32), [0, 255, 0, 255]);
        assert!(scope.pop().await.is_none());
    });
}
#[test]
fn custom_streams_uint16_instances_ranges_updates_and_imports_render() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let scope = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
        let mut r = MeshRenderer::new(&g);
        let mut t = target(&g, 1, false);
        let shader=g.device().create_shader_module(wgpu::ShaderModuleDescriptor{label:None,source:wgpu::ShaderSource::Wgsl(r#"
    struct Out {@builtin(position) position:vec4<f32>, @location(0) color:vec4<f32>};
    @vertex fn vertex_main(@location(0) p:vec2<f32>,@location(1) offset:vec2<f32>,@location(2) color:vec4<f32>)->Out {var o:Out;o.position=vec4(p+offset,0.,1.);o.color=color;return o;}
    @fragment fn fragment_main(i:Out)->@location(0) vec4<f32>{return i.color;}
    "#.into())});
        let layouts = [
            VertexLayout {
                stride: 8,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: wgpu::vertex_attr_array![0=>Float32x2].to_vec(),
            },
            VertexLayout {
                stride: 24,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: wgpu::vertex_attr_array![1=>Float32x2,2=>Float32x4].to_vec(),
            },
        ];
        let vertices = [[-0.45f32, 0.8], [-0.45, -0.8], [0.45, 0.8], [0.45, -0.8]];
        let instances = [[-0.5f32, 0., 1., 0., 0., 1.], [0.5, 0., 0., 1., 0., 1.]];
        let streams = [
            VertexStream {
                bytes: bytemuck::cast_slice(&vertices),
                layout: layouts[0].clone(),
            },
            VertexStream {
                bytes: bytemuck::cast_slice(&instances),
                layout: layouts[1].clone(),
            },
        ];
        let mut o = MeshOptions::new(&streams);
        o.indices = Some(MeshIndices::U16(&[0, 1, 2, 2, 1, 3]));
        o.dynamic = true;
        let mesh = g.create_mesh_with_options(o).unwrap();
        let mut options = MaterialOptions::new(&shader).blend(None);
        options.vertex_layouts = &layouts;
        let m = g.create_material(options);
        r.try_prepare_material(&m, &t.render_format())
            .await
            .unwrap();
        let draw = mesh.full_draw().instances(0..2);
        let p = pixels(&g, &mut t, |f| {
            let mut pass = f.render_pass().begin().unwrap();
            r.draw_range_with_material(&mut pass, &mesh, &m, &draw)
                .unwrap();
            assert!(matches!(
                r.draw_range_with_material(&mut pass, &mesh, &m, &MeshDraw::new(0..7)),
                Err(Error::InvalidGeometry)
            ));
        });
        assert_eq!(pixel(&p, 8, 32), [255, 0, 0, 255]);
        assert_eq!(pixel(&p, 56, 32), [0, 255, 0, 255]);
        let changed = [[-0.5f32, 0., 1., 1., 0., 1.], [0.5, 0., 0., 1., 1., 1.]];
        mesh.write_vertices(&g, 1, 0, bytemuck::cast_slice(&changed))
            .unwrap();
        assert!(matches!(
            mesh.write_vertices(&g, 1, 1, &[0; 4]),
            Err(Error::InvalidGeometry)
        ));
        let buffers = [
            MeshVertexBuffer {
                buffer: mesh.vertex_buffer_at(0).unwrap(),
                range: 0..32,
                layout: layouts[0].clone(),
            },
            MeshVertexBuffer {
                buffer: mesh.vertex_buffer_at(1).unwrap(),
                range: 0..48,
                layout: layouts[1].clone(),
            },
        ];
        let imported = g
            .create_mesh_from_buffers(
                &buffers,
                Some(MeshIndexBuffer {
                    buffer: mesh.index_buffer().unwrap(),
                    range: 0..12,
                    format: wgpu::IndexFormat::Uint16,
                }),
                wgpu::PrimitiveTopology::TriangleList,
            )
            .unwrap();
        let p = pixels(&g, &mut t, |f| {
            r.draw_range_with_material(&mut f.render_pass().begin().unwrap(), &imported, &m, &draw)
                .unwrap()
        });
        assert_eq!(pixel(&p, 8, 32), [255, 255, 0, 255]);
        assert_eq!(pixel(&p, 56, 32), [0, 255, 255, 255]);
        mesh.write_indices(&g, 0, bytemuck::cast_slice(&[0u16, 1, 2, 0, 1, 2]))
            .unwrap();
        let p = pixels(&g, &mut t, |f| {
            r.draw_range_with_material(&mut f.render_pass().begin().unwrap(), &imported, &m, &draw)
                .unwrap()
        });
        assert_eq!(pixel(&p, 8, 32), [255, 255, 0, 255]);
        assert_eq!(pixel(&p, 56, 32), [0; 4]);

        assert!(scope.pop().await.is_none());
    });
}
#[test]
fn checked_preparation_returns_diagnostics_without_caching_invalid_pipelines() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut r = MeshRenderer::new(&g);
        let mut o = MaterialOptions::new(r.default_material().shader());
        o.vertex_entry = "missing";
        let m = g.create_material(o);
        let format = RenderFormat::color(wgpu::TextureFormat::Rgba8Unorm, 1);
        assert!(matches!(
            r.try_prepare_material(&m, &format).await,
            Err(Error::Validation(_))
        ));
        assert!(matches!(
            r.try_prepare_material(&m, &format).await,
            Err(Error::Validation(_))
        ));
        let t = g.create_texture(TextureOptions::new(1, 1)).unwrap();
        let mut textures = TextureRenderer::new(&g);
        let b = textures
            .create_binding(t.view(), TextureBindingOptions::new())
            .unwrap();
        let shader = g
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(include_str!("texture.wgsl").into()),
            });
        let mut o = TextureMaterialOptions::new();
        o.shader = Some(&shader);
        o.vertex_entry = "missing";
        let m = g.create_texture_material(o);
        assert!(matches!(
            textures.try_prepare_material(&b, &m, &format).await,
            Err(Error::Validation(_))
        ));
        assert!(matches!(
            textures.try_prepare_material(&b, &m, &format).await,
            Err(Error::Validation(_))
        ));
    });
}

#[test]
fn line_strip_restart_supports_uint16_indices() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let scope = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
        let mut renderer = MeshRenderer::new(&g);
        let mut target = target(&g, 1, false);
        let vertices = [
            Vertex::new([-0.8, 0.5, 0.], [1.; 4]),
            Vertex::new([0.8, 0.5, 0.], [1.; 4]),
            Vertex::new([-0.8, -0.5, 0.], [1.; 4]),
            Vertex::new([0.8, -0.5, 0.], [1.; 4]),
        ];
        let streams = [VertexStream {
            bytes: bytemuck::cast_slice(&vertices),
            layout: VertexLayout::new(&Vertex::layout()),
        }];
        let mut options = MeshOptions::new(&streams);
        options.topology = wgpu::PrimitiveTopology::LineStrip;
        options.indices = Some(MeshIndices::U16(&[0, 1, u16::MAX, 2, 3]));
        let mesh = g.create_mesh_with_options(options).unwrap();
        let mut options = MaterialOptions::new(renderer.default_material().shader()).blend(None);
        options.topology = wgpu::PrimitiveTopology::LineStrip;
        options.strip_index_format = Some(wgpu::IndexFormat::Uint16);
        let material = g.create_material(options);
        let bytes = pixels(&g, &mut target, |f| {
            renderer
                .draw_with_material(&mut f.render_pass().begin().unwrap(), &mesh, &material)
                .unwrap()
        });
        assert!(bytes.as_chunks::<4>().0.iter().any(|p| p == &[255; 4]));
        assert_eq!(pixel(&bytes, 32, 32), [0; 4]);
        assert!(scope.pop().await.is_none());
    });
}
