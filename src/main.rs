use interest_management::main as networking;
use bevy_mod_scripting_plugin::console_integration::ScriptPlugin;
use bevy_ecs_tilemap_plugin::tiled::TilesPlugin;
use player::{PlayerClientPlugin, PlayerServerPlugin, PlayerSharedPlugin};
use level::{LevelClientPlugin, LevelServerPlugin, LevelSharedPlugin};
use remote_file::{RemoteFileClientPlugin, RemoteFileServerPlugin, RemoteFileSharedPlugin};
use script::{ScriptClientPlugin, ScriptServerPlugin, ScriptSharedPlugin};

pub mod player;
pub mod remote_file;
pub mod level;
pub mod script;
fn main() {
    println!("Running in directory: {}", std::env::current_dir().unwrap().display());

    let mut apps = networking::plugin_main();
    apps
        .add_user_client_plugins(ScriptPlugin)
        .add_user_client_plugins(TilesPlugin)
        .add_user_plugins(RemoteFileClientPlugin, RemoteFileServerPlugin, RemoteFileSharedPlugin)
        .add_user_plugins(PlayerClientPlugin, PlayerServerPlugin, PlayerSharedPlugin)
        .add_user_plugins(LevelClientPlugin, LevelServerPlugin, LevelSharedPlugin)
        .add_user_plugins(ScriptClientPlugin, ScriptServerPlugin, ScriptSharedPlugin);
    apps.run();
}