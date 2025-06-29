//! World / Scene related structs

mod ecs;
mod manager;
pub use ecs::*;
pub use manager::*;

pub struct Scene {
    name: String,
    registry: Registry,
}
