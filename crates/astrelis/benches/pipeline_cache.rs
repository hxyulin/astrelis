//! Material cache maintenance and warm preparation. CSV stdout, metadata stderr.
//! Run `cargo bench -p astrelis --bench pipeline_cache --locked`.
use astrelis::{
    FramebufferOptions, GraphicsContext, MaterialOptions, MeshRenderer, PipelineCacheStats,
    TextureBindingOptions, TextureDraw, TextureMaterialOptions, TextureOptions, TextureRenderer,
    Vertex, wgpu,
};
use std::{hint::black_box, time::Instant};

const SAMPLES: usize = 20;
const HITS: usize = 10_000;
const CHURN: usize = 16;

fn report(renderer: &str, operation: &str, mut samples: Vec<f64>) {
    samples.sort_by(f64::total_cmp);
    println!(
        "{renderer},{operation},{:.2},{:.2}",
        samples[SAMPLES / 2],
        samples[SAMPLES * 95 / 100]
    );
}

fn measure<R>(
    name: &str,
    renderer: &mut R,
    mut hit: impl FnMut(&mut R),
    mut insert: impl FnMut(&mut R),
    mut prune: impl FnMut(&mut R) -> usize,
    stats: impl Fn(&R) -> PipelineCacheStats,
) {
    let mut hits = Vec::new();
    let mut hits_with_stale = Vec::new();
    let mut churn = Vec::new();
    let mut maintenance = Vec::new();
    for _ in 0..SAMPLES {
        let start = Instant::now();
        for _ in 0..HITS {
            hit(black_box(renderer));
        }
        hits.push(start.elapsed().as_nanos() as f64 / HITS as f64);
        let start = Instant::now();
        for _ in 0..CHURN {
            insert(black_box(renderer));
        }
        churn.push(start.elapsed().as_nanos() as f64 / CHURN as f64);
        assert_eq!(
            stats(renderer),
            PipelineCacheStats {
                entries: CHURN + 1,
                live_entries: 1,
                stale_entries: CHURN
            }
        );
        let start = Instant::now();
        for _ in 0..HITS {
            hit(black_box(renderer));
        }
        hits_with_stale.push(start.elapsed().as_nanos() as f64 / HITS as f64);
        let start = Instant::now();
        assert_eq!(prune(renderer), CHURN);
        maintenance.push(start.elapsed().as_nanos() as f64 / CHURN as f64);
        assert_eq!(
            stats(renderer),
            PipelineCacheStats {
                entries: 1,
                live_entries: 1,
                stale_entries: 0
            }
        );
    }
    report(name, "warm_prepare", hits);
    report(name, "warm_prepare_with_16_stale", hits_with_stale);
    report(name, "material_create_prepare_drop", churn);
    report(name, "prune_per_stale_variant", maintenance);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await?;
        eprintln!(
            "adapter={:?}; samples={SAMPLES}; warm_lookups={HITS}; churn_batch={CHURN}; debug_assertions={}",
            g.adapter().get_info(),
            cfg!(debug_assertions)
        );
        let validation = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
        let mut target = g.create_framebuffer(
            FramebufferOptions::new(16, 16)
                .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC),
        )?;
        let format = target.render_format();
        let mut meshes = MeshRenderer::new(&g);
        let material = g.create_material(MaterialOptions::new(meshes.default_material().shader()));
        let mesh = g.create_mesh(
            &[
                Vertex::new([-1., -1., 0.], [1., 0., 0., 1.]),
                Vertex::new([3., -1., 0.], [1., 0., 0., 1.]),
                Vertex::new([-1., 3., 0.], [1., 0., 0., 1.]),
            ],
            &[0, 1, 2],
        )?;
        let mut textures = TextureRenderer::new(&g);
        let image = g.create_texture(TextureOptions::new(1, 1))?;
        image.write(&[0, 255, 0, 255])?;
        let binding = textures.create_binding(image.view(), TextureBindingOptions::new())?;
        let texture_material = g.create_texture_material(TextureMaterialOptions::new());
        meshes.prepare_material(&material, &format)?;
        textures.prepare_material(&binding, &texture_material, &format)?;
        // Validate the same workloads before and after cache churn. No rendering,
        // submission, readback, or statistics scan is inside the warm lookup timer.
        for before in [true, false] {
            {
                let mut frame = target.begin_frame()?;
                meshes.draw_with_material(&mut frame.render_pass().begin()?, &mesh, &material)?;
                frame.finish()?;
            }
            assert_eq!(
                &target.read_rgba8()?[(8 * 16 + 8) * 4..][..4],
                &[255, 0, 0, 255]
            );
            {
                let mut frame = target.begin_frame()?;
                textures.draw_with_material(
                    &mut frame.render_pass().begin()?,
                    &binding,
                    &texture_material,
                    TextureDraw::default(),
                )?;
                frame.finish()?;
            }
            assert_eq!(
                &target.read_rgba8()?[(8 * 16 + 8) * 4..][..4],
                &[0, 255, 0, 255]
            );
            if before {
                println!("renderer,operation,median_ns_per_variant,p95_ns_per_variant");
                measure(
                    "mesh",
                    &mut meshes,
                    |r| {
                        r.prepare_material(black_box(&material), black_box(&format))
                            .unwrap()
                    },
                    |r| {
                        let m = g.create_material(MaterialOptions::new(material.shader()));
                        r.prepare_material(&m, &format).unwrap();
                    },
                    MeshRenderer::prune_pipeline_cache,
                    MeshRenderer::pipeline_cache_stats,
                );
                measure(
                    "texture",
                    &mut textures,
                    |r| {
                        r.prepare_material(
                            black_box(&binding),
                            black_box(&texture_material),
                            black_box(&format),
                        )
                        .unwrap()
                    },
                    |r| {
                        let m = g.create_texture_material(TextureMaterialOptions::new());
                        r.prepare_material(&binding, &m, &format).unwrap();
                    },
                    TextureRenderer::prune_pipeline_cache,
                    TextureRenderer::pipeline_cache_stats,
                );
            }
        }
        assert!(validation.pop().await.is_none());
        eprintln!(
            "pixel_checks=passed; every churn batch pruned 16 stale variants and retained 1 live variant"
        );
        Ok(())
    })
}
