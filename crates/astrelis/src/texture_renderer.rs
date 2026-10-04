use std::collections::{HashMap, hash_map::Entry};

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::{Error, GraphicsContext, RenderPass, RenderTarget};

/// How the source texture stores alpha, independently of destination blending.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextureAlpha {
    /// Source RGB is independent of alpha, as in most uploaded image files.
    #[default]
    Straight,
    /// Source RGB already contains alpha, as in the built-in mesh framebuffer output.
    Premultiplied,
}

/// Sampling filter for a single-mip color texture. Addressing clamps to its edges.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextureFilter {
    /// Chooses the nearest texel, including on unfilterable float formats.
    Nearest,
    /// Interpolates neighboring texels. Requires a filterable source format.
    #[default]
    Linear,
}

/// How a texture draw combines its premultiplied output with existing color.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextureBlend {
    /// Premultiplied source-over alpha blending.
    #[default]
    Alpha,
    /// Replaces the destination with premultiplied output, including its alpha.
    Replace,
}

/// Immutable settings uploaded when preparing a [`TextureBinding`].
///
/// Rectangles are `[x, y, width, height]`, with nonnegative extents. Destination
/// coordinates are relative to the active pass viewport: `(0, 0)` is top-left and
/// `(1, 1)` bottom-right. Values outside that range are clipped by the pass.
/// Source coordinates are normalized texture UVs, also top-left based; coordinates
/// outside the texture clamp to its edges. Zero destination extents produce no
/// visible area; a zero source extent samples a point or line.
/// Rectangles and tint must be finite. Tint uses linear RGB and straight alpha;
/// alpha must be in `0..=1`. The default draws the full image over the full viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
#[must_use = "pass these options to TextureRenderer::create_binding"]
pub struct TextureDrawOptions {
    /// Destination rectangle in viewport-relative coordinates.
    pub destination: [f32; 4],
    /// Source rectangle in normalized UV coordinates.
    pub source: [f32; 4],
    /// Linear RGB tint and opacity, applied to premultiplied output.
    pub tint: [f32; 4],
    /// Source alpha interpretation.
    pub alpha: TextureAlpha,
    /// Sampling filter.
    pub filter: TextureFilter,
    /// Destination blending.
    pub blend: TextureBlend,
}

impl Default for TextureDrawOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl TextureDrawOptions {
    /// Selects a full-image draw with straight source alpha, linear filtering, and alpha blending.
    pub const fn new() -> Self {
        Self {
            destination: [0.0, 0.0, 1.0, 1.0],
            source: [0.0, 0.0, 1.0, 1.0],
            tint: [1.0; 4],
            alpha: TextureAlpha::Straight,
            filter: TextureFilter::Linear,
            blend: TextureBlend::Alpha,
        }
    }
    /// Selects the viewport-relative destination rectangle.
    pub const fn destination(mut self, rectangle: [f32; 4]) -> Self {
        self.destination = rectangle;
        self
    }
    /// Selects the normalized source crop.
    pub const fn source(mut self, rectangle: [f32; 4]) -> Self {
        self.source = rectangle;
        self
    }
    /// Selects a linear RGB tint and straight opacity.
    pub const fn tint(mut self, tint: [f32; 4]) -> Self {
        self.tint = tint;
        self
    }
    /// Selects straight or premultiplied source pixels.
    pub const fn alpha(mut self, alpha: TextureAlpha) -> Self {
        self.alpha = alpha;
        self
    }
    /// Selects nearest or linear sampling.
    pub const fn filter(mut self, filter: TextureFilter) -> Self {
        self.filter = filter;
        self
    }
    /// Selects alpha compositing or replacement.
    pub const fn blend(mut self, blend: TextureBlend) -> Self {
        self.blend = blend;
        self
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Parameters {
    destination: [f32; 4],
    source: [f32; 4],
    tint: [f32; 4],
}

/// A reusable texture source and prepared rectangle, tint, sampler, and bind group.
///
/// Create with [`TextureRenderer::create_binding`]. Preparation uploads immutable
/// draw parameters once; repeated draws create no GPU resources or uploads. Create another
/// binding to change settings. Bindings can be shared by texture renderers on the
/// same device and cloned without copying resources.
///
/// The binding retains the source view. When a framebuffer replaces its storage
/// after resize or MSAA changes, call [`TextureRenderer::rebind`] explicitly.
/// Pixel uploads to existing storage need no rebind. Raw view dimensions, devices,
/// and view-format compatibility use wgpu validation at binding creation.
#[derive(Clone, Debug)]
pub struct TextureBinding {
    device: wgpu::Device,
    instance: wgpu::Instance,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    parameters: wgpu::Buffer,
    group: wgpu::BindGroup,
    options: TextureDrawOptions,
}

impl TextureBinding {
    /// Returns the immutable settings used by this binding.
    pub fn options(&self) -> TextureDrawOptions {
        self.options
    }
    /// Returns the retained source view, including after its originating target resizes.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct PipelineKey {
    format: wgpu::TextureFormat,
    count: u32,
    depth_stencil: Option<wgpu::TextureFormat>,
    filter: TextureFilter,
    alpha: TextureAlpha,
    blend: TextureBlend,
}

/// Draws prepared texture rectangles into application-controlled render passes.
///
/// Construct independently with [`Self::new`]. This renderer owns no target or
/// window and does not clear, submit, or present. Pipeline keys include target
/// formats, MSAA count, filtering, source alpha, and blending. Prepare with
/// [`Self::prepare_for_target`] to avoid pipeline creation on first draw.
/// Depth/stencil tests and writes are disabled, allowing overlays in 3D passes.
/// Drawing restores wrapped viewport/scissor/reference state and its own bindings,
/// so mesh, texture, and application renderers can be interleaved in one pass.
#[derive(Debug)]
pub struct TextureRenderer {
    graphics: GraphicsContext,
    shader: wgpu::ShaderModule,
    nearest: wgpu::BindGroupLayout,
    linear: wgpu::BindGroupLayout,
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
}

impl TextureRenderer {
    /// Creates the shader and binding layouts, with an empty pipeline cache.
    pub fn new(graphics: &GraphicsContext) -> Self {
        let device = graphics.device();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Astrelis texture shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("texture.wgsl").into()),
        });
        Self {
            graphics: graphics.clone(),
            shader,
            nearest: binding_layout(device, false),
            linear: binding_layout(device, true),
            pipelines: HashMap::new(),
        }
    }

    /// Prepares a sampled 2D view with immutable draw settings.
    ///
    /// Accepts [`crate::Texture::view`], [`crate::Framebuffer::color_view`], and
    /// application-created views. Source storage must be single-sampled, have
    /// `TEXTURE_BINDING`, and supply float color texels. Linear filtering additionally
    /// needs a filterable format. Nearest filtering accepts unfilterable float formats.
    /// Settings and observable source properties are validated before allocation.
    /// Raw view device/dimension/format mismatches use wgpu error reporting.
    pub fn create_binding(
        &self,
        view: &wgpu::TextureView,
        options: TextureDrawOptions,
    ) -> Result<TextureBinding, Error> {
        validate_options(options)?;
        self.validate_source(view, options.filter)?;
        let device = self.graphics.device();
        let filter = match options.filter {
            TextureFilter::Nearest => wgpu::FilterMode::Nearest,
            TextureFilter::Linear => wgpu::FilterMode::Linear,
        };
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Astrelis texture sampler"),
            mag_filter: filter,
            min_filter: filter,
            ..Default::default()
        });
        let parameters = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Astrelis texture rectangle"),
            contents: bytemuck::bytes_of(&Parameters {
                destination: options.destination,
                source: options.source,
                tint: options.tint,
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let group = bind(
            device,
            self.layout(options.filter),
            view,
            &sampler,
            &parameters,
        );
        Ok(TextureBinding {
            device: device.clone(),
            instance: self.graphics.instance().clone(),
            view: view.clone(),
            sampler,
            parameters,
            group,
            options,
        })
    }

    /// Refreshes a binding's source view while retaining its settings, sampler, and parameters.
    ///
    /// Repeating with the same view does no GPU work. Returned errors preserve the
    /// previous binding. A new view creates one bind group, without pixel uploads.
    /// Previously recorded draws retain the old bind group; recording is a snapshot
    /// of bindings, unlike queue uploads into the same underlying texture storage.
    pub fn rebind(
        &self,
        binding: &mut TextureBinding,
        view: &wgpu::TextureView,
    ) -> Result<(), Error> {
        if &binding.device != self.graphics.device()
            || &binding.instance != self.graphics.instance()
        {
            return Err(Error::DeviceMismatch);
        }
        self.validate_source(view, binding.options.filter)?;
        if &binding.view == view {
            return Ok(());
        }
        let group = bind(
            self.graphics.device(),
            self.layout(binding.options.filter),
            view,
            &binding.sampler,
            &binding.parameters,
        );
        binding.group = group;
        binding.view = view.clone();
        Ok(())
    }

    /// Warms a binding's pipeline for a complete target configuration.
    ///
    /// Repeated preparation reuses the cache without capability queries or resource
    /// creation. Source rectangles and tint do not create pipeline variants.
    /// Returns `DeviceMismatch` for foreign bindings/targets, or format/sample
    /// errors for unsupported destinations. Raw shader validation follows wgpu.
    pub fn prepare_for_target(
        &mut self,
        binding: &TextureBinding,
        target: &RenderTarget<'_>,
    ) -> Result<(), Error> {
        if !target.graphics().same_device(&self.graphics)
            || (&binding.device != self.graphics.device()
                || &binding.instance != self.graphics.instance())
        {
            return Err(Error::DeviceMismatch);
        }
        self.pipeline(key(
            binding,
            target.format(),
            target.sample_count(),
            target.depth_stencil_format(),
        ))?;
        Ok(())
    }

    /// Warms a binding's pipeline for explicit attachment formats and sample count.
    ///
    /// Useful for framebuffers before acquiring a recording, or before changing MSAA.
    /// Validation and caching follow [`Self::prepare_for_target`].
    pub fn prepare(
        &mut self,
        binding: &TextureBinding,
        format: wgpu::TextureFormat,
        count: u32,
        depth_stencil: Option<wgpu::TextureFormat>,
    ) -> Result<(), Error> {
        if &binding.device != self.graphics.device()
            || &binding.instance != self.graphics.instance()
        {
            return Err(Error::DeviceMismatch);
        }
        self.pipeline(key(binding, format, count, depth_stencil))?;
        Ok(())
    }

    /// Draws one prepared rectangle, without parameter uploads or submission.
    ///
    /// A missing pipeline is created lazily. Prepare first to avoid GPU resource creation.
    /// Rectangles follow the current viewport; clipping follows the current scissor.
    /// Source storage must not alias an active color/resolve attachment. Rejected
    /// device, feedback, or destination format/sample checks record no draw.
    /// Other custom-view validation follows wgpu's error reporting.
    pub fn draw(
        &mut self,
        pass: &mut RenderPass<'_>,
        binding: &TextureBinding,
    ) -> Result<(), Error> {
        if !pass.same_device(&self.graphics)
            || (&binding.device != self.graphics.device()
                || &binding.instance != self.graphics.instance())
        {
            return Err(Error::DeviceMismatch);
        }
        if pass.uses_color_texture(binding.view.texture()) {
            return Err(Error::TextureFeedback);
        }
        let key = key(
            binding,
            pass.format(),
            pass.sample_count(),
            pass.depth_stencil_format(),
        );
        let pipeline = self.pipeline(key)?;
        pass.apply_raster_state();
        pass.inner.set_pipeline(pipeline);
        pass.inner.set_bind_group(0, &binding.group, &[]);
        pass.inner.draw(0..6, 0..1);
        Ok(())
    }

    fn layout(&self, filter: TextureFilter) -> &wgpu::BindGroupLayout {
        match filter {
            TextureFilter::Nearest => &self.nearest,
            TextureFilter::Linear => &self.linear,
        }
    }

    fn validate_source(
        &self,
        view: &wgpu::TextureView,
        filter: TextureFilter,
    ) -> Result<(), Error> {
        let texture = view.texture();
        let format = texture.format();
        let float = format.sample_type(None, Some(self.graphics.device().features()));
        if texture.dimension() != wgpu::TextureDimension::D2
            || texture.sample_count() != 1
            || !texture
                .usage()
                .contains(wgpu::TextureUsages::TEXTURE_BINDING)
            || !matches!(float, Some(wgpu::TextureSampleType::Float { .. }))
            || (filter == TextureFilter::Linear
                && !matches!(
                    float,
                    Some(wgpu::TextureSampleType::Float { filterable: true })
                ))
        {
            return Err(Error::InvalidTextureSource { format });
        }
        Ok(())
    }

    fn pipeline(&mut self, key: PipelineKey) -> Result<&wgpu::RenderPipeline, Error> {
        let layout = match key.filter {
            TextureFilter::Nearest => &self.nearest,
            TextureFilter::Linear => &self.linear,
        };
        let entry = match self.pipelines.entry(key) {
            Entry::Occupied(entry) => return Ok(entry.into_mut()),
            Entry::Vacant(entry) => entry,
        };
        let features = crate::target::format_features(&self.graphics, key.format);
        if !self
            .graphics
            .device()
            .features()
            .contains(key.format.required_features())
            || !features
                .allowed_usages
                .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
            || !matches!(
                key.format
                    .sample_type(None, Some(self.graphics.device().features())),
                Some(wgpu::TextureSampleType::Float { .. })
            )
        {
            return Err(Error::UnsupportedColorFormat { format: key.format });
        }
        if key.blend == TextureBlend::Alpha
            && !features
                .flags
                .contains(wgpu::TextureFormatFeatureFlags::BLENDABLE)
        {
            return Err(Error::UnsupportedMaterialFormat { format: key.format });
        }
        crate::target::validate_sample_count(
            key.format,
            key.count,
            &crate::target::sample_counts(features),
        )?;
        if let Some(format) = key.depth_stencil {
            let counts = crate::depth_stencil::supported_counts(
                &self.graphics,
                format,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            )?;
            crate::target::validate_sample_count(format, key.count, &counts)?;
        }
        let device = self.graphics.device();
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Astrelis texture pipeline layout"),
            bind_group_layouts: &[Some(layout)],
            immediate_size: 0,
        });
        Ok(entry.insert(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Astrelis texture pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vertex_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some(match key.alpha {
                        TextureAlpha::Straight => "fragment_straight",
                        TextureAlpha::Premultiplied => "fragment_premultiplied",
                    }),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: key.format,
                        blend: (key.blend == TextureBlend::Alpha)
                            .then_some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: key.depth_stencil.map(|format| wgpu::DepthStencilState {
                    format,
                    depth_write_enabled: format.has_depth_aspect().then_some(false),
                    depth_compare: None,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: key.count,
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            }),
        ))
    }
}

fn key(
    binding: &TextureBinding,
    format: wgpu::TextureFormat,
    count: u32,
    depth_stencil: Option<wgpu::TextureFormat>,
) -> PipelineKey {
    PipelineKey {
        format,
        count,
        depth_stencil,
        filter: binding.options.filter,
        alpha: binding.options.alpha,
        blend: binding.options.blend,
    }
}

fn validate_options(options: TextureDrawOptions) -> Result<(), Error> {
    let valid_rect = |r: [f32; 4]| {
        r.iter().all(|v| v.is_finite())
            && r[2] >= 0.0
            && r[3] >= 0.0
            && (r[0] + r[2]).is_finite()
            && (r[1] + r[3]).is_finite()
    };
    if !valid_rect(options.destination)
        || !valid_rect(options.source)
        || !options.tint.iter().all(|v| v.is_finite())
        || !(0.0..=1.0).contains(&options.tint[3])
    {
        return Err(Error::InvalidTextureDraw);
    }
    Ok(())
}

fn binding_layout(device: &wgpu::Device, filtering: bool) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Astrelis texture binding layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float {
                        filterable: filtering,
                    },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(if filtering {
                    wgpu::SamplerBindingType::Filtering
                } else {
                    wgpu::SamplerBindingType::NonFiltering
                }),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<Parameters>() as u64
                    ),
                },
                count: None,
            },
        ],
    })
}

fn bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    parameters: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Astrelis texture draw binding"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: parameters.as_entire_binding(),
            },
        ],
    })
}

#[cfg(test)]
#[path = "texture_tests.rs"]
mod tests;
