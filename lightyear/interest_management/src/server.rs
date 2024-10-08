use bevy::prelude::*;
use bevy::utils::Duration;
use bevy::utils::HashMap;
use leafwing_input_manager::prelude::{ActionState, InputMap};

use lightyear::prelude::server::*;
use lightyear::prelude::*;

use crate::protocol::*;
use crate::shared::{color_from_id, shared_movement_behaviour};
use lightyear::connection::id::ClientId;

const TILE_SIZE: i32 = 16; // 16 pixels x 16 pixels
const LEVEL_SIZE: i32 = 64; // 64 tiles x 64 tiles
const GRID_SIZE: i32 = TILE_SIZE * LEVEL_SIZE; // 1024 pixels x 1024 pixels
const VIEW_DISTANCE: i32 = 1; // in grid units (1 = can see 1 grid unit away)

// Plugin for server-specific logic
pub struct ExampleServerPlugin;

impl Plugin for ExampleServerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Global>();
        app.add_systems(Startup, init);
        // the physics/FixedUpdates systems that consume inputs should be run in this set
        app.add_systems(FixedUpdate, movement);
        app.add_systems(
            Update,
            (
                handle_connections,
                handle_disconnections,
                // we don't have to run interest management every tick, only every time
                // we are buffering replication messages
                interest_management.in_set(ReplicationSet::SendMessages),
                receive_message,
            ),
        );
    }
}

#[derive(Resource, Default)]
pub(crate) struct Global {
    pub client_id_to_entity_id: HashMap<ClientId, Entity>,
    pub client_id_to_room_id: HashMap<ClientId, RoomId>,
}

pub(crate) fn init(mut commands: Commands, mut room_manager: ResMut<RoomManager>) {
    commands.start_server();
    commands.spawn(
        TextBundle::from_section(
            "Server",
            TextStyle {
                font_size: 30.0,
                color: Color::WHITE,
                ..default()
            },
        )
        .with_style(Style {
            align_self: AlignSelf::End,
            ..default()
        }),
    );

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

/// Server connection system, create a player upon connection
pub(crate) fn handle_connections(
    mut room_manager: ResMut<RoomManager>,
    mut connections: EventReader<ConnectEvent>,
    mut commands: Commands,
) {
    for connection in connections.read() {
        let position = Vec2::ZERO + Vec2::new(100.0, 100.0);
        let room_id = get_room_id_from_grid_position(position);
        let client_id = connection.client_id;
        let entity = commands.spawn(
            PlayerBundle::new(client_id, position)
        );
        
        // we can control the player visibility in a more static manner by using rooms
        // we add all clients to a room, as well as all player entities
        // this means that all clients will be able to see all player entities
        room_manager.add_client(client_id, room_id);
        room_manager.add_entity(entity.id(), room_id);
    }
}

/// Handle client disconnections: we want to despawn every entity that was controlled by that client.
///
/// Lightyear creates one entity per client, which contains metadata associated with that client.
/// You can find that entity by calling `ConnectionManager::client_entity(client_id)`.
///
/// That client entity contains the `ControlledEntities` component, which is a set of entities that are controlled by that client.
///
/// By default, lightyear automatically despawns all the `ControlledEntities` when the client disconnects;
/// but in this example we will also do it manually to showcase how it can be done.
/// (however we don't actually run the system)
pub(crate) fn handle_disconnections(
    mut commands: Commands,
    mut disconnections: EventReader<DisconnectEvent>,
    manager: Res<ConnectionManager>,
    client_query: Query<&ControlledEntities>,
) {
    for disconnection in disconnections.read() {
        debug!("Client {:?} disconnected", disconnection.client_id);
        if let Ok(client_entity) = manager.client_entity(disconnection.client_id) {
            if let Ok(controlled_entities) = client_query.get(client_entity) {
                for entity in controlled_entities.entities() {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}

pub(crate) fn receive_message(mut messages: EventReader<MessageEvent<Message1>>) {
    for message in messages.read() {
        info!("recv message");
    }
}

fn get_grid_position(position: Vec2) -> Vec2 {
    Vec2::new(
        (position.x / GRID_SIZE as f32).floor(),
        (position.y / GRID_SIZE as f32).floor(),
    )
}

fn get_room_id_from_grid_position(grid_position: Vec2) -> RoomId {
    fn cantor_pairing(a: i64, b: i64) -> i64 {
        (0.5 * (a + b) as f64 * (a + b + 1) as f64 + b as f64) as i64
    }

    fn bijective_map(n: i64) -> i64 {
        if n >= 0 { 2 * n } else { -2 * n - 1 }
    }

    RoomId(cantor_pairing(bijective_map(grid_position.x as i64), bijective_map(grid_position.y as i64)) as u64)
}

/// Here we perform more "immediate" interest management: we will make a circle visible to a client
/// depending on the distance to the client's entity
pub(crate) fn interest_management(
    mut room_manager: ResMut<RoomManager>,
    mut player_query: Query<(&PlayerId, Entity, Ref<Position>, &mut LastPosition)>
) {
    for (client_id, entity, position, last_position) in player_query.iter() {
        if position.is_changed() {
            let grid_position = get_grid_position(position.0);
            match last_position.0 {
                None => {
                    // Add the player to all rooms in the view distance
                    for dx in -VIEW_DISTANCE..=VIEW_DISTANCE {
                        for dy in -VIEW_DISTANCE..=VIEW_DISTANCE {
                            let view_grid_pos = grid_position + Vec2::new(dx as f32, dy as f32);
                            let room_id = get_room_id_from_grid_position(view_grid_pos);
                            room_manager.add_client(client_id.0, room_id);
                            if dx == 0 && dy == 0 { // Only add the entity to the room if it's in the center grid
                                room_manager.add_entity(entity, room_id);
                                info!("Player spawned, added to center grid_pos {:?} (id: {:?})", view_grid_pos, room_id);
                            }
                        }
                    }
                },
                Some(last_position) => {
                    let last_grid_position = get_grid_position(last_position);
                    if grid_position != last_grid_position {
                        let mut last_grid_positions = Vec::new();
                        let mut grid_positions = Vec::new();
                        // Find all grid positions that are in the view distance of the player
                        // as well as grid positions that were in the previous view distance of the player
                        for dx in -VIEW_DISTANCE..=VIEW_DISTANCE {
                            for dy in -VIEW_DISTANCE..=VIEW_DISTANCE {
                                let last_grid_pos = last_grid_position + Vec2::new(dx as f32, dy as f32);
                                let grid_pos = grid_position + Vec2::new(dx as f32, dy as f32);
                                last_grid_positions.push(last_grid_pos);
                                grid_positions.push(grid_pos);
                            }
                        }
                        // Remove the entity from the room it was in before
                        {
                            let room_id = get_room_id_from_grid_position(last_grid_position);
                            room_manager.remove_entity(entity, room_id);
                            info!("Player entity removed from grid_pos {:?} (id: {:?})", last_grid_position, room_id);
                        }
                        // Add the entity to the room it is in now
                        {
                            let room_id = get_room_id_from_grid_position(grid_position);
                            room_manager.add_entity(entity, room_id);
                            info!("Player entity added to grid_pos {:?} (id: {:?})", grid_position, room_id);
                        }
                        // Remove the client from rooms that are no longer in view
                        for last_grid_pos in last_grid_positions.iter().filter(|&pos| !grid_positions.contains(pos)) {
                            let room_id = get_room_id_from_grid_position(*last_grid_pos);
                            room_manager.remove_client(client_id.0, room_id);
                            // info!("Client removed from grid_pos {:?} (id: {:?})", last_grid_pos, room_id);
                        }
                        // Add the client to rooms that are now in view
                        for grid_pos in grid_positions.iter().filter(|&pos| !last_grid_positions.contains(pos)) {
                            let room_id = get_room_id_from_grid_position(*grid_pos);
                            room_manager.add_client(client_id.0, room_id);
                            // info!("Client added to grid_pos {:?} (id: {:?})", grid_pos, room_id);
                        }
                    }
                }
            }
        }
    }
    for (client_id, entity, position, mut last_position) in player_query.iter_mut() {
        last_position.0 = Some(position.0);
    }
}

/// Read client inputs and move players
pub(crate) fn movement(
    mut position_query: Query<(&mut Position, &ActionState<Inputs>), Without<InputMap<Inputs>>>,
) {
    for (position, input) in position_query.iter_mut() {
        shared_movement_behaviour(position, input);
    }
}
