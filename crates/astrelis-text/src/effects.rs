//! Text effects including shadows, outlines, and glows.
//!
//! Provides visual effects for text rendering to enhance readability and aesthetics.
//!
//! # Example
//!
//! ```ignore
//! use astrelis_text::*;
//!
//! let text = Text::new("Hello")
//!     .size(24.0)
//!     .with_effect(TextEffect::shadow(
//!         Vec2::new(2.0, 2.0),
//!         Color::BLACK,
//!     ))
//!     .with_effect(TextEffect::outline(
//!         1.0,
//!         Color::WHITE,
//!     ));
//! ```

use astrelis_core::math::Vec2;
use astrelis_render::Color;

/// A visual effect applied to text.
#[derive(Debug, Clone)]
pub struct TextEffect {
    /// The type of effect
    pub effect_type: TextEffectType,
    /// Whether the effect is enabled
    pub enabled: bool,
}

/// Types of text effects.
#[derive(Debug, Clone, PartialEq)]
pub enum TextEffectType {
    /// Drop shadow effect
    Shadow {
        /// Offset from the text
        offset: Vec2,
        /// Blur radius (0 = hard edge)
        blur_radius: f32,
        /// Shadow color
        color: Color,
    },
    /// Outline effect
    Outline {
        /// Width of the outline in pixels
        width: f32,
        /// Outline color
        color: Color,
    },
    /// Glow effect
    Glow {
        /// Radius of the glow
        radius: f32,
        /// Glow color
        color: Color,
        /// Intensity multiplier (0.0 to 1.0+)
        intensity: f32,
    },
    /// Inner shadow effect
    InnerShadow {
        /// Offset from the text
        offset: Vec2,
        /// Blur radius
        blur_radius: f32,
        /// Shadow color
        color: Color,
    },
}

impl TextEffect {
    /// Create a new text effect.
    pub fn new(effect_type: TextEffectType) -> Self {
        Self {
            effect_type,
            enabled: true,
        }
    }

    /// Create a drop shadow effect.
    ///
    /// # Arguments
    ///
    /// * `offset` - Offset from the text (positive Y is down)
    /// * `color` - Shadow color
    pub fn shadow(offset: Vec2, color: Color) -> Self {
        Self::new(TextEffectType::Shadow {
            offset,
            blur_radius: 0.0,
            color,
        })
    }

    /// Create a drop shadow effect with blur.
    pub fn shadow_blurred(offset: Vec2, blur_radius: f32, color: Color) -> Self {
        Self::new(TextEffectType::Shadow {
            offset,
            blur_radius,
            color,
        })
    }

    /// Create an outline effect.
    ///
    /// # Arguments
    ///
    /// * `width` - Width of the outline in pixels
    /// * `color` - Outline color
    pub fn outline(width: f32, color: Color) -> Self {
        Self::new(TextEffectType::Outline { width, color })
    }

    /// Create a glow effect.
    ///
    /// # Arguments
    ///
    /// * `radius` - Radius of the glow
    /// * `color` - Glow color
    /// * `intensity` - Intensity multiplier (typically 0.5 to 1.0)
    pub fn glow(radius: f32, color: Color, intensity: f32) -> Self {
        Self::new(TextEffectType::Glow {
            radius,
            color,
            intensity,
        })
    }

    /// Create an inner shadow effect.
    pub fn inner_shadow(offset: Vec2, blur_radius: f32, color: Color) -> Self {
        Self::new(TextEffectType::InnerShadow {
            offset,
            blur_radius,
            color,
        })
    }

    /// Enable or disable the effect.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if the effect is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the effect type.
    pub fn effect_type(&self) -> &TextEffectType {
        &self.effect_type
    }

    /// Update the effect type.
    pub fn set_effect_type(&mut self, effect_type: TextEffectType) {
        self.effect_type = effect_type;
    }

    /// Check if this effect requires multi-pass rendering.
    pub fn requires_multi_pass(&self) -> bool {
        matches!(
            self.effect_type,
            TextEffectType::Shadow { blur_radius, .. } if blur_radius > 0.0
        ) || matches!(self.effect_type, TextEffectType::Glow { .. })
    }

    /// Get the rendering order priority.
    ///
    /// Lower values render first (background effects).
    /// Higher values render last (foreground effects).
    pub fn render_priority(&self) -> i32 {
        match self.effect_type {
            TextEffectType::Shadow { .. } => 0,
            TextEffectType::InnerShadow { .. } => 1,
            TextEffectType::Glow { .. } => 2,
            TextEffectType::Outline { .. } => 3,
        }
    }
}

/// Effect rendering configuration.
#[derive(Debug, Clone)]
pub struct EffectRenderConfig {
    /// Maximum blur radius for performance
    pub max_blur_radius: f32,
    /// Maximum glow radius for performance
    pub max_glow_radius: f32,
    /// Number of blur samples (higher = better quality, slower)
    pub blur_samples: u32,
}

impl Default for EffectRenderConfig {
    fn default() -> Self {
        Self {
            max_blur_radius: 10.0,
            max_glow_radius: 20.0,
            blur_samples: 9,
        }
    }
}

impl EffectRenderConfig {
    /// Create a low-quality configuration (faster).
    pub fn low() -> Self {
        Self {
            max_blur_radius: 5.0,
            max_glow_radius: 10.0,
            blur_samples: 5,
        }
    }

    /// Create a medium-quality configuration (balanced).
    pub fn medium() -> Self {
        Self::default()
    }

    /// Create a high-quality configuration (slower).
    pub fn high() -> Self {
        Self {
            max_blur_radius: 20.0,
            max_glow_radius: 40.0,
            blur_samples: 13,
        }
    }
}

/// Collection of text effects applied to text.
#[derive(Debug, Clone, Default)]
pub struct TextEffects {
    /// List of effects to apply
    effects: Vec<TextEffect>,
}

impl TextEffects {
    /// Create a new empty effects collection.
    pub fn new() -> Self {
        Self {
            effects: Vec::new(),
        }
    }

    /// Add an effect.
    pub fn add(&mut self, effect: TextEffect) {
        self.effects.push(effect);
    }

    /// Remove all effects.
    pub fn clear(&mut self) {
        self.effects.clear();
    }

    /// Get all effects.
    pub fn effects(&self) -> &[TextEffect] {
        &self.effects
    }

    /// Get mutable effects.
    pub fn effects_mut(&mut self) -> &mut Vec<TextEffect> {
        &mut self.effects
    }

    /// Check if any effects are enabled.
    pub fn has_enabled_effects(&self) -> bool {
        self.effects.iter().any(|e| e.enabled)
    }

    /// Get effects sorted by render priority.
    pub fn sorted_by_priority(&self) -> Vec<&TextEffect> {
        let mut sorted: Vec<_> = self.effects.iter().filter(|e| e.enabled).collect();
        sorted.sort_by_key(|e| e.render_priority());
        sorted
    }

    /// Calculate the expanded bounds needed for effects.
    ///
    /// Returns (left, top, right, bottom) expansion in pixels.
    pub fn calculate_bounds_expansion(&self) -> (f32, f32, f32, f32) {
        let mut left = 0.0f32;
        let mut top = 0.0f32;
        let mut right = 0.0f32;
        let mut bottom = 0.0f32;

        for effect in &self.effects {
            if !effect.enabled {
                continue;
            }

            match &effect.effect_type {
                TextEffectType::Shadow {
                    offset,
                    blur_radius,
                    ..
                } => {
                    let expansion = *blur_radius * 2.0;
                    left = left.max(-offset.x + expansion);
                    top = top.max(-offset.y + expansion);
                    right = right.max(offset.x + expansion);
                    bottom = bottom.max(offset.y + expansion);
                }
                TextEffectType::Outline { width, .. } => {
                    let expansion = *width;
                    left = left.max(expansion);
                    top = top.max(expansion);
                    right = right.max(expansion);
                    bottom = bottom.max(expansion);
                }
                TextEffectType::Glow { radius, .. } => {
                    left = left.max(*radius);
                    top = top.max(*radius);
                    right = right.max(*radius);
                    bottom = bottom.max(*radius);
                }
                TextEffectType::InnerShadow { .. } => {
                    // Inner shadows don't expand bounds
                }
            }
        }

        (left, top, right, bottom)
    }
}

/// Builder for creating text effects.
pub struct TextEffectsBuilder {
    effects: TextEffects,
}

impl TextEffectsBuilder {
    /// Create a new effects builder.
    pub fn new() -> Self {
        Self {
            effects: TextEffects::new(),
        }
    }

    /// Add a shadow effect.
    pub fn shadow(mut self, offset: Vec2, color: Color) -> Self {
        self.effects.add(TextEffect::shadow(offset, color));
        self
    }

    /// Add a blurred shadow effect.
    pub fn shadow_blurred(mut self, offset: Vec2, blur_radius: f32, color: Color) -> Self {
        self.effects
            .add(TextEffect::shadow_blurred(offset, blur_radius, color));
        self
    }

    /// Add an outline effect.
    pub fn outline(mut self, width: f32, color: Color) -> Self {
        self.effects.add(TextEffect::outline(width, color));
        self
    }

    /// Add a glow effect.
    pub fn glow(mut self, radius: f32, color: Color, intensity: f32) -> Self {
        self.effects.add(TextEffect::glow(radius, color, intensity));
        self
    }

    /// Add an inner shadow effect.
    pub fn inner_shadow(mut self, offset: Vec2, blur_radius: f32, color: Color) -> Self {
        self.effects
            .add(TextEffect::inner_shadow(offset, blur_radius, color));
        self
    }

    /// Add a custom effect.
    pub fn effect(mut self, effect: TextEffect) -> Self {
        self.effects.add(effect);
        self
    }

    /// Build the effects collection.
    pub fn build(self) -> TextEffects {
        self.effects
    }
}

impl Default for TextEffectsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shadow_effect() {
        let effect = TextEffect::shadow(Vec2::new(2.0, 2.0), Color::BLACK);
        assert!(effect.is_enabled());
        assert_eq!(effect.render_priority(), 0);
    }

    #[test]
    fn test_outline_effect() {
        let effect = TextEffect::outline(1.0, Color::WHITE);
        assert!(effect.is_enabled());
        assert_eq!(effect.render_priority(), 3);
    }

    #[test]
    fn test_glow_effect() {
        let effect = TextEffect::glow(5.0, Color::BLUE, 0.8);
        assert!(effect.is_enabled());
        assert!(effect.requires_multi_pass());
    }

    #[test]
    fn test_effects_builder() {
        let effects = TextEffectsBuilder::new()
            .shadow(Vec2::new(1.0, 1.0), Color::BLACK)
            .outline(1.0, Color::WHITE)
            .glow(3.0, Color::BLUE, 0.5)
            .build();

        assert_eq!(effects.effects().len(), 3);
        assert!(effects.has_enabled_effects());
    }

    #[test]
    fn test_effects_priority_sorting() {
        let mut effects = TextEffects::new();
        effects.add(TextEffect::outline(1.0, Color::WHITE)); // Priority 3
        effects.add(TextEffect::shadow(Vec2::ZERO, Color::BLACK)); // Priority 0
        effects.add(TextEffect::glow(5.0, Color::BLUE, 1.0)); // Priority 2

        let sorted = effects.sorted_by_priority();
        assert_eq!(sorted[0].render_priority(), 0);
        assert_eq!(sorted[1].render_priority(), 2);
        assert_eq!(sorted[2].render_priority(), 3);
    }

    #[test]
    fn test_bounds_expansion() {
        let effects = TextEffectsBuilder::new()
            .shadow(Vec2::new(2.0, 2.0), Color::BLACK)
            .outline(1.0, Color::WHITE)
            .build();

        let (left, top, right, bottom) = effects.calculate_bounds_expansion();
        assert!(left > 0.0);
        assert!(top > 0.0);
        assert!(right > 0.0);
        assert!(bottom > 0.0);
    }
}
