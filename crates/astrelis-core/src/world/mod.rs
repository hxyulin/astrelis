//! World / Scene related structs

mod ecs;
mod manager;
pub use ecs::*;
pub use manager::*;

pub struct Scene {
    pub name: String,
    pub registry: Registry,
}

impl Scene {
    pub fn new(name: String) -> Self {
        Self {
            name,
            registry: Registry::new(),
        }
    }
}
