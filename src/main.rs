use interest_management::main as networking;
use bevy_mod_scripting_plugin::console_integration::ScriptPlugin;
use bevy_ecs_tilemap_plugin::tiled::TilesPlugin;
use player::PlayerPlugin;
use level::{LevelClientPlugin, LevelServerPlugin, LevelSharedPlugin};

pub mod player;
pub mod level;

fn main() {
    println!("Running in directory: {}", std::env::current_dir().unwrap().display());

    let mut apps = networking::plugin_main();
    apps
        .add_user_client_plugins(ScriptPlugin)
        .add_user_client_plugins(TilesPlugin)
        .add_user_client_plugins(PlayerPlugin)
        .add_user_plugins(LevelClientPlugin, LevelServerPlugin, LevelSharedPlugin);
    apps.run();
}