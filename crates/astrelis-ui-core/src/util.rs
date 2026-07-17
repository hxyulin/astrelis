//! Internal geometry, layout, and text helpers shared across the core.

use super::*;

pub(crate) fn map_alignment(alignment: Alignment) -> AlignItems {
    match alignment {
        Alignment::Start => AlignItems::FLEX_START,
        Alignment::Center => AlignItems::CENTER,
        Alignment::End => AlignItems::FLEX_END,
        Alignment::Stretch => AlignItems::STRETCH,
    }
}

pub(crate) fn node_local_transform(node: &Node) -> Affine2 {
    let pivot = Vec2::new(
        node.bounds.origin.x + node.transform_origin.x,
        node.bounds.origin.y + node.transform_origin.y,
    );
    Affine2::from_translation(pivot) * node.transform * Affine2::from_translation(-pivot)
}

pub(crate) fn transformed_bounds(rect: LogicalRect, transform: Affine2) -> LogicalRect {
    let points = [
        Vec2::new(rect.min_x(), rect.min_y()),
        Vec2::new(rect.max_x(), rect.min_y()),
        Vec2::new(rect.max_x(), rect.max_y()),
        Vec2::new(rect.min_x(), rect.max_y()),
    ]
    .map(|point| transform.transform_point2(point));
    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);
    Rect::from_xywh(
        min_x,
        min_y,
        (max_x - min_x).max(0.0),
        (max_y - min_y).max(0.0),
    )
}

pub(crate) fn intersect_rect(a: LogicalRect, b: LogicalRect) -> LogicalRect {
    let x = a.min_x().max(b.min_x());
    let y = a.min_y().max(b.min_y());
    let max_x = a.max_x().min(b.max_x());
    let max_y = a.max_y().min(b.max_y());
    Rect::from_xywh(x, y, (max_x - x).max(0.0), (max_y - y).max(0.0))
}

pub(crate) fn scale_rect(rect: LogicalRect, scale: f32) -> PhysicalRect {
    Rect::from_xywh(
        rect.origin.x * scale,
        rect.origin.y * scale,
        rect.size.width * scale,
        rect.size.height * scale,
    )
}

pub(crate) fn route_is_visible<Message: 'static>(ui: &Ui<Message>, id: ElementId) -> bool {
    ui.route_to(id).is_ok_and(|route| {
        route.into_iter().all(|current| {
            ui.node(current)
                .is_ok_and(|node| node.visibility == Visibility::Visible)
        })
    })
}

pub(crate) fn apply_flex(style: &mut Style, flex: FlexStyle) {
    style.gap = TaffySize {
        width: LengthPercentage::length(flex.column_gap.max(0.0)),
        height: LengthPercentage::length(flex.row_gap.max(0.0)),
    };
    style.align_items = Some(map_alignment(flex.align_items));
    style.align_content = Some(match flex.align_content {
        Alignment::Start => AlignContent::FLEX_START,
        Alignment::Center => AlignContent::CENTER,
        Alignment::End => AlignContent::FLEX_END,
        Alignment::Stretch => AlignContent::STRETCH,
    });
    style.justify_content = Some(match flex.justify_content {
        Justification::Start => JustifyContent::FLEX_START,
        Justification::Center => JustifyContent::CENTER,
        Justification::End => JustifyContent::FLEX_END,
        Justification::SpaceBetween => JustifyContent::SPACE_BETWEEN,
        Justification::SpaceAround => JustifyContent::SPACE_AROUND,
        Justification::SpaceEvenly => JustifyContent::SPACE_EVENLY,
    });
    style.flex_wrap = match flex.wrap {
        FlexWrap::NoWrap => TaffyFlexWrap::NoWrap,
        FlexWrap::Wrap => TaffyFlexWrap::Wrap,
        FlexWrap::WrapReverse => TaffyFlexWrap::WrapReverse,
    };
}

pub(crate) fn snap_slider(value: f32, min: f32, max: f32, step: f32) -> f32 {
    let value = if value.is_finite() { value } else { min };
    (min + ((value.clamp(min, max) - min) / step).round() * step).clamp(min, max)
}

pub(crate) fn previous_grapheme(text: &str, index: usize) -> Option<usize> {
    text.grapheme_indices(true)
        .map(|(index, _)| index)
        .take_while(|candidate| *candidate < index)
        .last()
}

pub(crate) fn next_grapheme(text: &str, index: usize) -> Option<usize> {
    text.grapheme_indices(true)
        .map(|(index, _)| index)
        .find(|candidate| *candidate > index)
        .or_else(|| (index < text.len()).then_some(text.len()))
}

pub(crate) fn platform_error(error: PlatformError) -> UiError {
    UiError::new(format!("platform operation failed: {error}"))
}

pub(crate) fn to_layout_position(field: &TextFieldState, position: TextPosition) -> TextPosition {
    if !field.password {
        return TextPosition {
            byte_index: position.byte_index.min(field.text.len()),
            affinity: position.affinity,
        };
    }
    let graphemes = field.text[..position.byte_index.min(field.text.len())]
        .graphemes(true)
        .count();
    TextPosition {
        byte_index: graphemes * '•'.len_utf8(),
        affinity: position.affinity,
    }
}

pub(crate) fn from_layout_position(field: &TextFieldState, position: TextPosition) -> TextPosition {
    if !field.password {
        let mut index = position.byte_index.min(field.text.len());
        while !field.text.is_char_boundary(index) {
            index -= 1;
        }
        return TextPosition {
            byte_index: index,
            affinity: position.affinity,
        };
    }
    let target = position.byte_index / '•'.len_utf8();
    let byte_index = field
        .text
        .grapheme_indices(true)
        .nth(target)
        .map_or(field.text.len(), |(index, _)| index);
    TextPosition {
        byte_index,
        affinity: position.affinity,
    }
}
