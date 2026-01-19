//! Render graph system for automatic resource management and pass scheduling.
//!
//! The render graph provides:
//! - Automatic resource barriers and transitions
//! - Topological sort of render passes based on dependencies
//! - Resource lifetime tracking for optimization
//! - Clear dependency visualization
//!
//! # Example
//!
//! ```ignore
//! use astrelis_render::*;
//!
//! let mut graph = RenderGraph::new();
//!
//! // Add resources
//! let color_target = graph.add_texture(TextureDescriptor {
//!     size: (800, 600, 1),
//!     format: TextureFormat::Rgba8Unorm,
//!     usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
//!     ..Default::default()
//! });
//!
//! // Add passes
//! graph.add_pass(RenderGraphPass {
//!     name: "main_pass",
//!     inputs: vec![],
//!     outputs: vec![color_target],
//!     execute: Box::new(|ctx| {
//!         // Render code here
//!     }),
//! });
//!
//! // Compile and execute
//! let plan = graph.compile()?;
//! graph.execute(&context);
//! ```

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::GraphicsContext;

/// Resource identifier in the render graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceId(u64);

impl ResourceId {
    /// Create a new resource ID.
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get the raw ID value.
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Pass identifier in the render graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassId(u64);

impl PassId {
    /// Create a new pass ID.
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get the raw ID value.
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Resource type in the render graph.
#[derive(Debug, Clone, PartialEq)]
pub enum ResourceType {
    /// Texture resource
    Texture {
        size: (u32, u32, u32),
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    },
    /// Buffer resource
    Buffer {
        size: u64,
        usage: wgpu::BufferUsages,
    },
}

/// Resource information in the render graph.
#[derive(Debug, Clone)]
pub struct ResourceInfo {
    /// Resource ID
    pub id: ResourceId,
    /// Resource type and descriptor
    pub resource_type: ResourceType,
    /// Resource name for debugging
    pub name: String,
    /// First pass that reads this resource
    pub first_read: Option<PassId>,
    /// Last pass that writes this resource
    pub last_write: Option<PassId>,
    /// Last pass that reads this resource
    pub last_read: Option<PassId>,
}

/// Render context passed to pass execution functions.
pub struct RenderContext {
    /// Graphics context
    pub graphics: Arc<GraphicsContext>,
    /// Resource textures (if created)
    pub textures: HashMap<ResourceId, wgpu::Texture>,
    /// Resource buffers (if created)
    pub buffers: HashMap<ResourceId, wgpu::Buffer>,
}

impl RenderContext {
    /// Create a new render context.
    pub fn new(graphics: Arc<GraphicsContext>) -> Self {
        Self {
            graphics,
            textures: HashMap::new(),
            buffers: HashMap::new(),
        }
    }

    /// Get a texture by resource ID.
    pub fn get_texture(&self, id: ResourceId) -> Option<&wgpu::Texture> {
        self.textures.get(&id)
    }

    /// Get a buffer by resource ID.
    pub fn get_buffer(&self, id: ResourceId) -> Option<&wgpu::Buffer> {
        self.buffers.get(&id)
    }
}

/// A render pass in the graph.
pub struct RenderGraphPass {
    /// Pass name for debugging
    pub name: &'static str,
    /// Input resources (read)
    pub inputs: Vec<ResourceId>,
    /// Output resources (write)
    pub outputs: Vec<ResourceId>,
    /// Execution function
    pub execute: Box<dyn Fn(&mut RenderContext) + Send + Sync>,
}

impl RenderGraphPass {
    /// Create a new render pass.
    pub fn new(
        name: &'static str,
        inputs: Vec<ResourceId>,
        outputs: Vec<ResourceId>,
        execute: impl Fn(&mut RenderContext) + Send + Sync + 'static,
    ) -> Self {
        Self {
            name,
            inputs,
            outputs,
            execute: Box::new(execute),
        }
    }
}

/// Execution plan for the render graph.
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    /// Ordered list of pass IDs to execute
    pub pass_order: Vec<PassId>,
}

/// Render graph error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderGraphError {
    /// Cyclic dependency detected
    CyclicDependency,
    /// Resource not found
    ResourceNotFound(ResourceId),
    /// Pass not found
    PassNotFound(PassId),
    /// Invalid resource usage
    InvalidUsage(String),
}

impl std::fmt::Display for RenderGraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CyclicDependency => write!(f, "Cyclic dependency detected in render graph"),
            Self::ResourceNotFound(id) => write!(f, "Resource {:?} not found", id),
            Self::PassNotFound(id) => write!(f, "Pass {:?} not found", id),
            Self::InvalidUsage(msg) => write!(f, "Invalid resource usage: {}", msg),
        }
    }
}

impl std::error::Error for RenderGraphError {}

/// Render graph managing passes and resources.
pub struct RenderGraph {
    /// All render passes
    passes: HashMap<PassId, RenderGraphPass>,
    /// All resources
    resources: HashMap<ResourceId, ResourceInfo>,
    /// Next pass ID
    next_pass_id: u64,
    /// Next resource ID
    next_resource_id: u64,
    /// Execution plan (cached after compilation)
    execution_plan: Option<ExecutionPlan>,
}

impl RenderGraph {
    /// Create a new render graph.
    pub fn new() -> Self {
        Self {
            passes: HashMap::new(),
            resources: HashMap::new(),
            next_pass_id: 0,
            next_resource_id: 0,
            execution_plan: None,
        }
    }

    /// Add a texture resource to the graph.
    pub fn add_texture(
        &mut self,
        name: impl Into<String>,
        size: (u32, u32, u32),
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> ResourceId {
        let id = ResourceId::new(self.next_resource_id);
        self.next_resource_id += 1;

        let resource = ResourceInfo {
            id,
            resource_type: ResourceType::Texture {
                size,
                format,
                usage,
            },
            name: name.into(),
            first_read: None,
            last_write: None,
            last_read: None,
        };

        self.resources.insert(id, resource);
        self.execution_plan = None; // Invalidate plan

        id
    }

    /// Add a buffer resource to the graph.
    pub fn add_buffer(
        &mut self,
        name: impl Into<String>,
        size: u64,
        usage: wgpu::BufferUsages,
    ) -> ResourceId {
        let id = ResourceId::new(self.next_resource_id);
        self.next_resource_id += 1;

        let resource = ResourceInfo {
            id,
            resource_type: ResourceType::Buffer { size, usage },
            name: name.into(),
            first_read: None,
            last_write: None,
            last_read: None,
        };

        self.resources.insert(id, resource);
        self.execution_plan = None; // Invalidate plan

        id
    }

    /// Add a render pass to the graph.
    pub fn add_pass(&mut self, pass: RenderGraphPass) -> PassId {
        let id = PassId::new(self.next_pass_id);
        self.next_pass_id += 1;

        // Update resource usage tracking
        for &input_id in &pass.inputs {
            if let Some(resource) = self.resources.get_mut(&input_id) {
                if resource.first_read.is_none() {
                    resource.first_read = Some(id);
                }
                resource.last_read = Some(id);
            }
        }

        for &output_id in &pass.outputs {
            if let Some(resource) = self.resources.get_mut(&output_id) {
                resource.last_write = Some(id);
            }
        }

        self.passes.insert(id, pass);
        self.execution_plan = None; // Invalidate plan

        id
    }

    /// Compile the render graph into an execution plan.
    ///
    /// This performs topological sorting of passes based on their dependencies.
    pub fn compile(&mut self) -> Result<ExecutionPlan, RenderGraphError> {
        // Build dependency graph
        let mut dependencies: HashMap<PassId, HashSet<PassId>> = HashMap::new();
        let mut dependents: HashMap<PassId, HashSet<PassId>> = HashMap::new();

        for (&pass_id, pass) in &self.passes {
            dependencies.insert(pass_id, HashSet::new());
            dependents.entry(pass_id).or_insert_with(HashSet::new);

            // A pass depends on any pass that writes to its input resources
            for &input_id in &pass.inputs {
                // Find passes that write to this resource
                for (&other_pass_id, other_pass) in &self.passes {
                    if other_pass_id != pass_id && other_pass.outputs.contains(&input_id) {
                        dependencies.get_mut(&pass_id).unwrap().insert(other_pass_id);
                        dependents.entry(other_pass_id).or_insert_with(HashSet::new).insert(pass_id);
                    }
                }
            }
        }

        // Topological sort using Kahn's algorithm
        let mut sorted = Vec::new();
        let mut no_incoming: Vec<PassId> = dependencies
            .iter()
            .filter(|(_, deps)| deps.is_empty())
            .map(|(&id, _)| id)
            .collect();

        while let Some(pass_id) = no_incoming.pop() {
            sorted.push(pass_id);

            // Remove edges from this pass to its dependents
            if let Some(deps) = dependents.get(&pass_id) {
                for &dependent_id in deps {
                    if let Some(dep_set) = dependencies.get_mut(&dependent_id) {
                        dep_set.remove(&pass_id);
                        if dep_set.is_empty() {
                            no_incoming.push(dependent_id);
                        }
                    }
                }
            }
        }

        // Check for cycles
        if sorted.len() != self.passes.len() {
            return Err(RenderGraphError::CyclicDependency);
        }

        let plan = ExecutionPlan {
            pass_order: sorted,
        };

        self.execution_plan = Some(plan.clone());

        Ok(plan)
    }

    /// Execute the render graph.
    ///
    /// This must be called after `compile()`.
    pub fn execute(&self, graphics: Arc<GraphicsContext>) -> Result<(), RenderGraphError> {
        let plan = self
            .execution_plan
            .as_ref()
            .ok_or(RenderGraphError::InvalidUsage(
                "Graph not compiled".to_string(),
            ))?;

        let mut context = RenderContext::new(graphics);

        // Create resources (simplified - in reality would manage lifetimes)
        for (id, info) in &self.resources {
            match &info.resource_type {
                ResourceType::Texture {
                    size,
                    format,
                    usage,
                } => {
                    let texture = context.graphics.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some(&info.name),
                        size: wgpu::Extent3d {
                            width: size.0,
                            height: size.1,
                            depth_or_array_layers: size.2,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: *format,
                        usage: *usage,
                        view_formats: &[],
                    });
                    context.textures.insert(*id, texture);
                }
                ResourceType::Buffer { size, usage } => {
                    let buffer = context.graphics.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some(&info.name),
                        size: *size,
                        usage: *usage,
                        mapped_at_creation: false,
                    });
                    context.buffers.insert(*id, buffer);
                }
            }
        }

        // Execute passes in order
        for &pass_id in &plan.pass_order {
            if let Some(pass) = self.passes.get(&pass_id) {
                (pass.execute)(&mut context);
            }
        }

        Ok(())
    }

    /// Get the number of passes in the graph.
    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }

    /// Get the number of resources in the graph.
    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }

    /// Check if the graph has been compiled.
    pub fn is_compiled(&self) -> bool {
        self.execution_plan.is_some()
    }
}

impl Default for RenderGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_graph_new() {
        let graph = RenderGraph::new();
        assert_eq!(graph.pass_count(), 0);
        assert_eq!(graph.resource_count(), 0);
        assert!(!graph.is_compiled());
    }

    #[test]
    fn test_add_texture_resource() {
        let mut graph = RenderGraph::new();
        let tex = graph.add_texture(
            "color_target",
            (800, 600, 1),
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        assert_eq!(graph.resource_count(), 1);
        assert_eq!(tex.as_u64(), 0);
    }

    #[test]
    fn test_add_buffer_resource() {
        let mut graph = RenderGraph::new();
        let buf = graph.add_buffer(
            "vertex_buffer",
            1024,
            wgpu::BufferUsages::VERTEX,
        );
        assert_eq!(graph.resource_count(), 1);
        assert_eq!(buf.as_u64(), 0);
    }

    #[test]
    fn test_add_pass() {
        let mut graph = RenderGraph::new();
        let tex = graph.add_texture(
            "target",
            (800, 600, 1),
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );

        let pass = RenderGraphPass::new("test_pass", vec![], vec![tex], |_ctx| {});
        let pass_id = graph.add_pass(pass);

        assert_eq!(graph.pass_count(), 1);
        assert_eq!(pass_id.as_u64(), 0);
    }

    #[test]
    fn test_compile_simple() {
        let mut graph = RenderGraph::new();
        let tex = graph.add_texture(
            "target",
            (800, 600, 1),
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );

        let pass = RenderGraphPass::new("test_pass", vec![], vec![tex], |_ctx| {});
        graph.add_pass(pass);

        let result = graph.compile();
        assert!(result.is_ok());
        assert!(graph.is_compiled());
    }

    #[test]
    fn test_compile_multiple_passes() {
        let mut graph = RenderGraph::new();
        let tex1 = graph.add_texture(
            "tex1",
            (800, 600, 1),
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let tex2 = graph.add_texture(
            "tex2",
            (800, 600, 1),
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );

        // Pass 1 writes to tex1
        let pass1 = RenderGraphPass::new("pass1", vec![], vec![tex1], |_ctx| {});
        graph.add_pass(pass1);

        // Pass 2 reads tex1 and writes to tex2
        let pass2 = RenderGraphPass::new("pass2", vec![tex1], vec![tex2], |_ctx| {});
        graph.add_pass(pass2);

        let result = graph.compile();
        assert!(result.is_ok());

        let plan = result.unwrap();
        assert_eq!(plan.pass_order.len(), 2);
        // Pass 1 should come before pass 2
        assert!(plan.pass_order[0].as_u64() < plan.pass_order[1].as_u64());
    }

    #[test]
    fn test_resource_id_equality() {
        let id1 = ResourceId::new(1);
        let id2 = ResourceId::new(1);
        let id3 = ResourceId::new(2);
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_pass_id_equality() {
        let id1 = PassId::new(1);
        let id2 = PassId::new(1);
        let id3 = PassId::new(2);
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_error_display() {
        let err = RenderGraphError::CyclicDependency;
        assert!(format!("{}", err).contains("Cyclic"));

        let err = RenderGraphError::ResourceNotFound(ResourceId::new(42));
        assert!(format!("{}", err).contains("Resource"));
    }
}
