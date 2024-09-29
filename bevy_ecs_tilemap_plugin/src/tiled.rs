use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

use crate::helpers::tiled;

fn startup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let map_handle: Handle<tiled::TiledMap> = asset_server.load("map_1.tmx");

    commands.spawn(tiled::TiledMapBundle {
        tiled_map: map_handle,
        ..Default::default()
    });
}

pub struct TilesPlugin;

impl Plugin for TilesPlugin {
    fn build(&self, app: &mut App) {
        // Add your network systems, resources, etc. here
        app.add_plugins(TilemapPlugin)
            .add_plugins(tiled::TiledMapPlugin)
            .add_systems(Startup, startup);
    }
}