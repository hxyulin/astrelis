//! Builds and prints a display list without initializing a platform or GPU.

use astrelis_core::{
    color::Color,
    geometry::{Point, Rect},
};
use astrelis_paint::{Brush, FillRule, Painter, Path, StrokeStyle};

fn main() {
    let mut path = Path::builder();
    path.move_to(Point::new(20.0, 10.0)).unwrap();
    path.line_to(Point::new(40.0, 50.0)).unwrap();
    path.line_to(Point::new(0.0, 50.0)).unwrap();
    path.close().unwrap();
    let path = path.finish();

    let mut painter = Painter::new();
    painter
        .fill_rect(
            Rect::from_xywh(0.0, 0.0, 80.0, 60.0),
            Brush::Solid(Color::BLACK),
        )
        .unwrap();
    painter
        .fill_path(&path, FillRule::NonZero, Brush::Solid(Color::CYAN))
        .unwrap();
    painter
        .stroke_path(
            &path,
            StrokeStyle {
                width: 2.0,
                ..Default::default()
            },
            Brush::Solid(Color::WHITE),
        )
        .unwrap();

    println!("{:#?}", painter.finish().unwrap());
}
