use bytemuck::offset_of;
use glam::{Vec2, Vec3};
use wgpu::util::DeviceExt;

use crate::{
    alloc::{IndexSlot, SparseSet},
    world::Component,
};

#[repr(C)]
#[derive(Debug, Clone, Default, Copy, bytemuck::NoUninit, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: Vec3,
    pub texcoord: Vec2,
}

impl Vertex {
    pub fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        // NIGHTLY: we are using the compile_time offset_of feature
        assert_eq!(offset_of!(Vertex, pos), 0);
        assert_eq!(offset_of!(Vertex, texcoord), size_of::<Vec3>());

        static ATTRIBUTES: &[wgpu::VertexAttribute] = &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: size_of::<Vec3>() as u64,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x2,
            },
        ];
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: ATTRIBUTES,
        }
    }
}

pub enum MeshSource {
    Memory(Vec<Vertex>, Vec<u32>),
}

impl MeshSource {
    pub fn vertex(&self) -> &[Vertex] {
        // TODO: We might want to make this a Cow
        match self {
            Self::Memory(vertex, _) => vertex.as_slice(),
        }
    }

    pub fn index(&self) -> &[u32] {
        match self {
            Self::Memory(_, index) => index.as_slice(),
        }
    }
}

pub struct Mesh {
    name: String,
    source: MeshSource,
    gpu_mesh: Option<GpuMesh>,
}

impl Mesh {
    pub fn new(name: String, source: MeshSource) -> Self {
        Self {
            name,
            source,
            gpu_mesh: None,
        }
    }

    pub fn get_or_create_gpumesh<F>(&mut self, init_fn: F) -> &GpuMesh
    where
        F: FnOnce(&Mesh) -> GpuMesh,
    {
        if self.gpu_mesh.is_none() {
            self.gpu_mesh = Some(init_fn(&self));
        }
        self.gpu_mesh.as_ref().unwrap()
    }
}

pub struct GpuMesh {
    pub vertex: wgpu::Buffer,
    pub index: wgpu::Buffer,
    pub vertex_count: usize,
}

impl GpuMesh {
    pub fn from_mesh(mesh: &Mesh, device: &wgpu::Device) -> Self {
        Self::new(&mesh.source, device, mesh.name.as_str())
    }

    pub fn new(source: &MeshSource, device: &wgpu::Device, name: &str) -> Self {
        let vertices = source.vertex();
        assert!(!vertices.is_empty(), "meshes cannot have no vertices");
        let vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{}_vertex", name)),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indices = source.index();
        assert!(!indices.is_empty(), "meshes cannot have no indices");
        let index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{}_index", name)),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex,
            index,
            vertex_count: indices.len(),
        }
    }
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

    pub fn get_mesh_mut(&mut self, handle: MeshHandle) -> &mut Mesh {
        self.meshes.get_mut(handle.0)
    }

    pub fn remove_mesh(&mut self, handle: MeshHandle) -> Mesh {
        self.meshes.remove(handle.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MeshComponent(pub MeshHandle);

impl Component for MeshComponent {}
