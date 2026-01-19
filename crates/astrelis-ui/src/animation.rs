//! Animation system for UI widgets.
//!
//! Provides smooth transitions and effects for widget properties like position, size,
//! opacity, color, rotation, and scale.
//!
//! # Example
//!
//! ```ignore
//! use astrelis_ui::*;
//!
//! // Create an animation system
//! let mut anim_system = AnimationSystem::new();
//!
//! // Animate opacity
//! anim_system.animate(
//!     widget_id,
//!     Animation::new(AnimatableProperty::Opacity)
//!         .from(0.0)
//!         .to(1.0)
//!         .duration(0.3)
//!         .easing(EasingFunction::EaseInOut)
//! );
//!
//! // Update animations
//! anim_system.update(delta_time);
//!
//! // Apply animations to widgets
//! anim_system.apply(&mut ui_tree);
//! ```

use crate::widget_id::WidgetId;
use ahash::HashMap;

/// Properties that can be animated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnimatableProperty {
    /// Opacity (0.0 to 1.0)
    Opacity,
    /// X position
    PositionX,
    /// Y position
    PositionY,
    /// Width
    Width,
    /// Height
    Height,
    /// Rotation in radians
    Rotation,
    /// X scale
    ScaleX,
    /// Y scale
    ScaleY,
    /// Red color channel (0.0 to 1.0)
    ColorR,
    /// Green color channel (0.0 to 1.0)
    ColorG,
    /// Blue color channel (0.0 to 1.0)
    ColorB,
    /// Alpha color channel (0.0 to 1.0)
    ColorA,
    /// Border radius
    BorderRadius,
    /// Padding
    Padding,
}

/// Easing functions for animations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EasingFunction {
    /// Linear interpolation
    Linear,
    /// Ease in (slow start)
    EaseIn,
    /// Ease out (slow end)
    EaseOut,
    /// Ease in and out (slow start and end)
    EaseInOut,
    /// Bounce effect
    Bounce,
    /// Elastic effect
    Elastic,
    /// Quadratic ease in
    QuadIn,
    /// Quadratic ease out
    QuadOut,
    /// Quadratic ease in-out
    QuadInOut,
    /// Cubic ease in
    CubicIn,
    /// Cubic ease out
    CubicOut,
    /// Cubic ease in-out
    CubicInOut,
}

impl EasingFunction {
    /// Apply the easing function to a normalized time value (0.0 to 1.0).
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);

        match self {
            EasingFunction::Linear => t,
            EasingFunction::EaseIn => t * t,
            EasingFunction::EaseOut => t * (2.0 - t),
            EasingFunction::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
            EasingFunction::Bounce => {
                if t < 1.0 / 2.75 {
                    7.5625 * t * t
                } else if t < 2.0 / 2.75 {
                    let t = t - 1.5 / 2.75;
                    7.5625 * t * t + 0.75
                } else if t < 2.5 / 2.75 {
                    let t = t - 2.25 / 2.75;
                    7.5625 * t * t + 0.9375
                } else {
                    let t = t - 2.625 / 2.75;
                    7.5625 * t * t + 0.984375
                }
            }
            EasingFunction::Elastic => {
                if t == 0.0 || t == 1.0 {
                    t
                } else {
                    let p = 0.3;
                    let s = p / 4.0;
                    let t = t - 1.0;
                    -(2.0f32.powf(10.0 * t) * ((t - s) * (2.0 * std::f32::consts::PI) / p).sin())
                }
            }
            EasingFunction::QuadIn => t * t,
            EasingFunction::QuadOut => t * (2.0 - t),
            EasingFunction::QuadInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
            EasingFunction::CubicIn => t * t * t,
            EasingFunction::CubicOut => {
                let t = t - 1.0;
                t * t * t + 1.0
            }
            EasingFunction::CubicInOut => {
                let t = t * 2.0;
                if t < 1.0 {
                    0.5 * t * t * t
                } else {
                    let t = t - 2.0;
                    0.5 * (t * t * t + 2.0)
                }
            }
        }
    }
}

/// Animation state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationState {
    /// Animation is running
    Running,
    /// Animation is paused
    Paused,
    /// Animation is completed
    Completed,
}

/// An animation for a single property.
#[derive(Debug, Clone)]
pub struct Animation {
    /// The property being animated
    property: AnimatableProperty,
    /// Start value
    from: f32,
    /// End value
    to: f32,
    /// Duration in seconds
    duration: f32,
    /// Elapsed time in seconds
    elapsed: f32,
    /// Easing function
    easing: EasingFunction,
    /// Animation state
    state: AnimationState,
    /// Loop flag
    looping: bool,
    /// Yoyo flag (reverse direction at end)
    yoyo: bool,
    /// Current direction (1.0 = forward, -1.0 = reverse)
    direction: f32,
    /// Delay before starting (seconds)
    delay: f32,
    /// Delay elapsed time
    delay_elapsed: f32,
}

impl Animation {
    /// Create a new animation.
    pub fn new(property: AnimatableProperty) -> Self {
        Self {
            property,
            from: 0.0,
            to: 1.0,
            duration: 1.0,
            elapsed: 0.0,
            easing: EasingFunction::Linear,
            state: AnimationState::Running,
            looping: false,
            yoyo: false,
            direction: 1.0,
            delay: 0.0,
            delay_elapsed: 0.0,
        }
    }

    /// Set the start value.
    pub fn from(mut self, value: f32) -> Self {
        self.from = value;
        self
    }

    /// Set the end value.
    pub fn to(mut self, value: f32) -> Self {
        self.to = value;
        self
    }

    /// Set the duration in seconds.
    pub fn duration(mut self, duration: f32) -> Self {
        self.duration = duration;
        self
    }

    /// Set the easing function.
    pub fn easing(mut self, easing: EasingFunction) -> Self {
        self.easing = easing;
        self
    }

    /// Set looping.
    pub fn looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    /// Set yoyo (reverse at end).
    pub fn yoyo(mut self, yoyo: bool) -> Self {
        self.yoyo = yoyo;
        self
    }

    /// Set delay before starting (seconds).
    pub fn delay(mut self, delay: f32) -> Self {
        self.delay = delay;
        self
    }

    /// Get the property being animated.
    pub fn property(&self) -> AnimatableProperty {
        self.property
    }

    /// Get the current value.
    pub fn value(&self) -> f32 {
        // If we haven't started due to delay, return from value
        if self.delay_elapsed < self.delay {
            return self.from;
        }

        let t = (self.elapsed / self.duration).clamp(0.0, 1.0);
        let eased_t = self.easing.apply(t);

        // For both forward and reverse, interpolate based on elapsed time
        self.from + (self.to - self.from) * eased_t
    }

    /// Get the current state.
    pub fn state(&self) -> AnimationState {
        self.state
    }

    /// Pause the animation.
    pub fn pause(&mut self) {
        self.state = AnimationState::Paused;
    }

    /// Resume the animation.
    pub fn resume(&mut self) {
        if self.state == AnimationState::Paused {
            self.state = AnimationState::Running;
        }
    }

    /// Reset the animation.
    pub fn reset(&mut self) {
        self.elapsed = 0.0;
        self.delay_elapsed = 0.0;
        self.state = AnimationState::Running;
        self.direction = 1.0;
    }

    /// Update the animation by delta time.
    ///
    /// Returns true if the animation is still running.
    pub fn update(&mut self, delta_time: f32) -> bool {
        if self.state != AnimationState::Running {
            return self.state != AnimationState::Completed;
        }

        // Handle delay
        if self.delay_elapsed < self.delay {
            self.delay_elapsed += delta_time;
            if self.delay_elapsed < self.delay {
                return true;
            }
            // Continue with animation after delay
            let _ = delta_time - (self.delay - self.delay_elapsed);
        }

        self.elapsed += delta_time * self.direction;

        if self.direction > 0.0 && self.elapsed >= self.duration {
            if self.yoyo {
                self.direction = -1.0;
                self.elapsed = self.duration;
            } else if self.looping {
                self.elapsed = 0.0;
            } else {
                self.elapsed = self.duration;
                self.state = AnimationState::Completed;
                return false;
            }
        } else if self.direction < 0.0 && self.elapsed <= 0.0 {
            if self.looping {
                self.direction = 1.0;
                self.elapsed = 0.0;
            } else {
                self.elapsed = 0.0;
                self.state = AnimationState::Completed;
                return false;
            }
        }

        true
    }
}

/// A collection of animations for a single widget.
#[derive(Debug, Clone)]
pub struct WidgetAnimations {
    /// Animations for different properties
    animations: HashMap<AnimatableProperty, Animation>,
}

impl WidgetAnimations {
    /// Create a new widget animations collection.
    pub fn new() -> Self {
        Self {
            animations: HashMap::default(),
        }
    }

    /// Add an animation.
    pub fn add(&mut self, animation: Animation) {
        self.animations.insert(animation.property(), animation);
    }

    /// Remove an animation.
    pub fn remove(&mut self, property: AnimatableProperty) {
        self.animations.remove(&property);
    }

    /// Get an animation.
    pub fn get(&self, property: AnimatableProperty) -> Option<&Animation> {
        self.animations.get(&property)
    }

    /// Get a mutable animation.
    pub fn get_mut(&mut self, property: AnimatableProperty) -> Option<&mut Animation> {
        self.animations.get_mut(&property)
    }

    /// Update all animations.
    ///
    /// Returns true if any animations are still running.
    pub fn update(&mut self, delta_time: f32) -> bool {
        let mut any_running = false;

        self.animations.retain(|_, animation| {
            let running = animation.update(delta_time);
            any_running |= running;
            running
        });

        any_running
    }

    /// Get all current animation values.
    pub fn values(&self) -> HashMap<AnimatableProperty, f32> {
        self.animations
            .iter()
            .map(|(prop, anim)| (*prop, anim.value()))
            .collect()
    }

    /// Check if there are any animations.
    pub fn is_empty(&self) -> bool {
        self.animations.is_empty()
    }

    /// Clear all animations.
    pub fn clear(&mut self) {
        self.animations.clear();
    }
}

impl Default for WidgetAnimations {
    fn default() -> Self {
        Self::new()
    }
}

/// Animation system for managing widget animations.
pub struct AnimationSystem {
    /// Animations per widget
    widget_animations: HashMap<WidgetId, WidgetAnimations>,
}

impl AnimationSystem {
    /// Create a new animation system.
    pub fn new() -> Self {
        Self {
            widget_animations: HashMap::default(),
        }
    }

    /// Add an animation for a widget.
    pub fn animate(&mut self, widget_id: WidgetId, animation: Animation) {
        self.widget_animations
            .entry(widget_id)
            .or_insert_with(WidgetAnimations::new)
            .add(animation);
    }

    /// Remove an animation for a widget.
    pub fn remove_animation(&mut self, widget_id: WidgetId, property: AnimatableProperty) {
        if let Some(animations) = self.widget_animations.get_mut(&widget_id) {
            animations.remove(property);
            if animations.is_empty() {
                self.widget_animations.remove(&widget_id);
            }
        }
    }

    /// Remove all animations for a widget.
    pub fn remove_widget_animations(&mut self, widget_id: WidgetId) {
        self.widget_animations.remove(&widget_id);
    }

    /// Get animations for a widget.
    pub fn get_animations(&self, widget_id: WidgetId) -> Option<&WidgetAnimations> {
        self.widget_animations.get(&widget_id)
    }

    /// Get mutable animations for a widget.
    pub fn get_animations_mut(&mut self, widget_id: WidgetId) -> Option<&mut WidgetAnimations> {
        self.widget_animations.get_mut(&widget_id)
    }

    /// Update all animations by delta time.
    pub fn update(&mut self, delta_time: f32) {
        self.widget_animations.retain(|_, animations| {
            animations.update(delta_time);
            !animations.is_empty()
        });
    }

    /// Get all animated widget values.
    ///
    /// Returns a map of widget IDs to their animated property values.
    pub fn animated_values(&self) -> HashMap<WidgetId, HashMap<AnimatableProperty, f32>> {
        self.widget_animations
            .iter()
            .map(|(id, animations)| (*id, animations.values()))
            .collect()
    }

    /// Clear all animations.
    pub fn clear(&mut self) {
        self.widget_animations.clear();
    }

    /// Get the number of animated widgets.
    pub fn widget_count(&self) -> usize {
        self.widget_animations.len()
    }
}

impl Default for AnimationSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to create a fade-in animation.
pub fn fade_in(duration: f32) -> Animation {
    Animation::new(AnimatableProperty::Opacity)
        .from(0.0)
        .to(1.0)
        .duration(duration)
        .easing(EasingFunction::EaseInOut)
}

/// Helper function to create a fade-out animation.
pub fn fade_out(duration: f32) -> Animation {
    Animation::new(AnimatableProperty::Opacity)
        .from(1.0)
        .to(0.0)
        .duration(duration)
        .easing(EasingFunction::EaseInOut)
}

/// Helper function to create a slide-in animation from the left.
pub fn slide_in_left(from_x: f32, to_x: f32, duration: f32) -> Animation {
    Animation::new(AnimatableProperty::PositionX)
        .from(from_x)
        .to(to_x)
        .duration(duration)
        .easing(EasingFunction::EaseOut)
}

/// Helper function to create a slide-in animation from the top.
pub fn slide_in_top(from_y: f32, to_y: f32, duration: f32) -> Animation {
    Animation::new(AnimatableProperty::PositionY)
        .from(from_y)
        .to(to_y)
        .duration(duration)
        .easing(EasingFunction::EaseOut)
}

/// Helper function to create a scale animation.
pub fn scale(from: f32, to: f32, duration: f32) -> Animation {
    Animation::new(AnimatableProperty::ScaleX)
        .from(from)
        .to(to)
        .duration(duration)
        .easing(EasingFunction::EaseInOut)
}

/// Helper function to create a bounce animation.
pub fn bounce(duration: f32) -> Animation {
    Animation::new(AnimatableProperty::ScaleY)
        .from(1.0)
        .to(1.2)
        .duration(duration)
        .easing(EasingFunction::Bounce)
        .yoyo(true)
        .looping(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_easing() {
        let easing = EasingFunction::Linear;
        assert_eq!(easing.apply(0.0), 0.0);
        assert_eq!(easing.apply(0.5), 0.5);
        assert_eq!(easing.apply(1.0), 1.0);
    }

    #[test]
    fn test_animation_update() {
        let mut anim = Animation::new(AnimatableProperty::Opacity)
            .from(0.0)
            .to(1.0)
            .duration(1.0);

        // At start
        assert_eq!(anim.value(), 0.0);

        // Halfway
        assert!(anim.update(0.5));
        assert!((anim.value() - 0.5).abs() < 0.01);

        // Complete - should return false when done
        assert!(!anim.update(0.5));
        assert_eq!(anim.value(), 1.0);
        assert_eq!(anim.state(), AnimationState::Completed);
    }

    #[test]
    fn test_animation_system() {
        let mut system = AnimationSystem::new();
        let widget_id = WidgetId::new("test_widget");

        system.animate(widget_id, fade_in(1.0));

        assert_eq!(system.widget_count(), 1);

        system.update(1.0);

        // Animation should be complete and removed
        assert_eq!(system.widget_count(), 0);
    }

    #[test]
    fn test_looping_animation() {
        let mut anim = Animation::new(AnimatableProperty::Opacity)
            .from(0.0)
            .to(1.0)
            .duration(1.0)
            .looping(true);

        // First loop
        anim.update(1.0);
        assert_eq!(anim.state(), AnimationState::Running);
        assert_eq!(anim.value(), 0.0); // Should loop back

        // Second loop
        anim.update(0.5);
        assert!((anim.value() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_yoyo_animation() {
        let mut anim = Animation::new(AnimatableProperty::Opacity)
            .from(0.0)
            .to(1.0)
            .duration(1.0)
            .yoyo(true);

        // Forward - should still be running after first direction
        assert!(anim.update(1.0));
        assert!((anim.value() - 1.0).abs() < 0.01);
        assert_eq!(anim.state(), AnimationState::Running);

        // Reverse - should complete and return false
        assert!(!anim.update(1.0));
        assert!((anim.value() - 0.0).abs() < 0.01);
        assert_eq!(anim.state(), AnimationState::Completed);
    }
}
