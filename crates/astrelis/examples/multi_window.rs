//! Two independently sized surfaces sharing one context, renderer, and mesh.

mod support;

use astrelis::{Error, GraphicsContext, Mesh, Vertex};

fn meshes(graphics: &GraphicsContext) -> Result<Vec<Mesh>, Error> {
    Ok(vec![Mesh::new(
        graphics,
        &[
            Vertex::new([0.0, 0.7, 0.0], [1.0, 0.0, 0.0, 1.0]),
            Vertex::new([-0.7, -0.7, 0.0], [0.0, 1.0, 0.0, 1.0]),
            Vertex::new([0.7, -0.7, 0.0], [0.0, 0.0, 1.0, 1.0]),
        ],
        &[0, 1, 2],
    )?])
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    support::run(
        &[
            "Astrelis — shared GPU, first window",
            "Astrelis — shared GPU, second window",
        ],
        meshes,
    )
}
