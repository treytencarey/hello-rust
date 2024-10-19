use bevy::prelude::*;
use bevy_ecs_tilemap_plugin::helpers::tiled;
use interest_management::{client::{ComponentSyncMode, ConnectionManager, Interpolated, Predicted}, server::{get_grid_position, get_room_id_from_grid_position, Global, GRID_SIZE}, shared::{LastPosition, Position}};
use lightyear::{prelude::{server::{Replicate, RoomManager, SyncTarget}, AppComponentExt, AppMessageExt, ChannelDirection, NetworkRelevanceMode}, shared::replication::network_target::NetworkTarget};
use serde::{Deserialize, Serialize};
use lightyear::connection::id::ClientId;
use sha2::{Digest, Sha256};

use crate::player::{Channel1, REPLICATION_GROUP};

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
            group: REPLICATION_GROUP,
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

        app.register_message::<LevelFile>(ChannelDirection::Bidirectional);

        app.register_message::<LevelFileHash>(ChannelDirection::ClientToServer);
    }
}

fn level_get_hash(
    level_file_name: LevelFileName,
) -> LevelFileHash {
    // Get the file contents from the map_handle, hash it, and send to the server so we can level_hash_check it
    let file_path = format!("assets/{}", level_file_name.0);
    if let Ok(file_data) = std::fs::read(&file_path) {
        // Hash the file_data and send it to the server
        let mut hasher = Sha256::new();
        hasher.update(file_data);
        return LevelFileHash {
            hash: format!("{:x}", hasher.finalize()),
            file_name: level_file_name,
        }
    }
    return LevelFileHash {
        hash: "".to_string(),
        file_name: level_file_name,
    }
}

// ################################################################################################

pub struct LevelServerPlugin;

impl Plugin for LevelServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
        (level_uploaded, level_hash_check),
        );
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
            
            let level_entity = commands.spawn(
                LevelBundle::new(position, format!("map_{}.tmx", room_id.0))
            ).id();

            info!("Level spawned, added to room: {:?} {:?}", room_id.0, position);
            room_manager.add_entity(level_entity, room_id);
        }
    }
}

fn level_hash_check(
    mut reader: EventReader<lightyear::server::events::MessageEvent<LevelFileHash>>,
    mut connection: ResMut<lightyear::server::connection::ConnectionManager>,
) {
    for event in reader.read() {
        let server_level_hash = level_get_hash(event.message.file_name.clone());
        // If the client's file hash doesn't match the server's, send the file to the client
        if server_level_hash.hash != event.message.hash {
            info!("Level hash mismatch ({:?}): {:?}", server_level_hash.hash, event.message.file_name.0);
            // Read the file before sending it to the client
            match std::fs::read(format!("assets/{}", event.message.file_name.0)) {
                Ok(file_data) => {
                    let mut message = LevelFile {
                        data: file_data,
                        file_name: server_level_hash.file_name.clone(),
                    };
                    match connection.send_message_to_target::<Channel1, _>(
                        &mut message,
                        NetworkTarget::Single(*event.context()),
                    ) {
                        Ok(_) => {
                            info!("Message sent successfully.");
                        }
                        Err(e) => {
                            error!("Failed to send message: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to read file: {:?}", e);
                }
            }
        }
    }
}

pub(crate) fn level_uploaded(
    mut reader: EventReader<lightyear::server::events::MessageEvent<LevelFile>>,
    mut connection: ResMut<lightyear::server::connection::ConnectionManager>,
    global: ResMut<Global>,
) {
    for event in reader.read() {
        let client_id: ClientId = *event.context();

        // TODO - Check permissions

        // Save the level file to the disk
        match std::fs::write(format!("assets/{}", event.message.file_name.0), &event.message.data) {
            Ok(_) => {
                info!("Level downloaded: {:?}", event.message.file_name.0);
            }
            Err(e) => {
                error!("Failed to download level: {:?}", e);
            }
        }

        // Get rooms the uploader is in
        let room_ids = global.client_id_to_room_ids.get(&client_id).unwrap();
        // Get all clients in those rooms
        let client_ids: Vec<_> = room_ids.iter()
            .flat_map(|room_id| global.room_id_to_client_ids.get(room_id).unwrap())
            .filter(|&&room_client_id| room_client_id != client_id)
            .cloned()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        // Broadcast the message to all clients in the rooms
        info!("Client {:?} uploaded level file {:?}, broadcasting to clients {:?}", client_id, event.message.clone().file_name, client_ids);
        if !client_ids.is_empty() {
            match connection.send_message_to_target::<Channel1, _>(
                &mut event.message.clone(),
                NetworkTarget::Only(client_ids),
            ) {
                Ok(_) => {
                    info!("Message sent successfully.");
                }
                Err(e) => {
                    error!("Failed to send message: {:?}", e);
                }
            }
        }
    }
}

// ################################################################################################

pub struct LevelClientPlugin;

// Used to prevent a level_download from calling level_modified
#[derive(Resource, Default)]
pub(crate) struct LevelDownloads {
    pub file_names: Vec<LevelFileName>
}

impl Plugin for LevelClientPlugin {
    fn build(&self, app: &mut App) {
        app
        .insert_resource(LevelDownloads::default())
        .add_systems(
            Update,
        (level_spawn, level_modified, level_download),
        );
    }
}

fn level_spawn(
    mut commands: Commands,
    mut client: ResMut<ConnectionManager>,
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

        // Send a hash of the level file to the server, so it can check if it's the latest version
        client.send_message::<Channel1, LevelFileHash>(&mut level_get_hash(level_file_name.clone())).unwrap_or_else(|e| {
            error!("Failed to send message: {:?}", e);
        });

        // Spawn the Tiled map bundle
        commands.entity(entity).insert(tiled::TiledMapBundle {
            tiled_map: map_handle,
            transform: Transform::from_xyz(position.x + 500.0, position.y + 500.0, 500.0),
            ..Default::default()
        });
    }

}

fn level_modified(
    mut events: EventReader<AssetEvent<tiled::TiledMap>>,
    mut client: ResMut<ConnectionManager>,
    mut level_query: Query<(&LevelFileName, &mut Handle<tiled::TiledMap>)>,
    mut level_downloads: ResMut<LevelDownloads>,
) {
    for event in events.read() {
        // A level was modified
        if let AssetEvent::Modified { id } = event {
            for (level_file_name, map_handle) in &mut level_query {
                // Find the level that was modified
                if map_handle.id() == *id {
                    // 
                    if let Some(pos) = level_downloads.file_names.iter().position(|name| name == level_file_name) {
                        level_downloads.file_names.remove(pos);
                        info!("Level asset modified, but not uploaded to the server: {:?}", level_file_name.0);
                    }
                    // File was changed, but not from the server. Try uploading to the server, who broadcasts it.
                    else
                    {
                        info!("Level asset modified: {:?}", map_handle.path().unwrap().path());
                        match std::fs::read(format!("assets/{}", map_handle.path().unwrap().path().display())) {
                            Ok(file_data) => {
                                let mut message = LevelFile {
                                    data: file_data,
                                    file_name: level_file_name.clone(),
                                };
                                client.send_message::<Channel1, LevelFile>(&mut message).unwrap_or_else(|e| {
                                    error!("Failed to send message: {:?}", e);
                                });
                            }
                            Err(e) => {
                                error!("Failed to read file: {:?}", e);
                            }
                        }
                    }
                    break;
                }
            }
        }
    }
}

// System to receive messages on the client
pub(crate) fn level_download(
    mut reader: ResMut<Events<lightyear::client::events::MessageEvent<LevelFile>>>,
    mut level_downloads: ResMut<LevelDownloads>
) {
    for event in reader.drain() {
        level_downloads.file_names.push(event.message.file_name.clone());
        // If there is file_data, save it to the disk
        match std::fs::write(format!("assets/{}", event.message.file_name.0), &event.message.data) {
            Ok(_) => {
                info!("Level downloaded: {:?}", event.message.file_name.0);
            }
            Err(e) => {
                error!("Failed to download level: {:?}", e);
            }
        }
    }
}