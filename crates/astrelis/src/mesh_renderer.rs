use std::collections::{HashMap, hash_map::Entry};

use crate::{Error, GraphicsContext, Material, MaterialOptions, Mesh, RenderPass, Vertex};

type Pipelines = HashMap<(u64, wgpu::TextureFormat, u32), wgpu::RenderPipeline>;

/// A device-bound renderer for indexed triangle meshes, with optional materials.
///
/// Construct with [`Self::new`]. This rendering component caches pipelines by
/// material, target format, and sample count. The default material interpolates
/// vertex colors with premultiplied alpha blending. Custom materials select their
/// own shader, blending, and face culling. There is no depth testing.
///
/// Renderers own no windows or frames. Independent renderers can share a pass;
/// frame acquisition, pass configuration, submission, and presentation belong to
/// the application. Use [`Self::prepare`] or [`Self::prepare_material`] during
/// loading to create pipelines before the first draw that needs them.
#[derive(Debug)]
pub struct MeshRenderer {
    graphics: GraphicsContext,
    material: Material,
    pipelines: Pipelines,
}

impl MeshRenderer {
    /// Creates a renderer with the default colored-mesh material and empty cache.
    pub fn new(graphics: &GraphicsContext) -> Self {
        let shader = graphics
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Astrelis colored mesh shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("mesh.wgsl").into()),
            });
        Self {
            graphics: graphics.clone(),
            material: graphics.create_material(MaterialOptions::new(&shader)),
            pipelines: HashMap::new(),
        }
    }

    /// Prepares the default material's pipeline for a target format and MSAA count.
    ///
    /// Call with `target.format()` and `target.sample_count()` before rendering,
    /// or prepare a supported count before changing MSAA. Repeated calls reuse
    /// the cached pipeline, with no capability queries or GPU resource creation.
    /// This creates the wgpu pipeline without recording or submitting commands;
    /// it does not guarantee that the driver performs all work before a draw.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnsupportedMeshFormat`] for formats incompatible with
    /// floating-point output or alpha blending, [`Error::UnsupportedColorFormat`]
    /// for unavailable color formats, or [`Error::UnsupportedSampleCount`].
    pub fn prepare(&mut self, format: wgpu::TextureFormat, sample_count: u32) -> Result<(), Error> {
        pipeline(
            &self.graphics,
            &mut self.pipelines,
            &self.material,
            format,
            sample_count,
            true,
        )?;
        Ok(())
    }

    /// Prepares a custom material's pipeline without acquiring a frame.
    ///
    /// Pipelines are cached independently for each immutable material identity,
    /// format, and sample count. Cloned materials share cache entries. Resource
    /// bindings and their contents are not part of the key; changing a uniform,
    /// texture, or dynamic offset does not create another pipeline.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DeviceMismatch`] for a material on another device,
    /// [`Error::UnsupportedColorFormat`] for an unavailable color attachment,
    /// [`Error::UnsupportedMaterialFormat`] if the selected blending cannot be
    /// used with the format, or [`Error::UnsupportedSampleCount`]. Shader entry
    /// points, vertex/fragment interfaces, and binding layouts are validated by
    /// wgpu when the pipeline is created, using its error scopes or uncaptured
    /// error handler. Capture a wgpu validation scope around preparation when
    /// loading application shaders that may fail validation.
    pub fn prepare_material(
        &mut self,
        material: &Material,
        format: wgpu::TextureFormat,
        sample_count: u32,
    ) -> Result<(), Error> {
        if &material.device != self.graphics.device() {
            return Err(Error::DeviceMismatch);
        }
        pipeline(
            &self.graphics,
            &mut self.pipelines,
            material,
            format,
            sample_count,
            false,
        )?;
        Ok(())
    }

    /// Records one mesh draw using interpolated, premultiplied vertex colors.
    ///
    /// Does not acquire, clear, submit, present, or reupload mesh data. Each draw
    /// restores its pipeline and geometry bindings and applies the pass's chosen
    /// viewport and scissor. Pipelines are created lazily unless prepared earlier.
    /// Any number of renderers can contribute in application-defined order.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DeviceMismatch`] if the pass or mesh belongs to another
    /// device, or [`Error::UnsupportedMeshFormat`] for integer or nonblendable
    /// attachments. Rejected operations record no draw.
    pub fn draw(&mut self, pass: &mut RenderPass<'_>, mesh: &Mesh) -> Result<(), Error> {
        self.validate_devices(pass, mesh)?;
        let pipeline = pipeline(
            &self.graphics,
            &mut self.pipelines,
            &self.material,
            pass.format(),
            pass.sample_count(),
            true,
        )?;
        draw(pass, mesh, pipeline);
        Ok(())
    }

    /// Records one mesh draw using an application-created material.
    ///
    /// Bind this material's resource groups through [`RenderPass::as_wgpu`]
    /// before drawing. Bindings can vary between draws, including dynamic uniform
    /// offsets. The material supplies the pipeline; geometry and pass raster
    /// settings are restored just as in [`Self::draw`]. Mesh vertices remain the
    /// fixed [`Vertex`] layout; a custom vertex shader can transform positions.
    /// Integer fragment outputs are supported with a matching format and no blend.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DeviceMismatch`] before recording if the pass, mesh, or
    /// material belongs to another device. Format errors follow
    /// [`Self::prepare_material`]. wgpu validates shader interfaces at pipeline
    /// creation and application bindings when recording; those errors use wgpu's
    /// error reporting rather than this return value.
    pub fn draw_with_material(
        &mut self,
        pass: &mut RenderPass<'_>,
        mesh: &Mesh,
        material: &Material,
    ) -> Result<(), Error> {
        self.validate_devices(pass, mesh)?;
        if &material.device != self.graphics.device() {
            return Err(Error::DeviceMismatch);
        }
        let pipeline = pipeline(
            &self.graphics,
            &mut self.pipelines,
            material,
            pass.format(),
            pass.sample_count(),
            false,
        )?;
        draw(pass, mesh, pipeline);
        Ok(())
    }

    fn validate_devices(&self, pass: &RenderPass<'_>, mesh: &Mesh) -> Result<(), Error> {
        if pass.device() != self.graphics.device() || &mesh.device != self.graphics.device() {
            return Err(Error::DeviceMismatch);
        }
        Ok(())
    }
}

fn pipeline<'cache>(
    graphics: &GraphicsContext,
    pipelines: &'cache mut Pipelines,
    material: &Material,
    format: wgpu::TextureFormat,
    sample_count: u32,
    default_material: bool,
) -> Result<&'cache wgpu::RenderPipeline, Error> {
    let entry = match pipelines.entry((material.id, format, sample_count)) {
        Entry::Occupied(entry) => return Ok(entry.into_mut()),
        Entry::Vacant(entry) => entry,
    };
    let features = crate::target::format_features(graphics, format);
    if format.is_depth_stencil_format()
        || format.is_multi_planar_format()
        || !graphics
            .device()
            .features()
            .contains(format.required_features())
        || !features
            .allowed_usages
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
    {
        return Err(Error::UnsupportedColorFormat { format });
    }
    if default_material
        && (!matches!(
            format.sample_type(None, Some(graphics.device().features())),
            Some(wgpu::TextureSampleType::Float { .. })
        ) || !features
            .flags
            .contains(wgpu::TextureFormatFeatureFlags::BLENDABLE))
    {
        return Err(Error::UnsupportedMeshFormat { format });
    }
    if material.blend.is_some()
        && !features
            .flags
            .contains(wgpu::TextureFormatFeatureFlags::BLENDABLE)
    {
        return Err(Error::UnsupportedMaterialFormat { format });
    }
    crate::target::validate_sample_count(
        format,
        sample_count,
        &crate::target::sample_counts(features),
    )?;
    Ok(entry.insert(
        graphics
            .device()
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Astrelis mesh material pipeline"),
                layout: Some(&material.layout),
                vertex: wgpu::VertexState {
                    module: &material.shader,
                    entry_point: Some(&material.vertex_entry),
                    compilation_options: Default::default(),
                    buffers: &[Some(Vertex::layout())],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &material.shader,
                    entry_point: Some(&material.fragment_entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: material.blend,
                        write_mask: material.write_mask,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    front_face: material.front_face,
                    cull_mode: material.cull_mode,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: sample_count,
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            }),
    ))
}

fn draw(pass: &mut RenderPass<'_>, mesh: &Mesh, pipeline: &wgpu::RenderPipeline) {
    pass.apply_raster_state();
    pass.inner.set_pipeline(pipeline);
    pass.inner
        .set_vertex_buffer(0, mesh.vertex_buffer().slice(..));
    pass.inner
        .set_index_buffer(mesh.index_buffer().slice(..), wgpu::IndexFormat::Uint32);
    pass.inner.draw_indexed(0..mesh.index_count(), 0, 0..1);
}

#[cfg(test)]
#[path = "mesh_renderer_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "material_tests.rs"]
mod material_tests;
