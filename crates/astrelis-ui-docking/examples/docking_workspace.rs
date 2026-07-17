//! Dock-layout serialization, recovery, and mutation walkthrough.

use astrelis_ui_docking::{
    DockAxis, DockLayout, DockNode, DockPlacement, DockSide, DockTabs, FloatingRect,
    PanelDescriptor, PanelId,
};

fn panel(value: &str) -> PanelId {
    PanelId::new(value).expect("example panel identities are valid")
}

fn main() {
    let scene = panel("scene");
    let inspector = panel("inspector");
    let console = panel("console");
    let descriptors = vec![
        PanelDescriptor::new(scene.clone(), "Scene"),
        PanelDescriptor::new(inspector.clone(), "Inspector"),
        PanelDescriptor::new(console.clone(), "Console"),
    ];
    let default = DockLayout {
        root: Some(DockNode::Split {
            axis: DockAxis::Horizontal,
            ratio: 0.7,
            first: Box::new(DockNode::Tabs(
                DockTabs::new(vec![scene.clone()]).expect("non-empty tabs"),
            )),
            second: Box::new(DockNode::Tabs(
                DockTabs::new(vec![inspector.clone()]).expect("non-empty tabs"),
            )),
        }),
        floating: Vec::new(),
    };

    let stale_json = r#"{
        "root":{"kind":"tabs","panels":["removed-panel","scene","scene"],"active":"removed-panel"},
        "floating":[]
    }"#;
    let mut restored: DockLayout =
        serde_json::from_str(stale_json).expect("the example JSON is structurally valid");
    let report = restored.normalize(
        &descriptors,
        &default,
        FloatingRect::new(40.0, 40.0, 360.0, 260.0),
    );
    restored
        .place_panel(
            console,
            DockPlacement::Split {
                anchor: scene,
                side: DockSide::Bottom,
            },
        )
        .expect("the scene anchor survived recovery");

    let saved = serde_json::to_string_pretty(&restored).expect("layout is serializable");
    println!("Recovery report: {report:?}\n{saved}");
}
