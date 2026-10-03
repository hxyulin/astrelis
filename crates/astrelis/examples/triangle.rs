//! A triangle with interpolated vertex colors. Run with `--smoke` to auto-exit.

mod support;

use astrelis::{Error, GraphicsContext, Mesh, Vertex};

fn meshes(graphics: &GraphicsContext) -> Result<Vec<Mesh>, Error> {
    Ok(vec![Mesh::new(
        graphics,
        &[
            Vertex::new([0.0, 0.75, 0.0], [1.0, 0.15, 0.15, 1.0]),
            Vertex::new([-0.75, -0.65, 0.0], [0.15, 1.0, 0.15, 1.0]),
            Vertex::new([0.75, -0.65, 0.0], [0.15, 0.35, 1.0, 1.0]),
        ],
        &[0, 1, 2],
    )?])
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    support::run(&["Astrelis — indexed triangle"], meshes)
}
