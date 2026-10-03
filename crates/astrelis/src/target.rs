use crate::{Error, GraphicsContext};

/// A rendering destination. This first version supports window surfaces only.
#[derive(Debug)]
pub enum RenderTarget<'window> {
    /// An owned surface with managed physical size and presentation configuration.
    Surface(SurfaceTarget<'window>),
}

/// Opaque state carried by [`RenderTarget::Surface`].
///
/// Construct and resize it through [`RenderTarget`]. Window creation and event
/// handling remain application-owned; this type has no winit dependency.
#[derive(Debug)]
pub struct SurfaceTarget<'window> {
    pub(crate) surface: wgpu::Surface<'window>,
    pub(crate) graphics: GraphicsContext,
    pub(crate) configuration: wgpu::SurfaceConfiguration,
    pub(crate) size: [u32; 2],
}

impl<'window> RenderTarget<'window> {
    /// Takes ownership of a wgpu surface and configures it for FIFO presentation.
    ///
    /// Width and height are physical pixels. A zero dimension suspends rendering
    /// until a nonzero resize. An sRGB surface format is preferred when supported.
    /// The surface must have been created by the context's instance. The
    /// application must not configure the same surface concurrently.
    /// Prefer [`GraphicsContext::create_surface`] for normal window initialization.
    pub fn surface(
        graphics: &GraphicsContext,
        surface: wgpu::Surface<'window>,
        width: u32,
        height: u32,
    ) -> Result<Self, Error> {
        validate_size(graphics, width, height)?;
        if !graphics.adapter().is_surface_supported(&surface) {
            return Err(Error::UnsupportedSurface);
        }
        let capabilities = surface.get_capabilities(graphics.adapter());
        let mut configuration = surface
            .get_default_config(graphics.adapter(), width.max(1), height.max(1))
            .ok_or(Error::UnsupportedSurface)?;
        if !capabilities
            .present_modes
            .contains(&wgpu::PresentMode::Fifo)
        {
            return Err(Error::UnsupportedSurface);
        }
        if let Some(format) = capabilities.formats.iter().find(|format| format.is_srgb()) {
            configuration.format = *format;
        }
        configuration.present_mode = wgpu::PresentMode::Fifo;
        let target = SurfaceTarget {
            surface,
            graphics: graphics.clone(),
            configuration,
            size: [width, height],
        };
        if width != 0 && height != 0 {
            target.configure();
        }
        Ok(Self::Surface(target))
    }

    /// Updates the physical size, configuring only nonzero, changed dimensions.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), Error> {
        let Self::Surface(target) = self;
        validate_size(&target.graphics, width, height)?;
        if target.size == [width, height] {
            return Ok(());
        }
        target.size = [width, height];
        if width != 0 && height != 0 {
            target.configuration.width = width;
            target.configuration.height = height;
            target.configure();
        }
        Ok(())
    }

    /// Returns the current physical dimensions, including zero while suspended.
    pub fn size(&self) -> [u32; 2] {
        let Self::Surface(target) = self;
        target.size
    }

    /// Returns the selected surface format.
    pub fn format(&self) -> wgpu::TextureFormat {
        let Self::Surface(target) = self;
        target.configuration.format
    }

    /// Returns the owned wgpu surface for inspection or low-level integration.
    ///
    /// Configure and acquire through Astrelis while using [`crate::Renderer`].
    /// External reconfiguration would invalidate the target's managed state.
    pub fn as_surface(&self) -> &wgpu::Surface<'window> {
        let Self::Surface(target) = self;
        &target.surface
    }
}

impl SurfaceTarget<'_> {
    pub(crate) fn configure(&self) {
        self.surface
            .configure(self.graphics.device(), &self.configuration);
    }
}

fn validate_size(graphics: &GraphicsContext, width: u32, height: u32) -> Result<(), Error> {
    let max = graphics.device().limits().max_texture_dimension_2d;
    if width > max || height > max {
        return Err(Error::InvalidTargetSize { width, height, max });
    }
    Ok(())
}
