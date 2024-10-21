use bevy::prelude::*;
use bevy_ecs_tilemap_plugin::helpers::tiled;
use interest_management::{client::{ComponentSyncMode, ConnectionManager, Interpolated, Predicted}, server::{get_grid_position, get_room_id_from_grid_position, GRID_SIZE}, shared::{LastPosition, Position}};
use lightyear::{prelude::{server::{Replicate, RoomManager, SyncTarget}, AppComponentExt, ChannelDirection, NetworkRelevanceMode, ReplicationGroup}, shared::replication::network_target::NetworkTarget};
use serde::{Deserialize, Serialize};

use crate::remote_file::{remotefile_modified, RemoteFileBundle, RemoteFileParent};

// Level
#[derive(Bundle)]
pub(crate) struct LevelBundle {
    position: Position,
    last_position: LastPosition,
    replicate: Replicate,
    filename: LevelFileName,
}

impl LevelBundle {
    pub(crate) fn new(position: Vec2, filename: String) -> Self {
        let sync_target = SyncTarget {
            prediction: NetworkTarget::All,
            ..default()
        };
        let replicate = Replicate {
            sync: sync_target,
            relevance_mode: NetworkRelevanceMode::InterestManagement,
            group: ReplicationGroup::default(),
            ..default()
        };
        Self {
            replicate,
            position: Position(position),
            last_position: LastPosition(None),
            filename: LevelFileName(filename)
        }
    }
}

// 
#[derive(Default, Component, Serialize, Deserialize, Debug, PartialEq, Clone, Reflect)]
pub struct LevelFileName(pub String);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LevelFileHash {
    hash: String,
    file_name: LevelFileName,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LevelFile {
    data: Vec<u8>,
    file_name: LevelFileName,
}

impl Default for LevelFile {
    fn default() -> Self {
        LevelFile {
            data: Vec::new(),
            file_name: LevelFileName::default(),
        }
    }
}

// ################################################################################################

#[derive(Clone)]
pub struct LevelSharedPlugin;

impl Plugin for LevelSharedPlugin {
    fn build(&self, app: &mut App) {
        app.register_component::<LevelFileName>(ChannelDirection::ServerToClient)
            .add_prediction(ComponentSyncMode::Once)
            .add_interpolation(ComponentSyncMode::Once);

        app.register_component::<RemoteFileParent>(ChannelDirection::ServerToClient)
            .add_map_entities()
            .add_prediction(ComponentSyncMode::Once)
            .add_interpolation(ComponentSyncMode::Once);
    }
}

// ################################################################################################

pub struct LevelServerPlugin;

impl Plugin for LevelServerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConnectionManager>();
        app.add_systems(Startup, init);
    }
}

pub(crate) fn init(mut commands: Commands, mut room_manager: ResMut<RoomManager>) {
    const NUM_LEVELS: i32 = 3;
    for x in -NUM_LEVELS..=NUM_LEVELS {
        for y in -NUM_LEVELS..=NUM_LEVELS {
            let position = Vec2::new((x * GRID_SIZE) as f32, (y * GRID_SIZE) as f32);
            let room_id = get_room_id_from_grid_position(get_grid_position(position));
            let filename = format!("map_{}.tmx", room_id.0);

            let level_entity = commands.spawn(
                LevelBundle::new(position, filename.clone())
            ).id();
            let remote_file_entity = commands.spawn(
                RemoteFileBundle::new(filename.clone(), level_entity)
            ).id();

            info!("Level spawned, added to room: {:?} {:?}", room_id.0, position);
            room_manager.add_entity(level_entity, room_id);
            room_manager.add_entity(remote_file_entity, room_id);
        }
    }
}

// ################################################################################################

pub struct LevelClientPlugin;

impl Plugin for LevelClientPlugin {
    fn build(&self, app: &mut App) {
        app
        .add_systems(
            Update,
        (level_spawn, remotefile_modified::<tiled::TiledMap>)
        );
    }
}

fn level_spawn(
    mut commands: Commands,
    mut level_query: Query<
        (Entity, &LevelFileName, &Position),
        Or<(Added<Interpolated>, Added<Predicted>)>,
    >,
    asset_server: Res<AssetServer>,
) {
    for (entity, level_file_name, position) in &mut level_query {
        info!("Spawning level: {:?}, position: {:?}", level_file_name.0, position);
        
        // Load the Tiled map using the level file name
        let map_handle: Handle<tiled::TiledMap> = asset_server.load(&level_file_name.0);

        // Spawn the Tiled map bundle
        commands.entity(entity).insert(tiled::TiledMapBundle {
            tiled_map: map_handle,
            transform: Transform::from_xyz(position.x + 500.0, position.y + 500.0, 500.0),
            ..Default::default()
        });
    }
}