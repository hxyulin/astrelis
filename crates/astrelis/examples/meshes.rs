//! Two persistent meshes demonstrating submission order and alpha blending.

mod support;

use astrelis::{Error, GraphicsContext, Mesh, Vertex};

fn quad(graphics: &GraphicsContext, bounds: [f32; 4], color: [f32; 4]) -> Result<Mesh, Error> {
    let [left, bottom, right, top] = bounds;
    Mesh::new(
        graphics,
        &[
            Vertex::new([left, bottom, 0.0], color),
            Vertex::new([right, bottom, 0.0], color),
            Vertex::new([right, top, 0.0], color),
            Vertex::new([left, top, 0.0], color),
        ],
        &[0, 1, 2, 0, 2, 3],
    )
}

fn meshes(graphics: &GraphicsContext) -> Result<Vec<Mesh>, Error> {
    Ok(vec![
        quad(graphics, [-0.8, -0.65, 0.3, 0.7], [1.0, 0.18, 0.08, 1.0])?,
        quad(graphics, [-0.3, -0.7, 0.8, 0.65], [0.05, 0.35, 1.0, 0.65])?,
    ])
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    support::run(&["Astrelis — ordered meshes"], meshes)
}
