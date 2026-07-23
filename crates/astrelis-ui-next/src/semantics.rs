//! Incremental accessibility data.

use astrelis_core::geometry::LogicalRect;

use crate::NodeId;

/// Small semantic role set used by the research prototype.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SemanticRole {
    /// Structural group.
    #[default]
    Group,
    /// Static text.
    Label,
    /// Activatable button.
    Button,
    /// Hierarchical collection.
    Tree,
    /// Tabular collection.
    Table,
    /// One collection row.
    Row,
    /// Editable property.
    Field,
    /// Application-rendered viewport.
    RenderView,
}

/// Element-local accessible properties.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SemanticData {
    /// Semantic role.
    pub role: SemanticRole,
    /// Accessible name.
    pub label: String,
    /// Optional accessible value.
    pub value: Option<String>,
    /// Whether the node is selected.
    pub selected: Option<bool>,
    /// Whether the node is expanded.
    pub expanded: Option<bool>,
}

/// One complete accessible node in window coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticNode {
    /// Stable retained identity.
    pub id: NodeId,
    /// Parent identity.
    pub parent: Option<NodeId>,
    /// Window-space bounds.
    pub bounds: LogicalRect,
    /// Accessible properties.
    pub data: SemanticData,
    /// Whether the element accepts keyboard focus.
    pub focusable: bool,
    /// Whether the element currently has focus.
    pub focused: bool,
}

/// Accessibility changes since the preceding update.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AccessibilityUpdate {
    /// Inserted or changed nodes.
    pub changed: Vec<SemanticNode>,
    /// Removed retained identities.
    pub removed: Vec<NodeId>,
}
