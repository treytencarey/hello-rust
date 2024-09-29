use bevy::prelude::*;
use bevy_mod_scripting_plugin::console_integration::ScriptPlugin;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: "../assets".to_string(),
            ..default()
        }))
       .add_plugins(ScriptPlugin)
       .run();
}