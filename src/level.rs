use bevy::prelude::*;
use bevy_ecs_tilemap_plugin::helpers::tiled;
use interest_management::{client::{ClientConnection, Interpolated, Predicted}, protocol::{LevelFileName, Position}};

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            level_spawn,
        );
    }
}

fn level_spawn(
    connection: Res<ClientConnection>,
    mut commands: Commands,
    mut level_query: Query<
        (Entity, &LevelFileName, &Position),
        (Or<(Added<Interpolated>, Added<Predicted>)>),
    >,
    asset_server: Res<AssetServer>,
) {
    for (entity, level_file_name, position) in &mut level_query {
        info!("Spawning level: {:?}, position: {:?}", level_file_name.0, position);

        // Load the Tiled map using the level file name
        let map_handle: Handle<tiled::TiledMap> = asset_server.load(level_file_name.0.clone());

        // Spawn the Tiled map bundle
        commands.entity(entity).insert(tiled::TiledMapBundle {
            tiled_map: map_handle,
            transform: Transform::from_xyz(position.x + 500.0, position.y + 500.0, 500.0),
            ..Default::default()
        });
    }

}