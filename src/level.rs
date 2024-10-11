use bevy::prelude::*;
use bevy_ecs_tilemap_plugin::helpers::tiled;
use interest_management::{client::{ClientConnection, ConnectionManager, Interpolated, MessageEvent, Predicted}, protocol::{Channel1, LevelFile, LevelFileName, PlayerId, Position}};
use lightyear::shared::replication::network_target::NetworkTarget;

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
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
pub(crate) fn level_download(mut reader: ResMut<Events<MessageEvent<LevelFile>>>) {
    for event in reader.drain() {
        info!("Received message: {:?}", event.message());
    }
}