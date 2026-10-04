use std::collections::{HashMap, hash_map::Entry};

use crate::{Error, GraphicsContext, Material, MaterialOptions, Mesh, RenderPass};

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
        if !target.graphics().same_device(&self.graphics) {
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
        if (&material.device != self.graphics.device()
            || &material.instance != self.graphics.instance())
            || !target.graphics().same_device(&self.graphics)
        {
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
        if &material.device != self.graphics.device()
            || &material.instance != self.graphics.instance()
        {
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
        self.draw_range(pass, mesh, &mesh.full_draw())
    }

    /// Records a selected geometry range and instance range using default shading.
    pub fn draw_range(
        &mut self,
        pass: &mut RenderPass<'_>,
        mesh: &Mesh,
        draw: &crate::MeshDraw,
    ) -> Result<(), Error> {
        self.validate_devices(pass, mesh)?;
        mesh.validate_draw(&self.material, draw)?;
        let pipeline = pipeline(
            &self.graphics,
            &mut self.pipelines,
            &self.material,
            pass.single_color_format()?,
            pass.sample_count(),
            pass.depth_stencil_format(),
            true,
        )?;
        pass.apply_raster_state();
        pass.set_pipeline(pipeline);
        mesh.record(pass, draw);
        Ok(())
    }

    /// Binds one mesh and default material for repeated drawing within this pass.
    ///
    /// Device/layout checks, pipeline selection, and resource binding happen once.
    /// The scope borrows the mesh and pass, retains a pipeline handle, and creates no GPU resources
    /// when the pipeline is prepared. Use [`MeshDrawSession::pass`] for clipping,
    /// application bindings, or other renderers; the next scoped draw restores its
    /// pipeline and geometry. The renderer can be reused through the scoped pass.
    /// Dropping the scope leaves the pass open.
    pub fn bind<'draw, 'frame>(
        &mut self,
        pass: &'draw mut RenderPass<'frame>,
        mesh: &'draw Mesh,
    ) -> Result<MeshDrawSession<'draw, 'frame>, Error> {
        self.validate_devices(pass, mesh)?;
        let full_draw = mesh.full_draw();
        mesh.validate_draw(&self.material, &full_draw)?;
        let pipeline = pipeline(
            &self.graphics,
            &mut self.pipelines,
            &self.material,
            pass.single_color_format()?,
            pass.sample_count(),
            pass.depth_stencil_format(),
            true,
        )?;
        pass.apply_raster_state();
        pass.set_pipeline(pipeline);
        mesh.bind(pass);
        Ok(MeshDrawSession {
            pass,
            mesh,
            pipeline: pipeline.clone(),
            full_draw,
            dirty: false,
        })
    }

    /// Binds a mesh and explicit material for repeated scoped drawing.
    ///
    /// Errors match [`Self::draw_with_material`]. Application bind groups remain
    /// under caller control. Immutable compatibility is checked at binding; each
    /// [`MeshDrawSession::draw_range`] still validates its changing ranges.
    pub fn bind_with_material<'draw, 'frame>(
        &mut self,
        pass: &'draw mut RenderPass<'frame>,
        mesh: &'draw Mesh,
        material: &Material,
    ) -> Result<MeshDrawSession<'draw, 'frame>, Error> {
        self.validate_devices(pass, mesh)?;
        if &material.device != self.graphics.device()
            || &material.instance != self.graphics.instance()
        {
            return Err(Error::DeviceMismatch);
        }
        let full_draw = mesh.full_draw();
        mesh.validate_draw(material, &full_draw)?;
        validate_aspects(pass, material)?;
        let pipeline = pipeline(
            &self.graphics,
            &mut self.pipelines,
            material,
            pass.single_color_format()?,
            pass.sample_count(),
            pass.depth_stencil_format(),
            false,
        )?;
        pass.apply_raster_state();
        pass.set_pipeline(pipeline);
        mesh.bind(pass);
        Ok(MeshDrawSession {
            pass,
            mesh,
            pipeline: pipeline.clone(),
            full_draw,
            dirty: false,
        })
    }

    /// Records one mesh draw using an application-created material.
    ///
    /// Bind this material's resource groups through [`RenderPass::set_bind_group`]
    /// before drawing. Bindings can vary between draws, including dynamic uniform
    /// offsets. The material supplies the pipeline; geometry and pass raster
    /// settings are restored just as in [`Self::draw`]. Mesh layouts and topology must match the material; shaders may transform positions.
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
        self.draw_range_with_material(pass, mesh, material, &mesh.full_draw())
    }

    /// Records custom-layout geometry with explicit material, index/vertex range,
    /// and instances. Geometry layouts/topology must match material settings.
    pub fn draw_range_with_material(
        &mut self,
        pass: &mut RenderPass<'_>,
        mesh: &Mesh,
        material: &Material,
        draw: &crate::MeshDraw,
    ) -> Result<(), Error> {
        self.validate_devices(pass, mesh)?;
        if &material.device != self.graphics.device()
            || &material.instance != self.graphics.instance()
        {
            return Err(Error::DeviceMismatch);
        }
        mesh.validate_draw(material, draw)?;
        validate_aspects(pass, material)?;
        let pipeline = pipeline(
            &self.graphics,
            &mut self.pipelines,
            material,
            pass.single_color_format()?,
            pass.sample_count(),
            pass.depth_stencil_format(),
            false,
        )?;
        pass.apply_raster_state();
        pass.set_pipeline(pipeline);
        mesh.record(pass, draw);
        Ok(())
    }

    /// Prepares a material for a complete format value without acquiring a target.
    pub fn prepare_for_format(
        &mut self,
        material: &Material,
        format: &crate::RenderFormat,
    ) -> Result<(), Error> {
        if &material.device != self.graphics.device()
            || &material.instance != self.graphics.instance()
        {
            return Err(Error::DeviceMismatch);
        }
        let color = if format.colors.len() == 1 {
            format.colors[0]
        } else {
            None
        }
        .ok_or(Error::ExpectedSingleColor)?;
        pipeline(
            &self.graphics,
            &mut self.pipelines,
            material,
            color,
            format.sample_count,
            format.depth_stencil,
            false,
        )?;
        Ok(())
    }

    /// Captures wgpu validation diagnostics and caches only a successfully prepared pipeline.
    /// Shader modules and layouts can be checked separately with wgpu error scopes.
    pub async fn try_prepare_material(
        &mut self,
        material: &Material,
        format: &crate::RenderFormat,
    ) -> Result<(), Error> {
        if &material.device != self.graphics.device()
            || &material.instance != self.graphics.instance()
        {
            return Err(Error::DeviceMismatch);
        }
        let color = if format.colors.len() == 1 {
            format.colors[0]
        } else {
            None
        }
        .ok_or(Error::ExpectedSingleColor)?;
        let scope = self
            .graphics
            .device()
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let mut prepared = HashMap::new();
        let result = pipeline(
            &self.graphics,
            &mut prepared,
            material,
            color,
            format.sample_count,
            format.depth_stencil,
            false,
        )
        .map(|_| ());
        if let Some(error) = scope.pop().await {
            return Err(Error::Validation(error));
        }
        result?;
        self.pipelines.extend(prepared);
        Ok(())
    }

    fn validate_devices(&self, pass: &RenderPass<'_>, mesh: &Mesh) -> Result<(), Error> {
        if !pass.same_device(&self.graphics)
            || &mesh.device != self.graphics.device()
            || &mesh.instance != self.graphics.instance()
        {
            return Err(Error::DeviceMismatch);
        }
        Ok(())
    }
}

/// A scoped mesh binding with immutable compatibility validated once.
///
/// Obtain through [`MeshRenderer::bind`] or [`MeshRenderer::bind_with_material`].
/// Full draws emit commands directly; selected ranges retain bounds validation.
/// Pass access invalidates the scope's binding assumption, so subsequent draws
/// restore renderer-owned state. Application-owned bind groups are not restored.
/// The exclusive pass borrow prevents simultaneous direct use of the pass:
///
/// ```compile_fail
/// use astrelis::{Error, Mesh, MeshRenderer, RenderPass};
/// fn overlap(renderer: &mut MeshRenderer, pass: &mut RenderPass<'_>, mesh: &Mesh) -> Result<(), Error> {
///     let mut draws = renderer.bind(pass, mesh)?;
///     pass.set_stencil_reference(1);
///     draws.draw();
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct MeshDrawSession<'draw, 'frame> {
    pass: &'draw mut RenderPass<'frame>,
    mesh: &'draw Mesh,
    pipeline: wgpu::RenderPipeline,
    full_draw: crate::MeshDraw,
    dirty: bool,
}
impl<'draw, 'frame> MeshDrawSession<'draw, 'frame> {
    /// Draws the entire bound mesh with one instance, without repeating compatibility checks.
    #[inline]
    pub fn draw(&mut self) {
        self.restore();
        self.mesh.record_draw(self.pass, &self.full_draw);
    }
    /// Draws selected geometry/instances, checking bounds before recording a draw.
    /// Empty ranges follow the same semantics as [`MeshRenderer::draw_range`].
    pub fn draw_range(&mut self, draw: &crate::MeshDraw) -> Result<(), Error> {
        self.mesh.validate_range(draw)?;
        self.restore();
        self.mesh.record_draw(self.pass, draw);
        Ok(())
    }
    /// Borrows the pass for clipping, bindings, raw access, or another renderer.
    /// The next scoped draw restores its own pipeline/geometry and wrapped raster state.
    pub fn pass(&mut self) -> &mut RenderPass<'frame> {
        self.dirty = true;
        self.pass
    }
    #[inline]
    fn restore(&mut self) {
        if self.dirty {
            self.pass.apply_raster_state();
            self.pass.set_pipeline(&self.pipeline);
            self.mesh.bind(self.pass);
            self.dirty = false;
        }
    }
}
fn validate_aspects(pass: &RenderPass<'_>, material: &Material) -> Result<(), Error> {
    if let Some(state) = &material.depth_stencil {
        if pass.depth_read_only() && !state.is_depth_read_only() {
            return Err(Error::ReadOnlyDepth);
        }
        if pass.stencil_read_only() && !state.is_stencil_read_only(material.cull_mode) {
            return Err(Error::ReadOnlyStencil);
        }
    }
    Ok(())
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
    let vertex_layouts: Vec<_> = material
        .vertex_layouts
        .iter()
        .map(|l| Some(l.as_wgpu()))
        .collect();
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
                    buffers: &vertex_layouts,
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
                    topology: material.topology,
                    strip_index_format: material.strip_index_format,
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

#[cfg(test)]
#[path = "mesh_renderer_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "material_tests.rs"]
mod material_tests;

#[cfg(test)]
#[path = "depth_stencil_tests.rs"]
mod depth_stencil_tests;
