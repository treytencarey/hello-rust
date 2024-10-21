use bevy::{ecs::entity::MapEntities, prelude::*};
use bevy_ecs_tilemap_plugin::helpers::tiled;
use interest_management::{client::{ComponentSyncMode, ConnectionManager, Interpolated, Predicted}, server::Global};
use lightyear::{prelude::{server::{Replicate, SyncTarget}, AppComponentExt, AppMessageExt, ChannelDirection, NetworkRelevanceMode, ReplicationGroup}, shared::replication::network_target::NetworkTarget};
use serde::{Deserialize, Serialize};
use lightyear::connection::id::ClientId;
use sha2::{Digest, Sha256};

use crate::player::Channel1;

// RemoteFile
#[derive(Bundle)]
pub(crate) struct RemoteFileBundle {
    parent: RemoteFileParent,
    replicate: Replicate,
    filename: RemoteFileName,
}

impl RemoteFileBundle {
    pub(crate) fn new(filename: String, parent: Entity) -> Self {
        let sync_target = SyncTarget {
            prediction: NetworkTarget::All,
            ..default()
        };
        let replicate = Replicate {
            sync: sync_target,
            group: ReplicationGroup::default().set_id(parent.to_bits()),
            relevance_mode: NetworkRelevanceMode::InterestManagement,
            ..default()
        };
        Self {
            parent: RemoteFileParent(parent),
            replicate,
            filename: RemoteFileName(filename)
        }
    }
}

// 
#[derive(Default, Component, Serialize, Deserialize, Debug, PartialEq, Clone, Reflect)]
pub struct RemoteFileName(pub String);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RemoteFileHash {
    hash: String,
    file_name: RemoteFileName,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RemoteFile {
    data: Vec<u8>,
    file_name: RemoteFileName,
}

impl Default for RemoteFile {
    fn default() -> Self {
        RemoteFile {
            data: Vec::new(),
            file_name: RemoteFileName::default(),
        }
    }
}

// and deriving the `MapEntities` trait for the component.
#[derive(Component, Deserialize, Serialize, Clone, Debug, PartialEq, Reflect)]
pub struct RemoteFileParent(pub Entity);

impl MapEntities for RemoteFileParent {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.0 = entity_mapper.map_entity(self.0);
    }
}

// ################################################################################################

#[derive(Clone)]
pub struct RemoteFileSharedPlugin;

impl Plugin for RemoteFileSharedPlugin {
    fn build(&self, app: &mut App) {
        app.register_component::<RemoteFileName>(ChannelDirection::ServerToClient)
            .add_prediction(ComponentSyncMode::Once)
            .add_interpolation(ComponentSyncMode::Once);

        app.register_message::<RemoteFile>(ChannelDirection::Bidirectional);

        app.register_message::<RemoteFileHash>(ChannelDirection::ClientToServer);
    }
}

fn remotefile_get_hash(
    remote_file_name: RemoteFileName,
) -> RemoteFileHash {
    // Get the file contents from the map_handle, hash it, and send to the server so we can remotefile_hash_check it
    let file_path = format!("assets/{}", remote_file_name.0);
    if let Ok(file_data) = std::fs::read(&file_path) {
        // Hash the file_data and send it to the server
        let mut hasher = Sha256::new();
        hasher.update(file_data);
        return RemoteFileHash {
            hash: format!("{:x}", hasher.finalize()),
            file_name: remote_file_name,
        }
    }
    return RemoteFileHash {
        hash: "".to_string(),
        file_name: remote_file_name,
    }
}

// ################################################################################################

pub struct RemoteFileServerPlugin;

impl Plugin for RemoteFileServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
        (remotefile_uploaded, remotefile_hash_check),
        );
        app.init_resource::<ConnectionManager>();
    }
}

fn remotefile_hash_check(
    mut reader: EventReader<lightyear::server::events::MessageEvent<RemoteFileHash>>,
    mut connection: ResMut<lightyear::server::connection::ConnectionManager>,
) {
    for event in reader.read() {
        let server_remotefile_hash = remotefile_get_hash(event.message.file_name.clone());
        // If the client's file hash doesn't match the server's, send the file to the client
        if server_remotefile_hash.hash != event.message.hash {
            info!("RemoteFile hash mismatch ({:?}): {:?}", server_remotefile_hash.hash, event.message.file_name.0);
            // Read the file before sending it to the client
            match std::fs::read(format!("assets/{}", event.message.file_name.0)) {
                Ok(file_data) => {
                    let mut message = RemoteFile {
                        data: file_data,
                        file_name: server_remotefile_hash.file_name.clone(),
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

pub(crate) fn remotefile_uploaded(
    mut reader: EventReader<lightyear::server::events::MessageEvent<RemoteFile>>,
    mut connection: ResMut<lightyear::server::connection::ConnectionManager>,
    global: ResMut<Global>,
) {
    for event in reader.read() {
        let client_id: ClientId = *event.context();

        // TODO - Check permissions

        // Save the remotefile file to the disk
        match std::fs::write(format!("assets/{}", event.message.file_name.0), &event.message.data) {
            Ok(_) => {
                info!("RemoteFile downloaded: {:?}", event.message.file_name.0);
            }
            Err(e) => {
                error!("Failed to download remotefile: {:?}", e);
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
        if !client_ids.is_empty() {
            info!("Client {:?} uploaded remotefile file {:?}, broadcasting to clients {:?}", client_id, event.message.clone().file_name, client_ids);
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

pub struct RemoteFileClientPlugin;

// Used to prevent a remotefile_download from calling remotefile_modified
#[derive(Resource, Default)]
pub struct RemoteFileDownloads {
    pub file_names: Vec<RemoteFileName>
}

impl Plugin for RemoteFileClientPlugin {
    fn build(&self, app: &mut App) {
        app
        .insert_resource(RemoteFileDownloads::default())
        .add_systems(
            Update,
        (remotefile_spawn, remotefile_download),
        );
    }
}

fn remotefile_spawn(
    mut client: ResMut<ConnectionManager>,
    mut remotefile_query: Query<
        (Entity, &RemoteFileName),
        Or<(Added<Interpolated>, Added<Predicted>)>,
    >,
) {
    for (entity, remote_file_name) in &mut remotefile_query {
        info!("Spawning remotefile: {:?}", remote_file_name.0);

        // Send a hash of the remotefile file to the server, so it can check if it's the latest version
        client.send_message::<Channel1, RemoteFileHash>(&mut remotefile_get_hash(remote_file_name.clone())).unwrap_or_else(|e| {
            error!("Failed to send message: {:?}", e);
        });
    }
}

// TODO: This currently must be added to the ClientPlugin, to register which Asset
// type to listen for events on. Need a better solution.
pub fn remotefile_modified<T: Asset>(
    mut events: EventReader<AssetEvent<T>>,
    mut client: ResMut<ConnectionManager>,
    mut remotefile_query: Query<&mut Handle<T>>,
    mut remotefile_downloads: ResMut<RemoteFileDownloads>,
) {
    for event in events.read() {
        info!("RemoteFile asset event: {:?}", event);
        // A remotefile was modified
        if let AssetEvent::Modified { id } = event {
            for map_handle in &mut remotefile_query {
                // Find the remotefile that was modified
                if map_handle.id() == *id {
                    let remote_file_name = RemoteFileName(map_handle.path().unwrap().path().display().to_string());
                    // 
                    if let Some(pos) = remotefile_downloads.file_names.iter().position(|name| *name == remote_file_name) {
                        remotefile_downloads.file_names.remove(pos);
                        info!("RemoteFile asset modified, but not uploaded to the server: {:?}", remote_file_name);
                    }
                    // File was changed, but not from the server. Try uploading to the server, who broadcasts it.
                    else
                    {
                        info!("RemoteFile asset modified: {:?}", map_handle.path().unwrap().path());
                        match std::fs::read(format!("assets/{}", remote_file_name.0)) {
                            Ok(file_data) => {
                                let mut message = RemoteFile {
                                    data: file_data,
                                    file_name: remote_file_name,
                                };
                                client.send_message::<Channel1, RemoteFile>(&mut message).unwrap_or_else(|e| {
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
pub(crate) fn remotefile_download(
    mut reader: ResMut<Events<lightyear::client::events::MessageEvent<RemoteFile>>>,
    mut remotefile_downloads: ResMut<RemoteFileDownloads>
) {
    for event in reader.drain() {
        remotefile_downloads.file_names.push(event.message.file_name.clone());
        // If there is file_data, save it to the disk
        match std::fs::write(format!("assets/{}", event.message.file_name.0), &event.message.data) {
            Ok(_) => {
                info!("RemoteFile downloaded: {:?}", event.message.file_name.0);
            }
            Err(e) => {
                error!("Failed to download remotefile: {:?}", e);
            }
        }
    }
}