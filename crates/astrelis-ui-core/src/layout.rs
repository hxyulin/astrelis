//! Layout style vocabulary resolved by Taffy.

use super::*;

/// Four-sided logical inset.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    /// Left inset.
    pub left: f32,
    /// Top inset.
    pub top: f32,
    /// Right inset.
    pub right: f32,
    /// Bottom inset.
    pub bottom: f32,
}

impl Insets {
    /// Creates equal insets on every side.
    pub const fn all(value: f32) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }
}

/// Cross-axis alignment for row and column containers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Alignment {
    /// Align children to the leading edge.
    Start,
    /// Center children.
    Center,
    /// Align children to the trailing edge.
    End,
    /// Stretch children across the available cross axis.
    #[default]
    Stretch,
}

/// A layout length resolved by Taffy.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Length {
    /// Let layout determine the value.
    #[default]
    Auto,
    /// Logical pixels.
    Px(f32),
    /// Fraction of the containing block (`1.0` is 100%).
    Percent(f32),
}

impl Length {
    /// Creates a logical-pixel length.
    pub const fn px(value: f32) -> Self {
        Self::Px(value)
    }
    /// Creates a fractional percentage length.
    pub const fn percent(value: f32) -> Self {
        Self::Percent(value)
    }
}

/// Four independently configurable edges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edges<T> {
    /// Left edge.
    pub left: T,
    /// Top edge.
    pub top: T,
    /// Right edge.
    pub right: T,
    /// Bottom edge.
    pub bottom: T,
}

impl<T: Copy> Edges<T> {
    /// Uses one value for every edge.
    pub const fn all(value: T) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }
}

impl<T: Default> Default for Edges<T> {
    fn default() -> Self {
        Self {
            left: T::default(),
            top: T::default(),
            right: T::default(),
            bottom: T::default(),
        }
    }
}

/// Whether an element participates in normal flow.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Positioning {
    /// Normal flex layout.
    #[default]
    Flow,
    /// Positioned relative to the containing block.
    Absolute,
}

/// Flex line wrapping policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexWrap {
    /// Keep one line.
    #[default]
    NoWrap,
    /// Wrap onto additional lines.
    Wrap,
    /// Wrap in the reverse cross-axis direction.
    WrapReverse,
}

/// Main-axis distribution of flex children.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Justification {
    /// Pack at the start.
    #[default]
    Start,
    /// Pack at the center.
    Center,
    /// Pack at the end.
    End,
    /// Equal space between children.
    SpaceBetween,
    /// Equal space around children.
    SpaceAround,
    /// Equal space around and at the edges.
    SpaceEvenly,
}

/// Flex-container configuration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlexStyle {
    /// Horizontal gap.
    pub column_gap: f32,
    /// Vertical gap.
    pub row_gap: f32,
    /// Cross-axis child alignment.
    pub align_items: Alignment,
    /// Main-axis distribution.
    pub justify_content: Justification,
    /// Wrapped-line distribution.
    pub align_content: Alignment,
    /// Wrapping policy.
    pub wrap: FlexWrap,
}

impl Default for FlexStyle {
    fn default() -> Self {
        Self {
            column_gap: 0.0,
            row_gap: 0.0,
            align_items: Alignment::Stretch,
            justify_content: Justification::Start,
            align_content: Alignment::Stretch,
            wrap: FlexWrap::NoWrap,
        }
    }
}

/// Participation in layout, painting, semantics, and input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Visibility {
    /// Fully visible and interactive.
    #[default]
    Visible,
    /// Retains layout space but is not painted or interactive.
    Hidden,
    /// Removed from layout and interaction.
    Collapsed,
}

/// Child overflow policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Overflow {
    /// Permit descendants outside the element bounds.
    #[default]
    Visible,
    /// Clip descendants to the element bounds.
    Clip,
}

/// Optional per-element sizing constraints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutStyle {
    /// Preferred width.
    pub width: Length,
    /// Preferred height.
    pub height: Length,
    /// Minimum width.
    pub min_width: Length,
    /// Minimum height.
    pub min_height: Length,
    /// Maximum width.
    pub max_width: Length,
    /// Maximum height.
    pub max_height: Length,
    /// Outer spacing.
    pub margin: Edges<Length>,
    /// Flex growth factor.
    pub grow: f32,
    /// Flex shrink factor.
    pub shrink: f32,
    /// Initial main-axis size.
    pub basis: Length,
    /// Per-child cross-axis alignment override.
    pub align_self: Option<Alignment>,
    /// Flow or absolute positioning.
    pub positioning: Positioning,
    /// Absolute-position offsets.
    pub inset: Edges<Length>,
    /// Preferred width divided by height.
    pub aspect_ratio: Option<f32>,
}

impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            width: Length::Auto,
            height: Length::Auto,
            min_width: Length::Auto,
            min_height: Length::Auto,
            max_width: Length::Auto,
            max_height: Length::Auto,
            margin: Edges::all(Length::Px(0.0)),
            grow: 0.0,
            shrink: 1.0,
            basis: Length::Auto,
            align_self: None,
            positioning: Positioning::Flow,
            inset: Edges::default(),
            aspect_ratio: None,
        }
    }
}

impl<Message: 'static> Ui<Message> {
    pub(crate) fn invalidate_layout(&mut self) {
        self.dirty |= Dirty::MEASURE | Dirty::LAYOUT | Dirty::PAINT | Dirty::SEMANTICS;
    }

    pub(crate) fn taffy_style(&self, id: ElementId, node: &Node) -> Style {
        let dimension = |value: Length| match value {
            Length::Auto => Dimension::auto(),
            Length::Px(value) => Dimension::length(value.max(0.0)),
            Length::Percent(value) => Dimension::percent(value.max(0.0)),
        };
        let edge = |value: Length| match value {
            Length::Auto => LengthPercentageAuto::auto(),
            Length::Px(value) => LengthPercentageAuto::length(value),
            Length::Percent(value) => LengthPercentageAuto::percent(value),
        };
        let mut style = Style {
            display: if node.visibility == Visibility::Collapsed {
                Display::None
            } else {
                Display::Flex
            },
            size: TaffySize {
                width: dimension(node.style.width),
                height: dimension(node.style.height),
            },
            min_size: TaffySize {
                width: dimension(node.style.min_width),
                height: dimension(node.style.min_height),
            },
            max_size: TaffySize {
                width: dimension(node.style.max_width),
                height: dimension(node.style.max_height),
            },
            margin: TaffyRect {
                left: edge(node.style.margin.left),
                top: edge(node.style.margin.top),
                right: edge(node.style.margin.right),
                bottom: edge(node.style.margin.bottom),
            },
            inset: TaffyRect {
                left: edge(node.style.inset.left),
                top: edge(node.style.inset.top),
                right: edge(node.style.inset.right),
                bottom: edge(node.style.inset.bottom),
            },
            position: if node.style.positioning == Positioning::Absolute {
                TaffyPosition::Absolute
            } else {
                TaffyPosition::Relative
            },
            flex_grow: node.style.grow.max(0.0),
            flex_shrink: node.style.shrink.max(0.0),
            flex_basis: dimension(node.style.basis),
            align_self: node.style.align_self.map(map_alignment),
            aspect_ratio: node
                .style
                .aspect_ratio
                .filter(|value| value.is_finite() && *value > 0.0),
            ..Default::default()
        };
        if node.parent.is_none() {
            style.size = TaffySize {
                width: Dimension::length(self.viewport.width.max(0.0)),
                height: Dimension::length(self.viewport.height.max(0.0)),
            };
        }
        if node.style.grow > 0.0 {
            if node.style.min_width == Length::Auto {
                style.min_size.width = Dimension::length(0.0);
            }
            if node.style.min_height == Length::Auto {
                style.min_size.height = Dimension::length(0.0);
            }
        }
        match node.kind {
            Kind::Row { flex } => {
                style.flex_direction = FlexDirection::Row;
                apply_flex(&mut style, flex);
            }
            Kind::Column { flex } => {
                style.flex_direction = FlexDirection::Column;
                apply_flex(&mut style, flex);
            }
            Kind::Stack => {}
            Kind::FocusScope { .. } => {
                style.flex_direction = FlexDirection::Column;
            }
            Kind::Overlay { .. } => {
                style.flex_direction = FlexDirection::Column;
                style.position = TaffyPosition::Absolute;
            }
            Kind::Padding { insets } => {
                style.flex_direction = FlexDirection::Column;
                style.padding.left = LengthPercentage::length(insets.left.max(0.0));
                style.padding.top = LengthPercentage::length(insets.top.max(0.0));
                style.padding.right = LengthPercentage::length(insets.right.max(0.0));
                style.padding.bottom = LengthPercentage::length(insets.bottom.max(0.0));
            }
            Kind::Button { .. } | Kind::TextField(_) => {
                let insets = self.theme.control_padding;
                style.padding.left = LengthPercentage::length(insets.left);
                style.padding.top = LengthPercentage::length(insets.top);
                style.padding.right = LengthPercentage::length(insets.right);
                style.padding.bottom = LengthPercentage::length(insets.bottom);
                if node.children.iter().any(|child| {
                    self.node(*child)
                        .is_ok_and(|node| matches!(node.kind, Kind::Overlay { .. }))
                }) && let Some(size) = node.text_layout.as_ref().map(TextLayout::size)
                {
                    if node.style.min_width == Length::Auto {
                        style.min_size.width =
                            Dimension::length(size.width + insets.left + insets.right);
                    }
                    if node.style.min_height == Length::Auto {
                        style.min_size.height =
                            Dimension::length(size.height + insets.top + insets.bottom);
                    }
                }
            }
            Kind::Checkbox { .. } | Kind::Slider { .. } => {
                style.min_size.height = Dimension::length(28.0);
                style.min_size.width =
                    Dimension::length(if matches!(node.kind, Kind::Slider { .. }) {
                        160.0
                    } else {
                        28.0
                    });
            }
            Kind::ScrollView { .. } => {
                style.flex_direction = FlexDirection::Column;
                style.overflow.y = TaffyOverflow::Scroll;
                if node.style.min_height == Length::Auto {
                    style.min_size.height = Dimension::length(0.0);
                }
            }
            Kind::Custom => {
                let container = self.custom_widgets.get(&id).map_or(
                    WidgetContainerStyle {
                        padding: self.theme.control_padding,
                        gap: self.theme.gap,
                    },
                    |widget| widget.container_style(&self.theme),
                );
                style.flex_direction = FlexDirection::Column;
                style.gap.height = LengthPercentage::length(container.gap.max(0.0));
                style.padding.left = LengthPercentage::length(container.padding.left.max(0.0));
                style.padding.top = LengthPercentage::length(container.padding.top.max(0.0));
                style.padding.right = LengthPercentage::length(container.padding.right.max(0.0));
                style.padding.bottom = LengthPercentage::length(container.padding.bottom.max(0.0));
            }
            Kind::Label { .. } => {}
        }
        if node
            .parent
            .and_then(|id| self.node(id).ok())
            .is_some_and(|parent| matches!(parent.kind, Kind::Stack))
        {
            style.position = TaffyPosition::Absolute;
            if node.style.inset == Edges::all(Length::Auto)
                && node.style.width == Length::Auto
                && node.style.height == Length::Auto
            {
                style.inset = TaffyRect {
                    left: LengthPercentageAuto::length(0.0),
                    top: LengthPercentageAuto::length(0.0),
                    right: LengthPercentageAuto::length(0.0),
                    bottom: LengthPercentageAuto::length(0.0),
                };
            }
        }
        style
    }

    pub(crate) fn build_taffy(
        &self,
        tree: &mut TaffyTree<ElementId>,
        id: ElementId,
        mapping: &mut HashMap<ElementId, NodeId>,
    ) -> Result<NodeId, UiError> {
        let node = self.node(id)?;
        let children = node
            .children
            .iter()
            .map(|child| self.build_taffy(tree, *child, mapping))
            .collect::<Result<Vec<_>, _>>()?;
        let style = self.taffy_style(id, node);
        let taffy_id = if children.is_empty() {
            tree.new_leaf_with_context(style, id)
        } else {
            let result = tree.new_with_children(style, &children);
            if let Ok(node_id) = result {
                tree.set_node_context(node_id, Some(id))
                    .map_err(|error| UiError::new(error.to_string()))?;
            }
            result
        }
        .map_err(|error| UiError::new(error.to_string()))?;
        mapping.insert(id, taffy_id);
        Ok(taffy_id)
    }

    pub(crate) fn ensure_layout(&mut self) -> Result<(), UiError> {
        if !self.dirty.intersects(Dirty::MEASURE | Dirty::LAYOUT) {
            return Ok(());
        }
        astrelis_profiling::profile_scope!("ui.layout");
        self.prepare_text_layouts()?;
        let mut tree = TaffyTree::<ElementId>::new();
        tree.disable_rounding();
        let mut mapping = HashMap::new();
        let root = self.build_taffy(&mut tree, self.root, &mut mapping)?;
        let mut layouts = self
            .ids()
            .filter_map(|id| {
                self.node(id)
                    .ok()
                    .and_then(|node| node.text_layout.as_ref().map(|layout| (id, layout.size())))
            })
            .collect::<HashMap<_, _>>();
        for (id, widget) in &self.custom_widgets {
            let size = widget.intrinsic_size(&self.theme);
            if size != Size::ZERO {
                layouts.insert(*id, size);
            }
        }
        tree.compute_layout_with_measure(
            root,
            TaffySize {
                width: AvailableSpace::Definite(self.viewport.width.max(0.0)),
                height: AvailableSpace::Definite(self.viewport.height.max(0.0)),
            },
            |known, _available, _node, context, _style| {
                let Some(id) = context.copied() else {
                    return TaffySize::ZERO;
                };
                let measured = layouts.get(&id).copied().unwrap_or(Size::ZERO);
                TaffySize {
                    width: known.width.unwrap_or(measured.width),
                    height: known.height.unwrap_or(measured.height),
                }
            },
        )
        .map_err(|error| UiError::new(error.to_string()))?;
        self.assign_layout(&tree, &mapping, self.root, LogicalPoint::ZERO)?;
        self.position_overlays()?;
        if self.focus.is_none() {
            let autofocus = self.ids().find(|id| self.node(*id).is_ok_and(|node| matches!(node.kind, Kind::FocusScope { options, .. } if options.autofocus) || matches!(node.kind, Kind::Overlay { options, .. } if options.focus.autofocus)));
            if let Some(scope) = autofocus {
                let target = self.ids().find(|id| {
                    self.is_descendant_of(*id, scope)
                        && self.is_effectively_interactive(*id)
                        && self.is_focusable_id(*id)
                });
                if let Some(target) = target {
                    self.set_focus(Some(target))?;
                }
            }
        }
        self.ensure_caret_visible()?;
        self.dirty.remove(Dirty::MEASURE | Dirty::LAYOUT);
        self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        if self
            .focus
            .is_some_and(|id| !self.is_effectively_interactive(id))
        {
            self.set_focus(None)?;
        }
        let hovered_devices = self.hover_paths.keys().copied().collect::<Vec<_>>();
        for device in hovered_devices {
            if let Some(position) = self.pointer_positions.get(&device).copied() {
                let target = self.hit_test(position);
                self.set_hover(device, position, target)?;
            }
        }
        Ok(())
    }

    pub(crate) fn position_overlays(&mut self) -> Result<(), UiError> {
        let overlays = self
            .ids()
            .filter(|id| {
                self.node(*id)
                    .is_ok_and(|node| matches!(node.kind, Kind::Overlay { .. }))
            })
            .collect::<Vec<_>>();
        for id in overlays {
            let (owner, options) = match self.node(id)?.kind {
                Kind::Overlay { owner, options, .. } => (owner, options),
                _ => continue,
            };
            let anchor = self.node(owner)?.bounds;
            let bounds = self.node(id)?.bounds;
            let mut x = match options.side {
                OverlaySide::Left => anchor.origin.x - bounds.size.width,
                OverlaySide::Right => anchor.max_x(),
                OverlaySide::Center => {
                    anchor.origin.x + (anchor.size.width - bounds.size.width) * 0.5
                }
                OverlaySide::Above | OverlaySide::Below => match options.alignment {
                    OverlayAlignment::Start => anchor.origin.x,
                    OverlayAlignment::Center => {
                        anchor.origin.x + (anchor.size.width - bounds.size.width) * 0.5
                    }
                    OverlayAlignment::End => anchor.max_x() - bounds.size.width,
                },
            };
            let mut y = match options.side {
                OverlaySide::Above => anchor.origin.y - bounds.size.height,
                OverlaySide::Below => anchor.max_y(),
                OverlaySide::Center => {
                    anchor.origin.y + (anchor.size.height - bounds.size.height) * 0.5
                }
                OverlaySide::Left | OverlaySide::Right => match options.alignment {
                    OverlayAlignment::Start => anchor.origin.y,
                    OverlayAlignment::Center => {
                        anchor.origin.y + (anchor.size.height - bounds.size.height) * 0.5
                    }
                    OverlayAlignment::End => anchor.max_y() - bounds.size.height,
                },
            };
            x += options.offset.x;
            y += options.offset.y;
            if options.clamp_to_viewport {
                x = x.clamp(0.0, (self.viewport.width - bounds.size.width).max(0.0));
                y = y.clamp(0.0, (self.viewport.height - bounds.size.height).max(0.0));
            }
            self.translate_subtree(id, x - bounds.origin.x, y - bounds.origin.y)?;
        }
        Ok(())
    }

    pub(crate) fn translate_subtree(
        &mut self,
        id: ElementId,
        x: f32,
        y: f32,
    ) -> Result<(), UiError> {
        let children = self.node(id)?.children.clone();
        let node = self.node_mut(id)?;
        node.bounds.origin.x += x;
        node.bounds.origin.y += y;
        for child in children {
            self.translate_subtree(child, x, y)?;
        }
        Ok(())
    }

    pub(crate) fn ensure_caret_visible(&mut self) -> Result<(), UiError> {
        let Some(focus) = self.focus else {
            return Ok(());
        };
        let node = self.node(focus)?;
        let Some(layout) = node.text_layout.clone() else {
            return Ok(());
        };
        let Kind::TextField(field) = &node.kind else {
            return Ok(());
        };
        let caret = layout.caret_rect(to_layout_position(field, field.caret), 1.0);
        let available = (node.bounds.size.width
            - self.theme.control_padding.left
            - self.theme.control_padding.right)
            .max(0.0);
        let mut offset = field.horizontal_offset;
        if caret.origin.x < offset {
            offset = caret.origin.x;
        } else if caret.origin.x + caret.size.width > offset + available {
            offset = (caret.origin.x + caret.size.width - available).max(0.0);
        }
        self.text_field_mut(focus)?.horizontal_offset = offset;
        Ok(())
    }

    pub(crate) fn assign_layout(
        &mut self,
        tree: &TaffyTree<ElementId>,
        mapping: &HashMap<ElementId, NodeId>,
        id: ElementId,
        parent_origin: LogicalPoint,
    ) -> Result<(), UiError> {
        let layout = tree
            .layout(mapping[&id])
            .map_err(|error| UiError::new(error.to_string()))?;
        let origin = Point::new(
            parent_origin.x + layout.location.x,
            parent_origin.y + layout.location.y,
        );
        let children = self.node(id)?.children.clone();
        self.node_mut(id)?.bounds =
            Rect::from_xywh(origin.x, origin.y, layout.size.width, layout.size.height);
        for child in children {
            self.assign_layout(tree, mapping, child, origin)?;
        }
        if matches!(self.node(id)?.kind, Kind::ScrollView { .. }) {
            let bottom = self
                .node(id)?
                .children
                .iter()
                .filter_map(|child| self.subtree_bottom(*child).ok())
                .fold(origin.y, f32::max);
            let content_height = (bottom - origin.y).max(self.node(id)?.bounds.size.height);
            if let Kind::ScrollView {
                content_height: current,
                offset,
                ..
            } = &mut self.node_mut(id)?.kind
            {
                *current = content_height;
                *offset = (*offset).clamp(0.0, (content_height - layout.size.height).max(0.0));
            }
        }
        Ok(())
    }

    pub(crate) fn subtree_bottom(&self, id: ElementId) -> Result<f32, UiError> {
        let node = self.node(id)?;
        let mut bottom = node.bounds.max_y();
        if matches!(node.kind, Kind::ScrollView { .. }) {
            return Ok(bottom);
        }
        for child in &node.children {
            if !matches!(self.node(*child)?.kind, Kind::Overlay { .. }) {
                bottom = bottom.max(self.subtree_bottom(*child)?);
            }
        }
        Ok(bottom)
    }

    /// Returns the pointer target at a logical viewport position.
    pub fn hit_test_at(&mut self, point: LogicalPoint) -> Result<Option<ElementId>, UiError> {
        self.ensure_layout()?;
        Ok(self.hit_test(point))
    }

    /// Returns the current untransformed logical layout bounds of an element.
    pub fn layout_bounds<T>(&mut self, handle: ElementHandle<T>) -> Result<LogicalRect, UiError> {
        self.ensure_layout()?;
        Ok(self.node(handle.id)?.bounds)
    }
}
