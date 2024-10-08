use interest_management::{client::{ExampleClientPlugin, Predicted}, main as networking, protocol::Position, shared::SharedPlugin};
use bevy_mod_scripting_plugin::console_integration::ScriptPlugin;
use bevy_ecs_tilemap_plugin::tiled::TilesPlugin;
use bevy::prelude::*;
use player::PlayerPlugin; // Add this line to import the App and Plugin traits
use level::LevelPlugin;

pub mod player;
pub mod level;

fn main() {
    let mut apps = networking::plugin_main();
    apps
        .add_user_client_plugins(ScriptPlugin)
        .add_user_client_plugins(TilesPlugin)
        .add_user_client_plugins(PlayerPlugin)
        .add_user_client_plugins(LevelPlugin);
    apps.run();
}