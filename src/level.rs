use bevy::{prelude::*};
use bevy_ecs_tilemap_plugin::helpers::tiled;
use interest_management::{client::{ClientConnection, ComponentSyncMode, ConnectionManager, Interpolated, Predicted}, protocol::{Channel1, LastPosition, PlayerId, Position, REPLICATION_GROUP}, server::{get_grid_position, get_room_id_from_grid_position, Global, GRID_SIZE}};
use lightyear::{prelude::{server::{Replicate, RoomManager, SyncTarget}, AppComponentExt, AppMessageExt, ChannelDirection, InputChannel, NetworkRelevanceMode}, shared::replication::network_target::NetworkTarget};
use serde::{Deserialize, Serialize};
use lightyear::connection::id::ClientId;
use lightyear::prelude::server::*;

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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LevelFile(pub Vec<u8>);

#[derive(Component, Serialize, Deserialize, Debug, PartialEq, Clone, Reflect)]
pub struct LevelFileName(pub String);

// ################################################################################################

#[derive(Clone)]
pub struct LevelSharedPlugin;

impl Plugin for LevelSharedPlugin {
    fn build(&self, app: &mut App) {
        app.register_component::<LevelFileName>(ChannelDirection::ServerToClient)
            .add_prediction(ComponentSyncMode::Once)
            .add_interpolation(ComponentSyncMode::Once);

        app.register_message::<LevelFile>(ChannelDirection::ClientToServer);
        app.register_message::<LevelFile>(ChannelDirection::ServerToClient);
    }
}

// ################################################################################################

pub struct LevelServerPlugin;

impl Plugin for LevelServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
        (level_upload),
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
                LevelBundle::new(position, "map_0.tmx".to_owned())
            ).id();

            info!("Level spawned, added to room: {:?} {:?}", room_id.0, position);
            room_manager.add_entity(level_entity, room_id);
        }
    }
}

pub(crate) fn level_upload(
    mut reader: EventReader<lightyear::server::events::MessageEvent<LevelFile>>,
    mut connection: ResMut<lightyear::server::connection::ConnectionManager>,
    mut global: ResMut<Global>,
) {
    for mut event in reader.read() {
        let client_id: ClientId = *event.context();
        // TODO - Check permissions
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
        info!("Client {:?} uploaded level file {:?}, broadcasting to clients {:?}", client_id, event.message.0, client_ids);
        if !client_ids.is_empty() {
            match connection.send_message_to_target::<Channel1, _>(
                &mut LevelFile("Test".into()),
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

impl Plugin for LevelClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
        (level_spawn, level_modified, level_download),
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

fn level_modified(
    mut events: EventReader<AssetEvent<tiled::TiledMap>>,
    mut client: ResMut<ConnectionManager>,
    asset_server: Res<AssetServer>,
    mut level_query: Query<(&LevelFileName, &mut Handle<tiled::TiledMap>)>,
    players: Query<&PlayerId>,
) {
    for event in events.read() {
        if let AssetEvent::Modified { id } = event {
            for (level_file_name, mut map_handle) in &mut level_query {
                if map_handle.id() == *id {
                    info!("Level asset modified: {:?}", level_file_name.0);
                    // TODO: Upload the new level to the server, if not modified by the server
                    let mut message = LevelFile(level_file_name.0.as_bytes().to_vec());
                    info!("Send message: {:?}", message);
                    // the message will be re-broadcasted by the server to all clients
                    client.send_message::<Channel1, LevelFile>(&mut message).unwrap_or_else(|e| {
                        error!("Failed to send message: {:?}", e);
                    });
                    break;
                }
            }
        }
    }
}

// System to receive messages on the client
pub(crate) fn level_download(mut reader: ResMut<Events<lightyear::client::events::MessageEvent<LevelFile>>>) {
    for event in reader.drain() {
        info!("Received message: {:?}", event.message());
    }
}