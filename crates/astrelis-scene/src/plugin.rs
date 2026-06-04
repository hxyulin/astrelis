//! App integration.

use astrelis_app::{App, Phase, Plugin};

use crate::scene::Scene;

/// Registers a [`Scene`] resource and the per-frame propagation pass.
///
/// The pass runs in [`Phase::PostUpdate`]: mutate the scene in
/// `Update` systems, read world transforms/visibility in `Render`
/// systems. For mid-frame freshness call
/// [`Scene::flush_transforms`] directly.
pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Scene::new());
        app.add_system(Phase::PostUpdate, |resources| {
            let mut scene = resources.get_mut::<Scene>();
            scene.flush_transforms();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::scene::Scene;

    #[test]
    fn plugin_inserts_scene_resource() {
        let mut app = astrelis_app::App::new();
        ScenePlugin.build(&mut app);
        // The scene resource must exist and be usable immediately.
        let mut scene = app.resources().get_mut::<Scene>();
        let id = scene.spawn().id();
        assert!(scene.contains(id));
    }
}
