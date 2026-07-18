//! Display-list construction and node painting.

use super::*;

impl<Message: 'static> Ui<Message> {
    pub(crate) fn collect_paint_order(
        &self,
        id: ElementId,
        output: &mut Vec<ElementId>,
    ) -> Result<(), UiError> {
        let node = self.node(id)?;
        if node.visibility != Visibility::Visible {
            return Ok(());
        }
        output.push(id);
        let ordered = self.z_sorted_children(node);
        for child in ordered.as_deref().unwrap_or(&node.children) {
            if matches!(self.node(*child)?.kind, Kind::Overlay { .. }) {
                continue;
            }
            self.collect_paint_order(*child, output)?;
        }
        Ok(())
    }

    /// Generates the current backend-independent display list.
    pub fn display_list(&mut self) -> Result<DisplayList, UiError> {
        self.ensure_layout()?;
        astrelis_profiling::profile_scope!("ui.paint");
        let mut painter = Painter::new();
        painter
            .fill_rect(
                Rect::from_xywh(0.0, 0.0, self.viewport.width, self.viewport.height),
                Brush::Solid(self.theme.background),
            )
            .map_err(|error| UiError::new(error.to_string()))?;
        self.paint_node(self.root, &mut painter)?;
        let mut overlays = self
            .ids()
            .filter(|id| {
                self.node(*id)
                    .is_ok_and(|node| matches!(node.kind, Kind::Overlay { .. }))
            })
            .collect::<Vec<_>>();
        overlays.sort_by_key(|id| self.node(*id).map_or(0, |node| node.z_index));
        for overlay in overlays {
            self.paint_node(overlay, &mut painter)?;
        }
        let list = painter
            .finish()
            .map_err(|error| UiError::new(error.to_string()))?;
        self.dirty.remove(Dirty::PAINT);
        Ok(list)
    }

    pub(crate) fn paint_node(&self, id: ElementId, painter: &mut Painter) -> Result<(), UiError> {
        let node = self.node(id)?;
        if node.visibility != Visibility::Visible {
            return Ok(());
        }
        painter.save();
        let transform = node_local_transform(node);
        if transform != Affine2::IDENTITY {
            painter
                .transform(transform)
                .map_err(|error| UiError::new(error.to_string()))?;
        }
        if node.overflow == Overflow::Clip {
            painter
                .clip_rect(node.bounds)
                .map_err(|error| UiError::new(error.to_string()))?;
        }
        match &node.kind {
            Kind::Overlay { .. } => {
                // Overlays are floating surfaces (tooltips, popovers, menus).
                // Resolve the surface from the theme at paint time so it tracks
                // set_theme; an explicit override still wins.
                let background = node.visual.background.unwrap_or(self.theme.surface);
                let rounded = RoundedRect::new(
                    node.bounds,
                    CornerRadii::uniform(self.theme.radii.md.max(0.0)),
                )
                .map_err(|error| UiError::new(error.to_string()))?;
                self.paint_shadow(painter, node.bounds)?;
                painter
                    .fill_rounded_rect(rounded, Brush::Solid(background))
                    .map_err(|error| UiError::new(error.to_string()))?;
                painter
                    .stroke_rounded_rect(
                        rounded,
                        StrokeStyle {
                            width: self.theme.border_width,
                            ..Default::default()
                        },
                        Brush::Solid(self.theme.border),
                    )
                    .map_err(|error| UiError::new(error.to_string()))?;
            }
            Kind::Row { .. }
            | Kind::Column { .. }
            | Kind::Stack
            | Kind::FocusScope { .. }
            | Kind::Padding { .. }
                if node.visual.background.is_some() =>
            {
                painter
                    .fill_rect(
                        node.bounds,
                        Brush::Solid(node.visual.background.expect("checked above")),
                    )
                    .map_err(|error| UiError::new(error.to_string()))?;
            }
            Kind::Button { .. } => {
                let color = self.theme.button.resolve(ControlState {
                    enabled: node.enabled,
                    hovered: node.hovered,
                    pressed: node.pressed,
                });
                self.fill_control(
                    painter,
                    node.bounds,
                    node.visual.background.unwrap_or(color),
                )?;
            }
            Kind::TextField(_) => {
                self.fill_control(
                    painter,
                    node.bounds,
                    node.visual
                        .background
                        .unwrap_or(self.theme.field_background),
                )?;
            }
            Kind::Checkbox { checked, style } => {
                // Only enabled/disabled applies to a checkbox box; there is no
                // hover or press visual, so both are false in the resolve.
                let resolved = self.theme.button.resolve(ControlState {
                    enabled: node.enabled,
                    hovered: false,
                    pressed: false,
                });
                let background = node
                    .visual
                    .background
                    .or(style.background)
                    .unwrap_or(resolved);
                let radius = style.radius.unwrap_or(self.theme.radii.md).max(0.0);
                painter
                    .fill_rounded_rect(
                        RoundedRect::new(node.bounds, CornerRadii::uniform(radius))
                            .map_err(|error| UiError::new(error.to_string()))?,
                        Brush::Solid(background),
                    )
                    .map_err(|error| UiError::new(error.to_string()))?;
                if *checked {
                    let inset = self.theme.metrics.checkbox_inset;
                    painter
                        .fill_rounded_rect(
                            RoundedRect::new(
                                Rect::from_xywh(
                                    node.bounds.origin.x + inset,
                                    node.bounds.origin.y + inset,
                                    (node.bounds.size.width - inset * 2.0).max(0.0),
                                    (node.bounds.size.height - inset * 2.0).max(0.0),
                                ),
                                CornerRadii::uniform(self.theme.radii.sm),
                            )
                            .map_err(|error| UiError::new(error.to_string()))?,
                            Brush::Solid(style.indicator.unwrap_or(if node.enabled {
                                self.theme.accent
                            } else {
                                self.theme.disabled_foreground
                            })),
                        )
                        .map_err(|error| UiError::new(error.to_string()))?;
                }
            }
            Kind::Slider {
                min,
                max,
                value,
                style,
                ..
            } => {
                let track_color = style.track.unwrap_or_else(|| {
                    self.theme.button.resolve(ControlState {
                        enabled: node.enabled,
                        hovered: false,
                        pressed: false,
                    })
                });
                let thumb_color = style.thumb.unwrap_or(if node.enabled {
                    self.theme.accent
                } else {
                    self.theme.disabled_foreground
                });
                let outline_color = if node.enabled {
                    self.theme.foreground
                } else {
                    self.theme.disabled_foreground
                };
                let thumb_size = style.thumb_size.unwrap_or(self.theme.metrics.slider_thumb);
                let track_height = self.theme.metrics.slider_track;
                let center_y = node.bounds.origin.y + node.bounds.size.height * 0.5;
                let track = Rect::from_xywh(
                    node.bounds.origin.x,
                    center_y - track_height * 0.5,
                    node.bounds.size.width,
                    track_height,
                );
                painter
                    .fill_rounded_rect(
                        RoundedRect::new(track, CornerRadii::uniform(track_height * 0.5))
                            .map_err(|error| UiError::new(error.to_string()))?,
                        Brush::Solid(track_color),
                    )
                    .map_err(|error| UiError::new(error.to_string()))?;
                let t = (*value - *min) / (*max - *min);
                let thumb = Rect::from_xywh(
                    node.bounds.origin.x + t * node.bounds.size.width - thumb_size * 0.5,
                    center_y - thumb_size * 0.5,
                    thumb_size,
                    thumb_size,
                );
                painter
                    .fill_ellipse(thumb, Brush::Solid(thumb_color))
                    .map_err(|error| UiError::new(error.to_string()))?;
                painter
                    .stroke_ellipse(
                        thumb,
                        StrokeStyle {
                            width: 1.0,
                            ..Default::default()
                        },
                        Brush::Solid(outline_color),
                    )
                    .map_err(|error| UiError::new(error.to_string()))?;
            }
            Kind::ScrollView { .. } => {}
            Kind::Custom => {
                if let Some(widget) = self.custom_widgets.get(&id) {
                    widget.paint(painter, node.bounds, &self.theme)?;
                }
            }
            _ => {}
        }

        if self.focus == Some(id) && self.is_focusable_id(id) {
            let thickness = self.theme.metrics.focus_ring;
            let bounds = Rect::from_xywh(
                node.bounds.origin.x,
                node.bounds.origin.y,
                node.bounds.size.width,
                thickness,
            );
            painter
                .fill_rect(bounds, Brush::Solid(self.theme.accent))
                .map_err(|error| UiError::new(error.to_string()))?;
        }

        if let Some(layout) = &node.text_layout {
            let mut origin = node.bounds.origin;
            if matches!(node.kind, Kind::Button { .. } | Kind::TextField(_)) {
                origin.x += self.theme.control_padding.left;
                origin.y += self.theme.control_padding.top;
            }
            if let Kind::TextField(field) = &node.kind {
                let content = Rect::from_xywh(
                    node.bounds.origin.x + self.theme.control_padding.left,
                    node.bounds.origin.y + self.theme.control_padding.top,
                    (node.bounds.size.width
                        - self.theme.control_padding.left
                        - self.theme.control_padding.right)
                        .max(0.0),
                    (node.bounds.size.height
                        - self.theme.control_padding.top
                        - self.theme.control_padding.bottom)
                        .max(0.0),
                );
                painter.save();
                painter
                    .clip_rect(content)
                    .map_err(|error| UiError::new(error.to_string()))?;
                origin.x -= field.horizontal_offset;
                if self.focus == Some(id) && !field.text.is_empty() {
                    let (start, end) = field.selection();
                    if start != end {
                        for rect in layout.selection_rects(
                            to_layout_position(
                                field,
                                TextPosition {
                                    byte_index: start,
                                    ..Default::default()
                                },
                            ),
                            to_layout_position(
                                field,
                                TextPosition {
                                    byte_index: end,
                                    ..Default::default()
                                },
                            ),
                        ) {
                            painter
                                .fill_rect(
                                    Rect::from_xywh(
                                        origin.x + rect.origin.x,
                                        origin.y + rect.origin.y,
                                        rect.size.width,
                                        rect.size.height,
                                    ),
                                    Brush::Solid(self.theme.selection),
                                )
                                .map_err(|error| UiError::new(error.to_string()))?;
                        }
                    }
                }
                painter
                    .draw_text(layout, origin, 1.0)
                    .map_err(|error| UiError::new(error.to_string()))?;
                if self.focus == Some(id) {
                    let caret = layout.caret_rect(to_layout_position(field, field.caret), 1.0);
                    painter
                        .fill_rect(
                            Rect::from_xywh(
                                origin.x + caret.origin.x,
                                origin.y + caret.origin.y,
                                caret.size.width.max(1.0),
                                caret.size.height,
                            ),
                            Brush::Solid(self.theme.accent),
                        )
                        .map_err(|error| UiError::new(error.to_string()))?;
                }
                painter
                    .restore()
                    .map_err(|error| UiError::new(error.to_string()))?;
            } else {
                let clipped_control = matches!(node.kind, Kind::Button { .. });
                if clipped_control {
                    painter.save();
                    painter
                        .clip_rect(node.bounds)
                        .map_err(|error| UiError::new(error.to_string()))?;
                }
                painter
                    .draw_text(layout, origin, 1.0)
                    .map_err(|error| UiError::new(error.to_string()))?;
                if clipped_control {
                    painter
                        .restore()
                        .map_err(|error| UiError::new(error.to_string()))?;
                }
            }
        }
        let scroll_offset = match node.kind {
            Kind::ScrollView { offset, .. } => Some(offset),
            _ => None,
        };
        if let Some(offset) = scroll_offset {
            painter.save();
            painter
                .clip_rect(node.bounds)
                .map_err(|error| UiError::new(error.to_string()))?;
            painter
                .transform(Affine2::from_translation(Vec2::new(0.0, -offset)))
                .map_err(|error| UiError::new(error.to_string()))?;
        }
        let ordered = self.z_sorted_children(node);
        for child in ordered.as_deref().unwrap_or(&node.children) {
            if matches!(self.node(*child)?.kind, Kind::Overlay { .. }) {
                continue;
            }
            self.paint_node(*child, painter)?;
        }
        if scroll_offset.is_some() {
            painter
                .restore()
                .map_err(|error| UiError::new(error.to_string()))?;
            if let Kind::ScrollView {
                offset,
                content_height,
                style,
            } = &node.kind
                && *content_height > node.bounds.size.height + f32::EPSILON
            {
                let track_color = style.track.unwrap_or(self.theme.button.normal);
                let thumb_color = style.thumb.unwrap_or(self.theme.accent);
                let width = style
                    .width
                    .unwrap_or(self.theme.metrics.scrollbar_width)
                    .max(1.0);
                let track = Rect::from_xywh(
                    node.bounds.max_x() - width,
                    node.bounds.origin.y,
                    width,
                    node.bounds.size.height,
                );
                painter
                    .fill_rounded_rect(
                        RoundedRect::new(track, CornerRadii::uniform(width * 0.5))
                            .map_err(|error| UiError::new(error.to_string()))?,
                        Brush::Solid(track_color),
                    )
                    .map_err(|error| UiError::new(error.to_string()))?;
                let thumb_height = (node.bounds.size.height * node.bounds.size.height
                    / *content_height)
                    .max(self.theme.metrics.scrollbar_min_thumb)
                    .min(node.bounds.size.height);
                let travel = node.bounds.size.height - thumb_height;
                let max_offset = *content_height - node.bounds.size.height;
                let thumb = Rect::from_xywh(
                    track.origin.x,
                    track.origin.y + travel * *offset / max_offset,
                    width,
                    thumb_height,
                );
                painter
                    .fill_rounded_rect(
                        RoundedRect::new(thumb, CornerRadii::uniform(width * 0.5))
                            .map_err(|error| UiError::new(error.to_string()))?,
                        Brush::Solid(thumb_color),
                    )
                    .map_err(|error| UiError::new(error.to_string()))?;
            }
        }
        painter
            .restore()
            .map_err(|error| UiError::new(error.to_string()))?;
        Ok(())
    }

    pub(crate) fn fill_control(
        &self,
        painter: &mut Painter,
        bounds: LogicalRect,
        color: Color,
    ) -> Result<(), UiError> {
        let rounded = RoundedRect::new(bounds, CornerRadii::uniform(self.theme.radii.md.max(0.0)))
            .map_err(|error| UiError::new(error.to_string()))?;
        painter
            .fill_rounded_rect(rounded, Brush::Solid(color))
            .map_err(|error| UiError::new(error.to_string()))
    }

    /// Approximates the theme's drop-shadow behind `bounds`.
    ///
    /// `astrelis-paint` has no gaussian-blur primitive, so the penumbra is a
    /// short stack of translucent rounded rects that grow outward and fade.
    /// The caller paints the opaque surface over the returned shadow.
    pub(crate) fn paint_shadow(
        &self,
        painter: &mut Painter,
        bounds: LogicalRect,
    ) -> Result<(), UiError> {
        let shadow = self.theme.shadow;
        if shadow.color.a <= 0.0 || shadow.blur <= 0.0 {
            return Ok(());
        }
        let layers = 4;
        for i in 0..layers {
            let step = (i + 1) as f32 / layers as f32;
            let spread = shadow.blur * step;
            let mut color = shadow.color;
            color.a *= (1.0 - step) * 0.6 + 0.1;
            let rect = Rect::from_xywh(
                bounds.origin.x + shadow.offset.x - spread,
                bounds.origin.y + shadow.offset.y - spread,
                bounds.size.width + spread * 2.0,
                bounds.size.height + spread * 2.0,
            );
            painter
                .fill_rounded_rect(
                    RoundedRect::new(rect, CornerRadii::uniform(self.theme.radii.md + spread))
                        .map_err(|error| UiError::new(error.to_string()))?,
                    Brush::Solid(color),
                )
                .map_err(|error| UiError::new(error.to_string()))?;
        }
        Ok(())
    }
}
