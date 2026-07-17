//! Serializable docking-workspace policy built on public Astrelis UI APIs.

#![warn(missing_docs)]

mod model;
mod workspace;

pub use model::{
    DockAxis, DockError, DockLayout, DockNode, DockPlacement, DockSide, DockTabs, FloatingGroup,
    FloatingRect, NormalizationReport, PanelDescriptor, PanelId, PreferredPlacement,
};
pub use workspace::{
    DockAction, DockFloatFrame, DockGroup, DockOutcome, DockStyle, DockTab, DockWorkspace,
    DockWorkspaceSurface, SplitBranch,
};
