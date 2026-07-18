use std::any::Any;

use astrelis_core::geometry::{LogicalPoint, LogicalRect, LogicalSize, Physical, Rect, Size};
use astrelis_paint::{
    Brush, CompositorViewId, CornerRadii, ExternalImage, ImageOptions, ImageSampling, Painter,
    RoundedRect,
};
use astrelis_platform::{DeviceId, ElementState, ImeEvent, KeyboardInput, PointerButton};
use astrelis_ui_core::{
    ElementHandle, EventContext, RoutedEvent, RoutedEventKind, SemanticRole, Theme, Ui, UiError,
    Widget,
};

/// Content currently presented by a [`RenderView`].
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub enum RenderViewContent {
    /// No renderable allocation is currently available.
    Unavailable,
    /// A registered image is ready, with a rendered subextent at its origin.
    Ready {
        image: ExternalImage,
        source_extent: Size<Physical, u32>,
    },
    /// A compositor-managed scene, with direct rendering explicitly preferred.
    Composited {
        id: CompositorViewId,
        /// Whether exact rectangular views should use direct frame composition.
        prefer_direct: bool,
    },
    /// Scene rendering failed.
    Error(String),
}

/// A pointer position expressed in local and normalized view coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderViewPointerPosition {
    /// Position relative to the view's top-left corner.
    pub local: LogicalPoint,
    /// Local position divided by the view size; captured drags may be unbounded.
    pub normalized: LogicalPoint,
    /// Whether the position lies inside the rounded view shape.
    pub inside: bool,
}

/// Typed input delivered by a [`RenderView`].
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub enum RenderViewEvent {
    /// Pointer motion.
    PointerMoved {
        device_id: DeviceId,
        position: RenderViewPointerPosition,
    },
    /// Pointer button transition.
    PointerButton {
        device_id: DeviceId,
        position: RenderViewPointerPosition,
        button: PointerButton,
        state: ElementState,
    },
    /// Captured pointer cancellation.
    PointerCancelled { device_id: DeviceId },
    /// Wheel or trackpad displacement.
    Scroll {
        device_id: DeviceId,
        delta: LogicalPoint,
    },
    /// Keyboard input while focused.
    Keyboard(KeyboardInput),
    /// Input-method event while focused.
    Ime(ImeEvent),
    /// Keyboard focus transition.
    FocusChanged(bool),
}

/// Allocation hysteresis used by application-owned render textures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderViewResizePolicy {
    /// Allocation dimension quantum in physical pixels.
    pub quantum: u32,
    /// Utilization below which an allocation shrinks.
    pub shrink_threshold: f32,
}

impl Default for RenderViewResizePolicy {
    fn default() -> Self {
        Self {
            quantum: 64,
            shrink_threshold: 0.75,
        }
    }
}

impl RenderViewResizePolicy {
    /// Validates policy values.
    pub fn validate(self) -> Result<Self, UiError> {
        if self.quantum == 0
            || !self.shrink_threshold.is_finite()
            || !(0.0..1.0).contains(&self.shrink_threshold)
        {
            return Err(UiError::from_message(
                "render-view resize policy requires a non-zero quantum and a shrink threshold within 0..1",
            ));
        }
        Ok(self)
    }

    /// Chooses an allocation, returning `None` for a hidden or empty desired extent.
    pub fn allocation(
        self,
        current: Option<Size<Physical, u32>>,
        desired: Size<Physical, u32>,
        visible: bool,
    ) -> Option<Size<Physical, u32>> {
        if !visible || desired.width == 0 || desired.height == 0 {
            return current;
        }
        let bucket = |value: u32| value.div_ceil(self.quantum) * self.quantum;
        let next = Size::new(bucket(desired.width), bucket(desired.height));
        match current {
            None => Some(next),
            Some(old) if desired.width > old.width || desired.height > old.height => Some(next),
            Some(old)
                if (desired.width as f32) < old.width as f32 * self.shrink_threshold
                    || (desired.height as f32) < old.height as f32 * self.shrink_threshold =>
            {
                Some(next)
            }
            Some(old) => Some(old),
        }
    }
}

/// Geometry and scheduling state for one retained render view.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderViewSnapshot {
    /// Untransformed layout bounds.
    pub layout_bounds: LogicalRect,
    /// Transformed logical bounds.
    pub world_bounds: LogicalRect,
    /// Transformed physical bounds.
    pub physical_bounds: Rect<Physical>,
    /// Effective physical clip.
    pub physical_clip: Option<Rect<Physical>>,
    /// Effective visibility.
    pub visible: bool,
    /// Keyboard focus state.
    pub focused: bool,
    /// Outward-rounded desired content dimensions.
    pub desired_physical_size: Size<Physical, u32>,
    /// Whether application scene rendering should be recorded.
    pub should_render: bool,
}

/// Builds a scheduling snapshot for a retained render view.
pub fn render_view_snapshot<Message: 'static>(
    ui: &mut Ui<Message>,
    handle: ElementHandle<RenderView<Message>>,
) -> Result<RenderViewSnapshot, UiError> {
    let node = ui.inspect_element(handle)?;
    let width = node.physical_bounds.size.width.max(0.0).ceil() as u32;
    let height = node.physical_bounds.size.height.max(0.0).ceil() as u32;
    let desired = Size::new(width, height);
    Ok(RenderViewSnapshot {
        layout_bounds: node.layout_bounds,
        world_bounds: node.world_bounds,
        physical_bounds: node.physical_bounds,
        physical_clip: node.physical_clip,
        visible: node.effectively_visible,
        focused: node.focused,
        desired_physical_size: desired,
        should_render: node.effectively_visible && width > 0 && height > 0,
    })
}

/// Retained widget which composites an application-owned GPU texture.
pub struct RenderView<Message> {
    content: RenderViewContent,
    intrinsic_size: LogicalSize,
    corner_radius: f32,
    sampling: ImageSampling,
    label: String,
    focusable: bool,
    on_input: Box<dyn FnMut(RenderViewEvent) -> Message>,
}

impl<Message> RenderView<Message> {
    /// Creates an unavailable render view with a typed input callback.
    pub fn new(
        label: impl Into<String>,
        on_input: impl FnMut(RenderViewEvent) -> Message + 'static,
    ) -> Self {
        Self {
            content: RenderViewContent::Unavailable,
            intrinsic_size: Size::new(320.0, 180.0),
            corner_radius: 8.0,
            sampling: ImageSampling::Linear,
            label: label.into(),
            focusable: true,
            on_input: Box::new(on_input),
        }
    }
    /// Replaces displayed content.
    pub fn set_content(&mut self, content: RenderViewContent) {
        self.content = content;
    }
    /// Configures intrinsic logical dimensions.
    pub fn set_intrinsic_size(&mut self, size: LogicalSize) {
        self.intrinsic_size = size;
    }
    /// Configures rounded clipping radius.
    pub fn set_corner_radius(&mut self, radius: f32) {
        self.corner_radius = radius.max(0.0);
    }
    /// Configures texture filtering.
    pub fn set_sampling(&mut self, sampling: ImageSampling) {
        self.sampling = sampling;
    }
    /// Configures keyboard focus participation.
    pub fn set_focusable(&mut self, focusable: bool) {
        self.focusable = focusable;
    }

    fn position(
        &self,
        context: &EventContext<'_, Message>,
        window: LogicalPoint,
    ) -> Option<RenderViewPointerPosition> {
        let local = context.window_to_local(window)?;
        let bounds = context.bounds();
        let normalized =
            LogicalPoint::new(local.x / bounds.size.width, local.y / bounds.size.height);
        let inside = rounded_contains(local, bounds.size, self.corner_radius);
        Some(RenderViewPointerPosition {
            local,
            normalized,
            inside,
        })
    }
}

impl<Message: 'static> Widget<Message> for RenderView<Message> {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn intrinsic_size(&self, _theme: &Theme) -> LogicalSize {
        self.intrinsic_size
    }
    fn hit_testable(&self) -> bool {
        true
    }
    fn focusable(&self) -> bool {
        self.focusable
    }
    fn hit_test(&self, point: LogicalPoint, bounds: LogicalRect) -> bool {
        rounded_contains(
            LogicalPoint::new(point.x - bounds.origin.x, point.y - bounds.origin.y),
            bounds.size,
            self.corner_radius,
        )
    }
    fn event(&mut self, context: &mut EventContext<'_, Message>, event: &RoutedEvent) {
        let mapped = match &event.kind {
            RoutedEventKind::PointerMoved {
                device_id,
                position,
            } => self
                .position(context, *position)
                .map(|position| RenderViewEvent::PointerMoved {
                    device_id: *device_id,
                    position,
                }),
            RoutedEventKind::PointerButton {
                device_id,
                position,
                button,
                state,
            } => {
                if *button == PointerButton::Primary && *state == ElementState::Pressed {
                    context.request_focus();
                    context.capture_pointer(*device_id);
                }
                if *button == PointerButton::Primary && *state == ElementState::Released {
                    context.release_pointer(*device_id);
                }
                self.position(context, *position)
                    .map(|position| RenderViewEvent::PointerButton {
                        device_id: *device_id,
                        position,
                        button: *button,
                        state: *state,
                    })
            }
            RoutedEventKind::PointerCancelled { device_id } => {
                context.release_pointer(*device_id);
                Some(RenderViewEvent::PointerCancelled {
                    device_id: *device_id,
                })
            }
            RoutedEventKind::Scroll { device_id, delta } => Some(RenderViewEvent::Scroll {
                device_id: *device_id,
                delta: *delta,
            }),
            RoutedEventKind::Keyboard(value) => Some(RenderViewEvent::Keyboard(value.clone())),
            RoutedEventKind::Ime(value) => Some(RenderViewEvent::Ime(value.clone())),
            RoutedEventKind::FocusChanged(value) => Some(RenderViewEvent::FocusChanged(*value)),
            _ => None,
        };
        if let Some(event) = mapped {
            context.emit((self.on_input)(event));
        }
    }
    fn paint(
        &self,
        painter: &mut Painter,
        bounds: LogicalRect,
        theme: &Theme,
    ) -> Result<(), UiError> {
        let rounded = RoundedRect::new(bounds, CornerRadii::uniform(self.corner_radius))
            .map_err(|e| UiError::from_message(e.to_string()))?;
        match &self.content {
            RenderViewContent::Unavailable => {
                painter.fill_rounded_rect(rounded, Brush::Solid(theme.surface))
            }
            RenderViewContent::Error(_) => {
                painter.fill_rounded_rect(rounded, Brush::Solid(theme.danger))
            }
            RenderViewContent::Ready {
                image,
                source_extent,
            } => painter.with_save(|painter| {
                painter.clip_rounded_rect(rounded)?;
                painter.draw_external_image(
                    image,
                    bounds,
                    ImageOptions {
                        source: Some(Rect::from_xywh(
                            0.0,
                            0.0,
                            source_extent.width as f32,
                            source_extent.height as f32,
                        )),
                        sampling: self.sampling,
                        opacity: 1.0,
                    },
                )
            }),
            RenderViewContent::Composited { id, prefer_direct } => {
                if self.corner_radius == 0.0 {
                    painter.compositor_view(*id, bounds, *prefer_direct)
                } else {
                    // Rounded clips are intentionally represented in the display list;
                    // the compositor will select its texture-backed path.
                    painter.with_save(|painter| {
                        painter.clip_rounded_rect(rounded)?;
                        painter.compositor_view(*id, bounds, *prefer_direct)
                    })
                }
            }
        }
        .map_err(|e| UiError::from_message(e.to_string()))
    }
    fn semantics(&self) -> Option<(SemanticRole, String, Option<String>)> {
        Some((SemanticRole::Group, self.label.clone(), None))
    }
}

fn rounded_contains(point: LogicalPoint, size: LogicalSize, radius: f32) -> bool {
    if point.x < 0.0 || point.y < 0.0 || point.x > size.width || point.y > size.height {
        return false;
    }
    let r = radius.min(size.width * 0.5).min(size.height * 0.5);
    let cx = if point.x < r {
        r
    } else if point.x > size.width - r {
        size.width - r
    } else {
        point.x
    };
    let cy = if point.y < r {
        r
    } else if point.y > size.height - r {
        size.height - r
    } else {
        point.y
    };
    (point.x - cx).powi(2) + (point.y - cy).powi(2) <= r * r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_policy_buckets_and_applies_hysteresis() {
        let policy = RenderViewResizePolicy::default();
        let allocation = policy.allocation(None, Size::new(65, 63), true).unwrap();
        assert_eq!(allocation, Size::new(128, 64));
        assert_eq!(
            policy.allocation(Some(allocation), Size::new(100, 50), true),
            Some(allocation)
        );
        assert_eq!(
            policy.allocation(Some(allocation), Size::new(129, 50), true),
            Some(Size::new(192, 64))
        );
        assert_eq!(
            policy.allocation(Some(Size::new(256, 256)), Size::new(190, 200), true),
            Some(Size::new(192, 256))
        );
        assert_eq!(
            policy.allocation(Some(allocation), Size::new(0, 50), true),
            Some(allocation)
        );
        assert_eq!(
            policy.allocation(Some(allocation), Size::new(80, 50), false),
            Some(allocation)
        );
    }

    #[test]
    fn rounded_shape_excludes_corners() {
        assert!(!rounded_contains(
            LogicalPoint::new(0.0, 0.0),
            Size::new(100.0, 50.0),
            10.0
        ));
        assert!(rounded_contains(
            LogicalPoint::new(10.0, 10.0),
            Size::new(100.0, 50.0),
            10.0
        ));
    }
}
