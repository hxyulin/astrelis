use std::sync::atomic::AtomicBool;

use egui::Vec2;
use glam::Vec3;

use crate::{alloc::{IndexSlot, SparseSet}, world::Component};

pub struct Vertex {
    pub pos: Vec3,
    pub texcoord: Vec2,
}

pub enum MeshSource {
    Memory(Vec<Vertex>, Vec<u32>),
}

pub struct Mesh {
    source: MeshSource,
    gpu_mesh: Option<GpuMesh>,
}

impl Mesh {
    pub fn new(source: MeshSource) -> Self {
        Self {
            source,
            gpu_mesh: None,
        }
    }
}

pub struct GpuMesh {
    pub vertex: wgpu::Buffer,
    pub index: wgpu::Buffer,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct MeshHandle(IndexSlot);

// TODO: Refcount
pub struct MeshManager {
    meshes: SparseSet<Mesh>,
}

impl MeshManager {
    pub fn new() -> Self {
        Self {
            meshes: SparseSet::new(),
        }
    }

    pub fn create_mesh(&mut self, mesh: Mesh) -> MeshHandle {
        MeshHandle(self.meshes.push(mesh))
    }

    pub fn get_mesh(&self, handle: MeshHandle) -> &Mesh {
        self.meshes.get(handle.0)
    }

    pub fn remove_mesh(&mut self, handle: MeshHandle) -> Mesh {
        self.meshes.remove(handle.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MeshComponent(pub MeshHandle);

impl Component for MeshComponent {}
