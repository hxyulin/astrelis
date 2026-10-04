use std::collections::{HashMap, hash_map::Entry};

use crate::{Error, GraphicsContext, Material, MaterialOptions, Mesh, RenderPass, Vertex};

type Pipelines =
    HashMap<(u64, wgpu::TextureFormat, u32, Option<wgpu::TextureFormat>), wgpu::RenderPipeline>;

/// A device-bound renderer for indexed triangle meshes, with optional materials.
///
/// Construct with [`Self::new`]. This rendering component caches pipelines by
/// material, color format, sample count, and depth/stencil format. The default material interpolates
/// vertex colors with premultiplied alpha blending. Custom materials select their
/// own shader, blending, face culling, and depth/stencil state. Default shading
/// disables depth/stencil tests and writes, including on targets with these attachments.
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

    /// Borrows the default vertex-color material, including its reusable shader.
    ///
    /// Use its shader with [`MaterialOptions`] to create a colored-mesh material
    /// with custom depth/stencil state without writing a new shader.
    pub fn default_material(&self) -> &Material {
        &self.material
    }

    /// Prepares default shading for a target, including its optional depth/stencil format.
    ///
    /// Default shading does not test or write depth/stencil. It uses a compatible
    /// inert pipeline state when an attachment is present, allowing 2D overlays
    /// to share a depth-enabled pass. Errors follow [`Self::prepare`].
    /// A target on another device returns [`Error::DeviceMismatch`].
    pub fn prepare_for_target(&mut self, target: &crate::RenderTarget<'_>) -> Result<(), Error> {
        if target.device() != self.graphics.device() {
            return Err(Error::DeviceMismatch);
        }
        pipeline(
            &self.graphics,
            &mut self.pipelines,
            &self.material,
            target.format(),
            target.sample_count(),
            target.depth_stencil_format(),
            true,
        )?;
        Ok(())
    }

    /// Prepares a material against a target's full attachment configuration.
    ///
    /// This checks explicit depth/stencil format requirements before creating a
    /// pipeline, and also supports materials with depth/stencil tests disabled.
    /// A material or target from another device returns [`Error::DeviceMismatch`].
    /// An explicit format mismatch returns [`Error::DepthStencilMismatch`].
    /// Other errors and shader validation follow [`Self::prepare_material`].
    pub fn prepare_material_for_target(
        &mut self,
        material: &Material,
        target: &crate::RenderTarget<'_>,
    ) -> Result<(), Error> {
        if &material.device != self.graphics.device() || target.device() != self.graphics.device() {
            return Err(Error::DeviceMismatch);
        }
        pipeline(
            &self.graphics,
            &mut self.pipelines,
            material,
            target.format(),
            target.sample_count(),
            target.depth_stencil_format(),
            false,
        )?;
        Ok(())
    }

    /// Prepares the default material's pipeline for a target format and MSAA count.
    ///
    /// This prepares a color-only pipeline. Use [`Self::prepare_for_target`] for
    /// targets with depth/stencil attachments. Call with `target.format()` and
    /// `target.sample_count()` before rendering,
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
            None,
            true,
        )?;
        Ok(())
    }

    /// Prepares a custom material's pipeline without acquiring a frame.
    ///
    /// Pipelines are cached independently for each immutable material identity,
    /// color format, sample count, and depth/stencil format. This method uses the
    /// material's explicit depth/stencil format, or none; use
    /// [`Self::prepare_material_for_target`] when disabled tests share a target
    /// that has depth/stencil storage. Cloned materials share entries. Resource
    /// bindings and their contents are not part of the key; changing a uniform,
    /// texture, or dynamic offset does not create another pipeline.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DeviceMismatch`] for a material on another device,
    /// [`Error::UnsupportedColorFormat`] for an unavailable color attachment,
    /// [`Error::UnsupportedMaterialFormat`] if the selected blending cannot be
    /// used with the format, [`Error::UnsupportedDepthStencilFormat`] for invalid
    /// or unavailable depth/stencil formats, or [`Error::UnsupportedSampleCount`]. Shader entry
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
            material.depth_stencil.as_ref().map(|state| state.format),
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
            pass.depth_stencil_format(),
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
    /// Depth/stencil tests follow the material. Dynamic stencil reference follows
    /// the pass's wrapped setter; raw reference changes are restored on each draw.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DeviceMismatch`] before recording if the pass, mesh, or
    /// material belongs to another device. Format errors follow
    /// [`Self::prepare_material`]. wgpu validates shader interfaces at pipeline
    /// creation and application bindings when recording; those errors use wgpu's
    /// error reporting rather than this return value.
    /// An explicit material depth/stencil format must match the pass or returns
    /// [`Error::DepthStencilMismatch`]. Writes to read-only aspects return
    /// [`Error::ReadOnlyDepth`] or [`Error::ReadOnlyStencil`] without a draw.
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
        if let Some(state) = &material.depth_stencil {
            if pass.depth_read_only() && !state.is_depth_read_only() {
                return Err(Error::ReadOnlyDepth);
            }
            if pass.stencil_read_only() && !state.is_stencil_read_only(material.cull_mode) {
                return Err(Error::ReadOnlyStencil);
            }
        }
        let pipeline = pipeline(
            &self.graphics,
            &mut self.pipelines,
            material,
            pass.format(),
            pass.sample_count(),
            pass.depth_stencil_format(),
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
    depth_stencil_format: Option<wgpu::TextureFormat>,
    default_material: bool,
) -> Result<&'cache wgpu::RenderPipeline, Error> {
    if let Some(state) = &material.depth_stencil
        && Some(state.format) != depth_stencil_format
    {
        return Err(Error::DepthStencilMismatch {
            material: state.format,
            pass: depth_stencil_format,
        });
    }
    let entry = match pipelines.entry((material.id, format, sample_count, depth_stencil_format)) {
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
    if let Some(depth_format) = depth_stencil_format {
        let counts = crate::depth_stencil::supported_counts(
            graphics,
            depth_format,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        )?;
        crate::target::validate_sample_count(depth_format, sample_count, &counts)?;
    }
    let depth_stencil = material.depth_stencil.clone().or_else(|| {
        depth_stencil_format.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: format.has_depth_aspect().then_some(false),
            depth_compare: None,
            stencil: Default::default(),
            bias: Default::default(),
        })
    });
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
                depth_stencil,
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

#[cfg(test)]
#[path = "depth_stencil_tests.rs"]
mod depth_stencil_tests;
