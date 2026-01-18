//! Blend mode presets for common rendering scenarios.

/// Predefined blend modes for common use cases.
///
/// Use these to configure how source and destination colors are combined
/// during rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Default)]
pub enum BlendMode {
    /// No blending - source completely replaces destination.
    Replace,

    /// Standard alpha blending for transparent content.
    ///
    /// Formula: `src.rgb * src.a + dst.rgb * (1 - src.a)`
    ///
    /// Use for: Transparent UI over game content, sprites with transparency.
    #[default]
    Alpha,

    /// Premultiplied alpha blending.
    ///
    /// Formula: `src.rgb + dst.rgb * (1 - src.a)`
    ///
    /// Use for: Blitting framebuffers with premultiplied alpha, compositing.
    PremultipliedAlpha,

    /// Additive blending - colors are added together.
    ///
    /// Formula: `src.rgb + dst.rgb`
    ///
    /// Use for: Glow effects, particles, light sources.
    Additive,

    /// Multiplicative blending.
    ///
    /// Formula: `src.rgb * dst.rgb`
    ///
    /// Use for: Shadows, color tinting.
    Multiply,

    /// Custom blend state for advanced use cases.
    Custom(wgpu::BlendState),
}

impl BlendMode {
    /// Convert to wgpu BlendState.
    pub fn to_blend_state(self) -> Option<wgpu::BlendState> {
        match self {
            BlendMode::Replace => Some(wgpu::BlendState::REPLACE),
            BlendMode::Alpha => Some(wgpu::BlendState::ALPHA_BLENDING),
            BlendMode::PremultipliedAlpha => Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            BlendMode::Additive => Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            BlendMode::Multiply => Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Dst,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::DstAlpha,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            BlendMode::Custom(state) => Some(state),
        }
    }

    /// Create a color target state with this blend mode.
    pub fn to_color_target_state(self, format: wgpu::TextureFormat) -> wgpu::ColorTargetState {
        wgpu::ColorTargetState {
            format,
            blend: self.to_blend_state(),
            write_mask: wgpu::ColorWrites::ALL,
        }
    }
}


impl From<BlendMode> for Option<wgpu::BlendState> {
    fn from(mode: BlendMode) -> Self {
        mode.to_blend_state()
    }
}

impl From<wgpu::BlendState> for BlendMode {
    fn from(state: wgpu::BlendState) -> Self {
        BlendMode::Custom(state)
    }
}
