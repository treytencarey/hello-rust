use avian2d::{prelude::{Gravity, Physics, PhysicsSet}, sync::SyncPlugin, PhysicsPlugins};
use bevy::{app::{App, FixedUpdate, Plugin, PostUpdate}, math::Vec2, prelude::IntoSystemSetConfigs, time::Time};
use bevy::prelude::PluginGroup;
use interest_management::main as networking;
use bevy_mod_scripting_plugin::console_integration::ScriptPlugin;
use bevy_ecs_tilemap_plugin::tiled::TilesPlugin;
use player::{FixedSet, PlayerClientPlugin, PlayerServerPlugin, PlayerSharedPlugin};
use level::{LevelClientPlugin, LevelServerPlugin, LevelSharedPlugin};
use remote_file::{RemoteFileClientPlugin, RemoteFileServerPlugin, RemoteFileSharedPlugin};
use script::{ScriptClientPlugin, ScriptServerPlugin, ScriptSharedPlugin};

pub const FIXED_TIMESTEP_HZ: f64 = 64.0;

#[derive(Clone)]
pub struct SharedPlugin {
    pub(crate) show_confirmed: bool,
}

impl Plugin for SharedPlugin {
    fn build(&self, app: &mut App) {
        // Physics
        //
        // we use Position and Rotation as primary source of truth, so no need to sync changes
        // from Transform->Pos, just Pos->Transform.
        app.insert_resource(avian2d::sync::SyncConfig {
            transform_to_position: false,
            position_to_transform: true,
        });
        // We change SyncPlugin to PostUpdate, because we want the visually interpreted values
        // synced to transform every time, not just when Fixed schedule runs.
        app.add_plugins(
            PhysicsPlugins::new(FixedUpdate)
                .build()
                .disable::<SyncPlugin>(),
        )
        .add_plugins(SyncPlugin::new(PostUpdate));

        app.insert_resource(Time::new_with(Physics::fixed_once_hz(FIXED_TIMESTEP_HZ)));
        app.insert_resource(Gravity(Vec2::ZERO));

        app.configure_sets(
            FixedUpdate,
            (
                // make sure that any physics simulation happens after the Main SystemSet
                // (where we apply user's actions)
                (
                    PhysicsSet::Prepare,
                    PhysicsSet::StepSimulation,
                    PhysicsSet::Sync,
                )
                    .in_set(FixedSet::Physics),
                (FixedSet::Main, FixedSet::Physics).chain(),
            ),
        );
    }
}

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
        .add_user_shared_plugins(SharedPlugin {
            show_confirmed: true,
        })
        .add_user_plugins(RemoteFileClientPlugin, RemoteFileServerPlugin, RemoteFileSharedPlugin)
        .add_user_plugins(PlayerClientPlugin, PlayerServerPlugin, PlayerSharedPlugin)
        .add_user_plugins(LevelClientPlugin, LevelServerPlugin, LevelSharedPlugin)
        .add_user_plugins(ScriptClientPlugin, ScriptServerPlugin, ScriptSharedPlugin);
    apps.run();
}