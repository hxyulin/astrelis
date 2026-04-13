//! Asset server plugin.

use astrelis_assets::AssetServer;

use crate::phase::Phase;
use crate::plugin::Plugin;

/// Plugin that provides the asset loading system.
///
/// Inserts an [`AssetServer`] resource and calls `update()` each
/// frame to process completed loads.
pub struct AssetPlugin {
    /// Root directory for asset files.
    pub asset_dir: String,
}

impl Default for AssetPlugin {
    fn default() -> Self {
        Self {
            asset_dir: "assets".to_string(),
        }
    }
}

impl Plugin for AssetPlugin {
    fn build(&self, app: &mut crate::app::App) {
        app.insert_resource(AssetServer::new(&self.asset_dir));

        app.add_system(Phase::PreUpdate, |resources| {
            let server = resources.get::<AssetServer>();
            let _events = server.update();
        });
    }
}
