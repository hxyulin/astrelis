# astrelis-ui-docking

Serializable editor/workspace docking policy built above `astrelis-ui-core`
and `astrelis-ui-widgets`.

The crate keeps application panel contents in stable retained hosts while its
own tabs, split panes, drop targets, and in-window floating chrome are rebuilt
as the layout changes. `DockLayout` contains only stable `PanelId` values,
split ratios, active tabs, tab order, and floating geometry, and derives
`serde::Serialize` and `serde::Deserialize` so applications can choose their
own persistence format.

Applications drain mapped `DockAction` messages from `Ui`, pass them to
`DockWorkspace::apply`, and persist `DockWorkspace::layout` when the returned
`DockOutcome` reports a change. Register all panels before calling `restore` so
unknown, duplicate, and required-missing panels can be recovered
deterministically.

Run the native end-to-end workspace with:

```text
cargo run -p astrelis-ui-docking --example docking_workspace_native
```

The toolbar can save and reload the current layout in memory, restore the
default layout, load an intentionally stale layout, reopen closed panels, and
float the Assets panel. Edit the Scene or Inspector fields before moving their
tabs to verify that app-owned retained state survives docking reconciliation.
Drag tabs onto tab headers to reorder them, into the center or edges of panel
groups to tab or split them, or onto the narrow outer workspace border to make
them float. Floating surfaces can be moved from their title control and resized
from all edges and corners.

Run the model-only serialization and recovery walkthrough with:

```text
cargo run -p astrelis-ui-docking --example docking_workspace
```

The model walkthrough can also be checked for the browser target with:

```text
cargo check -p astrelis-ui-docking --target wasm32-unknown-unknown \
  --example docking_workspace
```
