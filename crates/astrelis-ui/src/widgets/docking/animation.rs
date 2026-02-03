//! Docking-specific animation state for smooth visual transitions.
//!
//! This module provides animation types tailored to docking operations:
//! - Ghost tab following the cursor during cross-container drag
//! - Panel transitions when splits are created or collapsed
//! - Separator easing for smooth splitter drag
//! - Tab reorder slide animation
//! - Drop preview fade and bounds transitions

use crate::tree::LayoutRect;
use astrelis_core::math::Vec2;

/// Exponential lerp factor for smooth following animations.
const EXP_LERP_SPEED: f32 = 12.0;

/// Default ghost tab opacity.
const GHOST_TAB_OPACITY: f32 = 0.7;

/// Default panel transition duration in seconds.
const PANEL_TRANSITION_DURATION: f32 = 0.2;

/// Default tab reorder slide duration in seconds.
const TAB_REORDER_DURATION: f32 = 0.15;

/// Drop preview fade speed (opacity per second).
const DROP_PREVIEW_FADE_SPEED: f32 = 8.0;

/// Aggregate animation state for the docking system.
///
/// Holds all active docking animations. The event handler creates animations
/// at trigger points, and the renderer queries them for smooth visual output.
#[derive(Debug, Default)]
pub struct DockAnimationState {
    /// Ghost tab following cursor during cross-container drag.
    pub ghost_tab: Option<GhostTabAnimation>,
    /// Ghost tab group following cursor during group drag.
    pub ghost_group: Option<GhostGroupAnimation>,
    /// Panel transition when a split is created or collapsed.
    pub panel_transition: Option<PanelTransition>,
    /// Separator easing for smooth splitter drag.
    pub separator_ease: Option<SeparatorEase>,
    /// Tab reorder slide animation.
    pub tab_reorder: Option<TabReorderAnimation>,
    /// Drop preview fade and bounds transition.
    pub drop_preview: Option<DropPreviewAnimation>,
}

impl DockAnimationState {
    /// Create a new empty animation state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update all active animations with the given delta time.
    ///
    /// Returns `true` if any animation is still active (needs another frame).
    pub fn update(&mut self, dt: f32) -> bool {
        let mut any_active = false;

        if let Some(ref mut anim) = self.ghost_tab {
            anim.update(dt);
            if anim.is_done() {
                self.ghost_tab = None;
            } else {
                any_active = true;
            }
        }

        if let Some(ref mut anim) = self.ghost_group {
            anim.update(dt);
            if anim.is_done() {
                self.ghost_group = None;
            } else {
                any_active = true;
            }
        }

        if let Some(ref mut anim) = self.panel_transition {
            anim.update(dt);
            if anim.is_done() {
                self.panel_transition = None;
            } else {
                any_active = true;
            }
        }

        if let Some(ref mut anim) = self.separator_ease {
            anim.update(dt);
            if anim.is_done() {
                self.separator_ease = None;
            } else {
                any_active = true;
            }
        }

        if let Some(ref mut anim) = self.tab_reorder {
            anim.update(dt);
            if anim.is_done() {
                self.tab_reorder = None;
            } else {
                any_active = true;
            }
        }

        if let Some(ref mut anim) = self.drop_preview {
            anim.update(dt);
            if anim.is_done() {
                self.drop_preview = None;
            } else {
                any_active = true;
            }
        }

        any_active
    }

    /// Check if any animation is active.
    pub fn has_active_animations(&self) -> bool {
        self.ghost_tab.is_some()
            || self.ghost_group.is_some()
            || self.panel_transition.is_some()
            || self.separator_ease.is_some()
            || self.tab_reorder.is_some()
            || self.drop_preview.is_some()
    }

    /// Clear all animations.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

// ---------------------------------------------------------------------------
// Ghost Tab Animation
// ---------------------------------------------------------------------------

/// A floating ghost tab that follows the cursor during cross-container drag.
///
/// The ghost uses exponential lerp to smoothly follow the cursor position,
/// giving a fluid feel to drag operations.
#[derive(Debug, Clone)]
pub struct GhostTabAnimation {
    /// Current rendered position of the ghost tab.
    pub position: Vec2,
    /// Target position (cursor position).
    pub target: Vec2,
    /// Size of the ghost tab.
    pub size: Vec2,
    /// Label text for the ghost tab.
    pub label: String,
    /// Current opacity (0.0 = invisible, 1.0 = fully visible).
    pub opacity: f32,
    /// Whether the ghost is fading out (after drop or cancel).
    pub fading_out: bool,
}

impl GhostTabAnimation {
    /// Create a new ghost tab animation at the given position.
    pub fn new(position: Vec2, size: Vec2, label: String) -> Self {
        Self {
            position,
            target: position,
            size,
            label,
            opacity: 0.0,
            fading_out: false,
        }
    }

    /// Update the ghost tab animation.
    pub fn update(&mut self, dt: f32) {
        if self.fading_out {
            // Fade out quickly
            self.opacity = (self.opacity - DROP_PREVIEW_FADE_SPEED * dt).max(0.0);
        } else {
            // Fade in
            self.opacity = (self.opacity + DROP_PREVIEW_FADE_SPEED * dt).min(GHOST_TAB_OPACITY);
            // Exponential lerp toward target
            let factor = 1.0 - (-EXP_LERP_SPEED * dt).exp();
            self.position = self.position.lerp(self.target, factor);
        }
    }

    /// Set the target position (cursor position with offset).
    pub fn set_target(&mut self, target: Vec2) {
        self.target = target;
    }

    /// Start fading out the ghost.
    pub fn fade_out(&mut self) {
        self.fading_out = true;
    }

    /// Check if the animation is done (faded out completely).
    pub fn is_done(&self) -> bool {
        self.fading_out && self.opacity <= 0.0
    }
}

// ---------------------------------------------------------------------------
// Ghost Group Animation
// ---------------------------------------------------------------------------

/// A floating ghost representing an entire tab group during cross-container drag.
///
/// Similar to `GhostTabAnimation` but represents a full tab bar with multiple labels.
#[derive(Debug, Clone)]
pub struct GhostGroupAnimation {
    /// Current rendered position of the ghost group.
    pub position: Vec2,
    /// Target position (cursor position).
    pub target: Vec2,
    /// Size of the ghost group (full tab bar dimensions).
    pub size: Vec2,
    /// Labels of all tabs in the group.
    pub labels: Vec<String>,
    /// Current opacity (0.0 = invisible, 1.0 = fully visible).
    pub opacity: f32,
    /// Whether the ghost is fading out (after drop or cancel).
    pub fading_out: bool,
}

impl GhostGroupAnimation {
    /// Create a new ghost group animation at the given position.
    pub fn new(position: Vec2, size: Vec2, labels: Vec<String>) -> Self {
        Self {
            position,
            target: position,
            size,
            labels,
            opacity: 0.0,
            fading_out: false,
        }
    }

    /// Update the ghost group animation.
    pub fn update(&mut self, dt: f32) {
        if self.fading_out {
            self.opacity = (self.opacity - DROP_PREVIEW_FADE_SPEED * dt).max(0.0);
        } else {
            self.opacity = (self.opacity + DROP_PREVIEW_FADE_SPEED * dt).min(GHOST_TAB_OPACITY);
            let factor = 1.0 - (-EXP_LERP_SPEED * dt).exp();
            self.position = self.position.lerp(self.target, factor);
        }
    }

    /// Set the target position (cursor position with offset).
    pub fn set_target(&mut self, target: Vec2) {
        self.target = target;
    }

    /// Start fading out the ghost.
    pub fn fade_out(&mut self) {
        self.fading_out = true;
    }

    /// Check if the animation is done (faded out completely).
    pub fn is_done(&self) -> bool {
        self.fading_out && self.opacity <= 0.0
    }
}

// ---------------------------------------------------------------------------
// Panel Transition
// ---------------------------------------------------------------------------

/// Animates a split ratio from one value to another when panels are created or collapsed.
#[derive(Debug, Clone)]
pub struct PanelTransition {
    /// Starting split ratio.
    pub from_ratio: f32,
    /// Target split ratio.
    pub to_ratio: f32,
    /// Current animated ratio.
    pub current_ratio: f32,
    /// Elapsed time.
    elapsed: f32,
    /// Total duration.
    duration: f32,
}

impl PanelTransition {
    /// Create a new panel transition.
    pub fn new(from_ratio: f32, to_ratio: f32) -> Self {
        Self {
            from_ratio,
            to_ratio,
            current_ratio: from_ratio,
            elapsed: 0.0,
            duration: PANEL_TRANSITION_DURATION,
        }
    }

    /// Create a panel transition with a custom duration.
    pub fn with_duration(mut self, duration: f32) -> Self {
        self.duration = duration;
        self
    }

    /// Update the transition.
    pub fn update(&mut self, dt: f32) {
        self.elapsed += dt;
        let t = (self.elapsed / self.duration).min(1.0);
        // Ease-out cubic: 1 - (1-t)^3
        let eased = 1.0 - (1.0 - t).powi(3);
        self.current_ratio = self.from_ratio + (self.to_ratio - self.from_ratio) * eased;
    }

    /// Check if the transition is complete.
    pub fn is_done(&self) -> bool {
        self.elapsed >= self.duration
    }

    /// Get the progress (0.0 to 1.0).
    pub fn progress(&self) -> f32 {
        (self.elapsed / self.duration).min(1.0)
    }
}

// ---------------------------------------------------------------------------
// Separator Ease
// ---------------------------------------------------------------------------

/// Smoothly interpolates the current splitter ratio toward a target ratio.
///
/// During splitter drag, instead of snapping immediately, the separator
/// eases toward the target position.
#[derive(Debug, Clone)]
pub struct SeparatorEase {
    /// Current rendered ratio.
    pub current_ratio: f32,
    /// Target ratio set by drag input.
    pub target_ratio: f32,
    /// Whether the drag is still active.
    pub active: bool,
}

impl SeparatorEase {
    /// Create a new separator ease at the given ratio.
    pub fn new(ratio: f32) -> Self {
        Self {
            current_ratio: ratio,
            target_ratio: ratio,
            active: true,
        }
    }

    /// Set the target ratio (called on every drag update).
    pub fn set_target(&mut self, target: f32) {
        self.target_ratio = target;
    }

    /// Stop the easing (drag ended).
    pub fn stop(&mut self) {
        self.active = false;
    }

    /// Update the separator ease.
    pub fn update(&mut self, dt: f32) {
        let factor = 1.0 - (-EXP_LERP_SPEED * dt).exp();
        self.current_ratio += (self.target_ratio - self.current_ratio) * factor;
    }

    /// Check if the animation is done (close enough to target and inactive).
    pub fn is_done(&self) -> bool {
        !self.active && (self.current_ratio - self.target_ratio).abs() < 0.001
    }
}

// ---------------------------------------------------------------------------
// Tab Reorder Animation
// ---------------------------------------------------------------------------

/// Slides a tab from its old position to its new position after reordering.
#[derive(Debug, Clone)]
pub struct TabReorderAnimation {
    /// Tab index that was reordered.
    pub tab_index: usize,
    /// Starting X offset from the final position.
    pub from_offset_x: f32,
    /// Current X offset (converges to 0.0).
    pub current_offset_x: f32,
    /// Elapsed time.
    elapsed: f32,
    /// Total duration.
    duration: f32,
}

impl TabReorderAnimation {
    /// Create a new tab reorder animation.
    ///
    /// `from_offset_x` is the horizontal distance from the old position to the new position
    /// (negative = tab moved right, positive = tab moved left).
    pub fn new(tab_index: usize, from_offset_x: f32) -> Self {
        Self {
            tab_index,
            from_offset_x,
            current_offset_x: from_offset_x,
            elapsed: 0.0,
            duration: TAB_REORDER_DURATION,
        }
    }

    /// Update the reorder animation.
    pub fn update(&mut self, dt: f32) {
        self.elapsed += dt;
        let t = (self.elapsed / self.duration).min(1.0);
        // Ease-out quadratic
        let eased = 1.0 - (1.0 - t) * (1.0 - t);
        self.current_offset_x = self.from_offset_x * (1.0 - eased);
    }

    /// Check if the animation is complete.
    pub fn is_done(&self) -> bool {
        self.elapsed >= self.duration
    }
}

// ---------------------------------------------------------------------------
// Drop Preview Animation
// ---------------------------------------------------------------------------

/// Smooth opacity and bounds transition for the cross-container drop preview.
///
/// When the preview zone changes (e.g., user moves cursor from left edge to center),
/// the preview smoothly transitions to the new bounds and fades between states.
#[derive(Debug, Clone)]
pub struct DropPreviewAnimation {
    /// Current opacity (0.0 = invisible, 1.0 = fully visible).
    pub opacity: f32,
    /// Target opacity.
    pub target_opacity: f32,
    /// Current preview bounds.
    pub current_bounds: LayoutRect,
    /// Target preview bounds.
    pub target_bounds: LayoutRect,
}

impl DropPreviewAnimation {
    /// Create a new drop preview animation.
    pub fn new(bounds: LayoutRect) -> Self {
        Self {
            opacity: 0.0,
            target_opacity: 1.0,
            current_bounds: bounds,
            target_bounds: bounds,
        }
    }

    /// Set a new target (zone changed).
    pub fn set_target(&mut self, bounds: LayoutRect) {
        self.target_bounds = bounds;
        self.target_opacity = 1.0;
    }

    /// Start fading out (cursor left all drop targets).
    pub fn fade_out(&mut self) {
        self.target_opacity = 0.0;
    }

    /// Update the drop preview animation.
    pub fn update(&mut self, dt: f32) {
        // Smooth opacity
        let opacity_diff = self.target_opacity - self.opacity;
        self.opacity += opacity_diff * (1.0 - (-DROP_PREVIEW_FADE_SPEED * dt).exp());

        // Smooth bounds
        let factor = 1.0 - (-EXP_LERP_SPEED * dt).exp();
        self.current_bounds.x += (self.target_bounds.x - self.current_bounds.x) * factor;
        self.current_bounds.y += (self.target_bounds.y - self.current_bounds.y) * factor;
        self.current_bounds.width +=
            (self.target_bounds.width - self.current_bounds.width) * factor;
        self.current_bounds.height +=
            (self.target_bounds.height - self.current_bounds.height) * factor;
    }

    /// Check if the animation is done (faded out completely).
    pub fn is_done(&self) -> bool {
        self.target_opacity <= 0.0 && self.opacity < 0.01
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghost_tab_fades_in() {
        let mut anim = GhostTabAnimation::new(Vec2::ZERO, Vec2::new(100.0, 28.0), "Tab".into());
        assert_eq!(anim.opacity, 0.0);

        // Simulate several frames
        for _ in 0..10 {
            anim.update(1.0 / 60.0);
        }
        assert!(anim.opacity > 0.0);
        assert!(!anim.is_done());
    }

    #[test]
    fn ghost_tab_fades_out() {
        let mut anim = GhostTabAnimation::new(Vec2::ZERO, Vec2::new(100.0, 28.0), "Tab".into());
        anim.opacity = GHOST_TAB_OPACITY;
        anim.fade_out();

        for _ in 0..60 {
            anim.update(1.0 / 60.0);
        }
        assert!(anim.is_done());
    }

    #[test]
    fn ghost_tab_follows_target() {
        let mut anim = GhostTabAnimation::new(Vec2::ZERO, Vec2::new(100.0, 28.0), "Tab".into());
        anim.set_target(Vec2::new(200.0, 100.0));

        for _ in 0..60 {
            anim.update(1.0 / 60.0);
        }
        // Should be close to target after 1 second at 60fps
        assert!((anim.position.x - 200.0).abs() < 1.0);
        assert!((anim.position.y - 100.0).abs() < 1.0);
    }

    #[test]
    fn panel_transition_completes() {
        let mut anim = PanelTransition::new(0.0, 0.5);
        assert_eq!(anim.current_ratio, 0.0);

        // Run for full duration
        let steps = (PANEL_TRANSITION_DURATION * 60.0) as usize + 1;
        for _ in 0..steps {
            anim.update(1.0 / 60.0);
        }
        assert!(anim.is_done());
        assert!((anim.current_ratio - 0.5).abs() < 0.01);
    }

    #[test]
    fn separator_ease_converges() {
        let mut anim = SeparatorEase::new(0.3);
        anim.set_target(0.7);
        anim.stop();

        for _ in 0..120 {
            anim.update(1.0 / 60.0);
        }
        assert!(anim.is_done());
        assert!((anim.current_ratio - 0.7).abs() < 0.001);
    }

    #[test]
    fn tab_reorder_slides_to_zero() {
        let mut anim = TabReorderAnimation::new(0, -50.0);
        assert_eq!(anim.current_offset_x, -50.0);

        let steps = (TAB_REORDER_DURATION * 60.0) as usize + 1;
        for _ in 0..steps {
            anim.update(1.0 / 60.0);
        }
        assert!(anim.is_done());
        assert!(anim.current_offset_x.abs() < 0.1);
    }

    #[test]
    fn drop_preview_fades_in_and_out() {
        let bounds = LayoutRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };
        let mut anim = DropPreviewAnimation::new(bounds);
        assert_eq!(anim.opacity, 0.0);

        // Fade in
        for _ in 0..30 {
            anim.update(1.0 / 60.0);
        }
        assert!(anim.opacity > 0.5);

        // Start fade out
        anim.fade_out();
        for _ in 0..60 {
            anim.update(1.0 / 60.0);
        }
        assert!(anim.is_done());
    }

    #[test]
    fn drop_preview_transitions_bounds() {
        let bounds = LayoutRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };
        let mut anim = DropPreviewAnimation::new(bounds);

        let new_bounds = LayoutRect {
            x: 50.0,
            y: 50.0,
            width: 200.0,
            height: 200.0,
        };
        anim.set_target(new_bounds);

        for _ in 0..120 {
            anim.update(1.0 / 60.0);
        }
        assert!((anim.current_bounds.x - 50.0).abs() < 1.0);
        assert!((anim.current_bounds.width - 200.0).abs() < 1.0);
    }

    #[test]
    fn dock_animation_state_updates_all() {
        let mut state = DockAnimationState::new();

        state.ghost_tab = Some(GhostTabAnimation::new(
            Vec2::ZERO,
            Vec2::new(100.0, 28.0),
            "Tab".into(),
        ));
        state.panel_transition = Some(PanelTransition::new(0.0, 0.5));

        assert!(state.has_active_animations());

        // Update for a while
        for _ in 0..120 {
            state.update(1.0 / 60.0);
        }

        // Panel transition should be done and cleared
        assert!(state.panel_transition.is_none());
        // Ghost tab is still animating (not fading out)
        assert!(state.ghost_tab.is_some());
    }
}
