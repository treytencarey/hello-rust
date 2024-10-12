use interest_management::{client::{ExampleClientPlugin, Predicted}, main as networking, protocol::Position, shared::SharedPlugin};
use bevy_mod_scripting_plugin::console_integration::ScriptPlugin;
use bevy_ecs_tilemap_plugin::tiled::TilesPlugin;
use bevy::prelude::*;
use level::{LevelClientPlugin, LevelServerPlugin, LevelSharedPlugin};

pub mod level;

fn main() {
    let mut apps = networking::plugin_main();
    apps
        .add_user_client_plugins(ScriptPlugin)
        .add_user_client_plugins(TilesPlugin)
        .add_user_plugins(LevelClientPlugin, LevelServerPlugin, LevelSharedPlugin);
    apps.run();
}