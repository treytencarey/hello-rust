use bevy::prelude::*;
use bevy_ecs_tilemap_plugin::tiled::TilesPlugin;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: "../assets".to_string(),
            ..default()
        }))
       .add_plugins(TilesPlugin)
       .run();
}