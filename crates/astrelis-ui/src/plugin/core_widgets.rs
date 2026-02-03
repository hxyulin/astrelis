//! Render, measure, and event handler functions for core widget types.
//!
//! These functions are registered via [`CorePlugin`] and dispatched
//! through the [`WidgetTypeRegistry`] instead of downcast chains.

use crate::draw_list::{DrawCommand, ImageCommand, QuadCommand, TextCommand};
use crate::plugin::registry::{EventResponse, WidgetOverflow, WidgetRenderContext};
use crate::widgets::scroll_container::ScrollContainer;
use crate::style::Overflow;
use crate::widgets::{Button, Container, HScrollbar, Image, Text, TextInput, Tooltip, VScrollbar};
use astrelis_core::math::Vec2;
use astrelis_winit::event::PhysicalKey;
use std::any::Any;

// ---------------------------------------------------------------------------
// Container
// ---------------------------------------------------------------------------

pub fn render_container(widget: &dyn Any, ctx: &mut WidgetRenderContext<'_>) -> Vec<DrawCommand> {
    let container = widget.downcast_ref::<Container>().unwrap();
    let mut commands = Vec::new();

    // Background quad
    if let Some(bg_color) = container.style.background_color {
        commands.push(DrawCommand::Quad(
            QuadCommand::rounded(
                ctx.abs_position,
                ctx.layout_size,
                bg_color,
                container.style.border_radius,
                0,
            )
            .with_clip(ctx.clip_rect),
        ));
    }

    // Border quad
    if container.style.border_width > 0.0
        && let Some(border_color) = container.style.border_color {
            commands.push(DrawCommand::Quad(
                QuadCommand::bordered(
                    ctx.abs_position,
                    ctx.layout_size,
                    border_color,
                    container.style.border_width,
                    container.style.border_radius,
                    0,
                )
                .with_clip(ctx.clip_rect),
            ));
        }

    commands
}

pub fn container_overflow(widget: &dyn Any) -> WidgetOverflow {
    let container = widget.downcast_ref::<Container>().unwrap();
    WidgetOverflow {
        overflow_x: container.style.overflow_x,
        overflow_y: container.style.overflow_y,
    }
}

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

pub fn render_text(widget: &dyn Any, ctx: &mut WidgetRenderContext<'_>) -> Vec<DrawCommand> {
    let text = widget.downcast_ref::<Text>().unwrap();
    let mut commands = Vec::new();

    let request_id =
        ctx.text_pipeline
            .request_shape(text.content.clone(), text.font_id, text.font_size, None);

    if let Some(shaped) = ctx.text_pipeline.get_completed(request_id) {
        use astrelis_text::VerticalAlign;
        let text_height = shaped.bounds().1;
        let text_y = match text.vertical_align {
            VerticalAlign::Top => ctx.abs_position.y,
            VerticalAlign::Center => ctx.abs_position.y + (ctx.layout_size.y - text_height) * 0.5,
            VerticalAlign::Bottom => ctx.abs_position.y + (ctx.layout_size.y - text_height),
        };

        let text_color = text.color.unwrap_or(ctx.theme_colors.text_primary);
        commands.push(DrawCommand::Text(
            TextCommand::new(
                Vec2::new(ctx.abs_position.x, text_y),
                shaped,
                text_color,
                0,
            )
            .with_clip(ctx.clip_rect),
        ));
    }

    commands
}

// ---------------------------------------------------------------------------
// Button
// ---------------------------------------------------------------------------

pub fn render_button(widget: &dyn Any, ctx: &mut WidgetRenderContext<'_>) -> Vec<DrawCommand> {
    let button = widget.downcast_ref::<Button>().unwrap();
    let mut commands = Vec::new();

    let bg_color = button.current_bg_color_themed(ctx.theme_colors.primary, None, None);

    // Background
    commands.push(DrawCommand::Quad(
        QuadCommand::rounded(ctx.abs_position, ctx.layout_size, bg_color, 4.0, 0)
            .with_clip(ctx.clip_rect),
    ));

    // Text label
    let request_id = ctx.text_pipeline.request_shape(
        button.label.clone(),
        button.font_id,
        button.font_size,
        None,
    );

    if let Some(shaped) = ctx.text_pipeline.get_completed(request_id) {
        let text_x = ctx.abs_position.x + (ctx.layout_size.x - shaped.bounds().0) * 0.5;
        let text_height = shaped.bounds().1;
        let text_y = ctx.abs_position.y + (ctx.layout_size.y - text_height) * 0.5;

        let btn_text_color = button.text_color.unwrap_or(ctx.theme_colors.text_primary);
        commands.push(DrawCommand::Text(
            TextCommand::new(Vec2::new(text_x, text_y), shaped, btn_text_color, 1)
                .with_clip(ctx.clip_rect),
        ));
    }

    commands
}

// ---------------------------------------------------------------------------
// Image
// ---------------------------------------------------------------------------

pub fn render_image(widget: &dyn Any, ctx: &mut WidgetRenderContext<'_>) -> Vec<DrawCommand> {
    let image = widget.downcast_ref::<Image>().unwrap();
    let mut commands = Vec::new();

    if let Some(texture) = &image.texture {
        commands.push(DrawCommand::Image(
            ImageCommand::new(
                ctx.abs_position,
                ctx.layout_size,
                texture.clone(),
                image.uv,
                image.tint,
                image.border_radius,
                image.sampling,
                0,
            )
            .with_clip(ctx.clip_rect),
        ));
    }

    commands
}

// ---------------------------------------------------------------------------
// Tooltip
// ---------------------------------------------------------------------------

pub fn render_tooltip(widget: &dyn Any, _ctx: &mut WidgetRenderContext<'_>) -> Vec<DrawCommand> {
    let _tooltip = widget.downcast_ref::<Tooltip>().unwrap();
    // Tooltips are rendered through the overlay system, not the main render path.
    Vec::new()
}

// ---------------------------------------------------------------------------
// HScrollbar
// ---------------------------------------------------------------------------

pub fn render_hscrollbar(widget: &dyn Any, ctx: &mut WidgetRenderContext<'_>) -> Vec<DrawCommand> {
    let scrollbar = widget.downcast_ref::<HScrollbar>().unwrap();
    let mut commands = Vec::new();

    if scrollbar.needs_scrollbar() {
        let track_rect = crate::tree::LayoutRect {
            x: ctx.abs_position.x,
            y: ctx.abs_position.y,
            width: ctx.layout_size.x,
            height: ctx.layout_size.y,
        };

        // Track background
        commands.push(DrawCommand::Quad(
            QuadCommand::rounded(
                ctx.abs_position,
                ctx.layout_size,
                scrollbar.theme.track_color,
                scrollbar.theme.thumb_border_radius,
                0,
            )
            .with_clip(ctx.clip_rect),
        ));

        // Thumb
        let thumb = scrollbar.thumb_bounds(&track_rect);
        let thumb_color = scrollbar.current_thumb_color();
        commands.push(DrawCommand::Quad(
            QuadCommand::rounded(
                Vec2::new(thumb.x, thumb.y),
                Vec2::new(thumb.width, thumb.height),
                thumb_color,
                scrollbar.theme.thumb_border_radius,
                1,
            )
            .with_clip(ctx.clip_rect),
        ));
    }

    commands
}

// ---------------------------------------------------------------------------
// VScrollbar
// ---------------------------------------------------------------------------

pub fn render_vscrollbar(widget: &dyn Any, ctx: &mut WidgetRenderContext<'_>) -> Vec<DrawCommand> {
    let scrollbar = widget.downcast_ref::<VScrollbar>().unwrap();
    let mut commands = Vec::new();

    if scrollbar.needs_scrollbar() {
        let track_rect = crate::tree::LayoutRect {
            x: ctx.abs_position.x,
            y: ctx.abs_position.y,
            width: ctx.layout_size.x,
            height: ctx.layout_size.y,
        };

        // Track background
        commands.push(DrawCommand::Quad(
            QuadCommand::rounded(
                ctx.abs_position,
                ctx.layout_size,
                scrollbar.theme.track_color,
                scrollbar.theme.thumb_border_radius,
                0,
            )
            .with_clip(ctx.clip_rect),
        ));

        // Thumb
        let thumb = scrollbar.thumb_bounds(&track_rect);
        let thumb_color = scrollbar.current_thumb_color();
        commands.push(DrawCommand::Quad(
            QuadCommand::rounded(
                Vec2::new(thumb.x, thumb.y),
                Vec2::new(thumb.width, thumb.height),
                thumb_color,
                scrollbar.theme.thumb_border_radius,
                1,
            )
            .with_clip(ctx.clip_rect),
        ));
    }

    commands
}

// ---------------------------------------------------------------------------
// ScrollContainer
// ---------------------------------------------------------------------------

pub fn render_scroll_container(
    widget: &dyn Any,
    ctx: &mut WidgetRenderContext<'_>,
) -> Vec<DrawCommand> {
    let sc = widget.downcast_ref::<ScrollContainer>().unwrap();
    let mut commands = Vec::new();

    // Background quad
    if let Some(bg_color) = sc.style.background_color {
        commands.push(DrawCommand::Quad(
            QuadCommand::rounded(
                ctx.abs_position,
                ctx.layout_size,
                bg_color,
                sc.style.border_radius,
                0,
            )
            .with_clip(ctx.clip_rect),
        ));
    }

    // Border quad
    if sc.style.border_width > 0.0
        && let Some(border_color) = sc.style.border_color {
            commands.push(DrawCommand::Quad(
                QuadCommand::bordered(
                    ctx.abs_position,
                    ctx.layout_size,
                    border_color,
                    sc.style.border_width,
                    sc.style.border_radius,
                    0,
                )
                .with_clip(ctx.clip_rect),
            ));
        }

    let abs_layout = crate::tree::LayoutRect {
        x: ctx.abs_position.x,
        y: ctx.abs_position.y,
        width: ctx.layout_size.x,
        height: ctx.layout_size.y,
    };

    // Vertical scrollbar
    if sc.should_show_v_scrollbar() {
        let track = sc.v_scrollbar_track(&abs_layout);
        commands.push(DrawCommand::Quad(
            QuadCommand::rounded(
                Vec2::new(track.x, track.y),
                Vec2::new(track.width, track.height),
                sc.scrollbar_theme.track_color,
                sc.scrollbar_theme.thumb_border_radius,
                2,
            )
            .with_clip(ctx.clip_rect),
        ));
        let thumb = sc.v_scrollbar_thumb(&abs_layout);
        commands.push(DrawCommand::Quad(
            QuadCommand::rounded(
                Vec2::new(thumb.x, thumb.y),
                Vec2::new(thumb.width, thumb.height),
                sc.v_thumb_color(),
                sc.scrollbar_theme.thumb_border_radius,
                3,
            )
            .with_clip(ctx.clip_rect),
        ));
    }

    // Horizontal scrollbar
    if sc.should_show_h_scrollbar() {
        let track = sc.h_scrollbar_track(&abs_layout);
        commands.push(DrawCommand::Quad(
            QuadCommand::rounded(
                Vec2::new(track.x, track.y),
                Vec2::new(track.width, track.height),
                sc.scrollbar_theme.track_color,
                sc.scrollbar_theme.thumb_border_radius,
                2,
            )
            .with_clip(ctx.clip_rect),
        ));
        let thumb = sc.h_scrollbar_thumb(&abs_layout);
        commands.push(DrawCommand::Quad(
            QuadCommand::rounded(
                Vec2::new(thumb.x, thumb.y),
                Vec2::new(thumb.width, thumb.height),
                sc.h_thumb_color(),
                sc.scrollbar_theme.thumb_border_radius,
                3,
            )
            .with_clip(ctx.clip_rect),
        ));
    }

    commands
}

pub fn scroll_container_offset(widget: &dyn Any) -> Vec2 {
    let sc = widget.downcast_ref::<ScrollContainer>().unwrap();
    sc.scroll_offset
}

pub fn scroll_container_clips(_widget: &dyn Any) -> bool {
    true
}

pub fn scroll_container_overflow(_widget: &dyn Any) -> WidgetOverflow {
    WidgetOverflow {
        overflow_x: Overflow::Hidden,
        overflow_y: Overflow::Hidden,
    }
}

// ---------------------------------------------------------------------------
// Button event handlers
// ---------------------------------------------------------------------------

/// Handle mouse hover enter/leave for Button.
pub fn button_hover(widget: &mut dyn Any, entering: bool) {
    let button = widget.downcast_mut::<Button>().unwrap();
    button.is_hovered = entering;
}

/// Handle mouse press/release for Button.
pub fn button_press(widget: &mut dyn Any, pressing: bool) {
    let button = widget.downcast_mut::<Button>().unwrap();
    button.is_pressed = pressing;
}

/// Handle click for Button — invokes the user callback.
pub fn button_click(widget: &mut dyn Any) -> EventResponse {
    let button = widget.downcast_mut::<Button>().unwrap();
    if let Some(callback) = button.on_click.clone() {
        callback();
        tracing::debug!("Button clicked: {}", button.label);
    }
    EventResponse::None
}

// ---------------------------------------------------------------------------
// TextInput event handlers
// ---------------------------------------------------------------------------

/// Handle click for TextInput — sets focus.
pub fn text_input_click(widget: &mut dyn Any) -> EventResponse {
    let text_input = widget.downcast_mut::<TextInput>().unwrap();
    text_input.is_focused = true;
    tracing::debug!("Text input focused");
    EventResponse::RequestFocus
}

/// Handle keyboard input for TextInput.
pub fn text_input_key(widget: &mut dyn Any, key: &PhysicalKey) -> EventResponse {
    let text_input = widget.downcast_mut::<TextInput>().unwrap();
    if let PhysicalKey::Code(code) = key {
        use astrelis_winit::event::KeyCode;
        match code {
            KeyCode::Backspace => {
                text_input.delete_char();
            }
            KeyCode::Escape => {
                text_input.is_focused = false;
                return EventResponse::ReleaseFocus;
            }
            _ => {}
        }
    }
    EventResponse::None
}

/// Handle character input for TextInput.
pub fn text_input_char(widget: &mut dyn Any, c: char) {
    let text_input = widget.downcast_mut::<TextInput>().unwrap();
    if !c.is_control() {
        text_input.insert_char(c);
    }
}
