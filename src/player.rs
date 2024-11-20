#![allow(deprecated)]
use std::sync::Mutex;

use avian2d::prelude::{AngularVelocity, LinearVelocity, Position};
use bevy::{ecs::entity::MapEntities, prelude::*, render::RenderPlugin};
use bevy_mod_scripting::prelude::*;
use client::{ComponentSyncMode, Confirmed, PredictionSet};
use leafwing_input_manager::{action_state::ActionState, InputManagerBundle};
use leafwing_input_manager::input_map::InputMap;
use interest_management::{client::{ClientConnection, NetClient, Predicted}, server::{get_room_id_from_grid_position, Global}, shared::{Inputs, LastPosition, PlayerId, PlayerRoom, PhysicsBundle}};
use lightyear::prelude::ReplicationGroup;
use lightyear::prelude::server::{ControlledBy, Replicate, SyncTarget};
use lightyear::prelude::*;
use lightyear::shared::replication::components::Controlled;
use lightyear::utils::avian2d::*;
use server::{RoomManager, ServerReplicationSet};

// For prediction, we want everything entity that is predicted to be part of the same replication group
// This will make sure that they will be replicated in the same message and that all the entities in the group
// will always be consistent (= on the same tick)
pub const REPLICATION_GROUP: ReplicationGroup = ReplicationGroup::new_id(1);

#[derive(Debug, Default, Clone, Reflect, Component, LuaProxy, Serialize, Deserialize, PartialEq)]
#[reflect(Component, LuaProxyable)]
pub struct PlayerState {
    pub room: u64,
}

#[derive(Default)]
pub struct PlayerAPI;

impl APIProvider for PlayerAPI {
    type APITarget = Mutex<Lua>;
    type ScriptContext = Mutex<Lua>;
    type DocTarget = LuaDocFragment;

    fn attach_api(&mut self, _: &mut Self::APITarget) -> Result<(), ScriptError> {
        // we don't actually provide anything global
        Ok(())
    }

    fn register_with_app(&self, app: &mut App) {
        // this will register the `LuaProxyable` typedata since we derived it
        // this will resolve retrievals of this component to our custom lua object
        app.register_type::<PlayerState>();
    }
}

// ################################################################################################

// Player
#[derive(Bundle)]
pub(crate) struct PlayerBundle {
    id: PlayerId,
    room_id: PlayerRoom,
    position: Position,
    last_position: LastPosition, // used for checking if the position has crossed a grid boundary
    color: PlayerColor,
    replicate: Replicate,
    player_state: PlayerState,
    physics: PhysicsBundle,
    inputs: InputManagerBundle<Inputs>,
    // IMPORTANT: this lets the server know that the entity is pre-predicted
    // when the server replicates this entity; we will get a Confirmed entity which will use this entity
    // as the Predicted version
    pre_predicted: PrePredicted,
}

// Animation
#[derive(Bundle)]
pub(crate) struct AnimationBundle {
    parent: PlayerParent,
    animation_timer: AnimationTimer,
    animation_indices: AnimationIndices,
    animation_sprite_bundle: AnimationSpriteBundle,
    atlas: PlayerTextureAtlasLayout,
    replicate: Replicate,
}

impl PlayerBundle {
    pub(crate) fn new(id: ClientId, position: Vec2) -> Self {
        let color = color_from_id(id);
        let replicate = Replicate {
            sync: SyncTarget {
                prediction: NetworkTarget::Single(id),
                interpolation: NetworkTarget::AllExceptSingle(id),
            },
            controlled_by: ControlledBy {
                target: NetworkTarget::Single(id),
                ..default()
            },
            // use network relevance for replication
            relevance_mode: NetworkRelevanceMode::InterestManagement,
            // NOTE (important): all entities that are being predicted need to be part of the same replication-group
            //  so that all their updates are sent as a single message and are consistent (on the same tick)
            group: REPLICATION_GROUP,
            ..default()
        };

        // Use only the subset of sprites in the sheet that make up the run animation
        Self {
            id: PlayerId(id),
            room_id: PlayerRoom(0),
            position: Position(position),
            last_position: LastPosition(None),
            color: PlayerColor(color),
            replicate,
            player_state: PlayerState {
                room: 0,
            },
            physics: PhysicsBundle::player(),
            inputs: InputManagerBundle::<Inputs> {
                action_state: ActionState::default(),
                input_map: Self::get_input_map(),
            },
            pre_predicted: PrePredicted::default(),
        }
    }
    pub(crate) fn get_input_map() -> InputMap<Inputs> {
        InputMap::new([
            (Inputs::Right, KeyCode::ArrowRight),
            (Inputs::Right, KeyCode::KeyD),
            (Inputs::Left, KeyCode::ArrowLeft),
            (Inputs::Left, KeyCode::KeyA),
            (Inputs::Up, KeyCode::ArrowUp),
            (Inputs::Up, KeyCode::KeyW),
            (Inputs::Down, KeyCode::ArrowDown),
            (Inputs::Down, KeyCode::KeyS),
            (Inputs::Delete, KeyCode::Backspace),
            (Inputs::Spawn, KeyCode::Space),
        ])
    }
}

impl AnimationBundle {
    pub(crate) fn new(id: ClientId, parent: Entity) -> Self {
        let animation_indices = AnimationIndices { first: 0, last: 3 };
        Self {
            parent: PlayerParent(parent),
            animation_timer: AnimationTimer(Timer::from_seconds(0.3, TimerMode::Repeating)),
            animation_indices,
            animation_sprite_bundle: AnimationSpriteBundle {
                transform: Transform::from_xyz(0., 0., 17.).with_scale(Vec3::splat(2.0)),
                texture: PlayerTexture("EPIC RPG World - Ancient Ruins V 1.9.1/ERW - Ancient Ruins V 1.9.1/Characters/silly luck creature-idle.png".to_string()),
            },
            atlas: PlayerTextureAtlasLayout(PlayerTextureLayout {
                tile_size: UVec2::new(96, 85),
                columns: 4,
                rows: 1,
                offset: None,
            }),
            replicate: Replicate {
                sync: SyncTarget {
                    prediction: NetworkTarget::Single(id),
                    interpolation: NetworkTarget::AllExceptSingle(id),
                },
                controlled_by: ControlledBy {
                    target: NetworkTarget::Single(id),
                    ..default()
                },
                // replicate this entity within the same replication group as the parent
                group: ReplicationGroup::default().set_id(parent.to_bits()),
                ..default()
            },
        }
    }
}

// and deriving the `MapEntities` trait for the component.
#[derive(Component, Deserialize, Serialize, Clone, Debug, PartialEq, Reflect)]
pub struct PlayerParent(pub Entity);

impl MapEntities for PlayerParent {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.0 = entity_mapper.map_entity(self.0);
    }
}

// Components

#[derive(Component, Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct PlayerColor(pub(crate) Color);

#[derive(Component, Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct PlayerTextureAtlasLayout(pub PlayerTextureLayout);

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct PlayerTextureLayout {
    pub tile_size: UVec2,
    pub columns: u32,
    pub rows: u32,
    pub offset: Option<UVec2>
}

#[derive(Component, Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct PlayerTexture(pub String);

// Channels

#[derive(Channel)]
pub struct Channel1;

// Messages

#[derive(Component, Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct AnimationIndices {
    pub first: usize,
    pub last: usize,
}

#[derive(Component, Deref, DerefMut, Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct AnimationTimer(pub Timer);

#[derive(Component, Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct AnimationSpriteBundle {
    pub transform: Transform,
    pub texture: PlayerTexture,
}

// ################################################################################################

#[derive(Clone)]
pub struct PlayerSharedPlugin;

impl Plugin for PlayerSharedPlugin {
    fn build(&self, app: &mut App) {
        // If we can render, add box drawing
        if app.is_plugin_added::<RenderPlugin>() {
            app.add_systems(Update, draw_boxes);
        }

        // inputs
        app.add_plugins(LeafwingInputPlugin::<Inputs>::default());
        // components
        app.register_component::<PlayerId>(ChannelDirection::ServerToClient)
            .add_prediction(ComponentSyncMode::Once)
            .add_interpolation(ComponentSyncMode::Once);

        app.register_component::<Position>(ChannelDirection::Bidirectional)
            .add_prediction(ComponentSyncMode::Full)
            .add_interpolation(ComponentSyncMode::Full)
            .add_interpolation_fn(position::lerp)
            .add_correction_fn(position::lerp);

        app.register_component::<PlayerColor>(ChannelDirection::ServerToClient)
            .add_prediction(ComponentSyncMode::Once)
            .add_interpolation(ComponentSyncMode::Once);

        app.register_component::<PlayerTextureAtlasLayout>(ChannelDirection::ServerToClient)
            .add_prediction(ComponentSyncMode::Simple)
            .add_interpolation(ComponentSyncMode::Simple);
        
        app.register_component::<AnimationSpriteBundle>(ChannelDirection::ServerToClient)
            .add_prediction(ComponentSyncMode::Simple)
            .add_interpolation(ComponentSyncMode::Simple);

        app.register_component::<AnimationIndices>(ChannelDirection::ServerToClient)
            .add_prediction(ComponentSyncMode::Simple)
            .add_interpolation(ComponentSyncMode::Simple);

        app.register_component::<AnimationTimer>(ChannelDirection::ServerToClient)
            .add_prediction(ComponentSyncMode::Simple)
            .add_interpolation(ComponentSyncMode::Simple);

        app.register_component::<PlayerState>(ChannelDirection::ServerToClient)
            .add_prediction(ComponentSyncMode::Simple)
            .add_interpolation(ComponentSyncMode::Simple);

        app.register_component::<PlayerParent>(ChannelDirection::ServerToClient)
            .add_map_entities()
            .add_prediction(ComponentSyncMode::Once)
            .add_interpolation(ComponentSyncMode::Once);

        // NOTE: interpolation/correction is only needed for components that are visually displayed!
        // we still need prediction to be able to correctly predict the physics on the client
        app.register_component::<LinearVelocity>(ChannelDirection::Bidirectional)
            .add_prediction(ComponentSyncMode::Full);

        app.register_component::<AngularVelocity>(ChannelDirection::Bidirectional)
            .add_prediction(ComponentSyncMode::Full);

        // channels
        app.add_channel::<Channel1>(ChannelSettings { 
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        });
    }
}

/// System that draws the boxed of the player positions.
/// The components should be replicated from the server to the client
/// This time we will only draw the predicted/interpolated entities
pub(crate) fn draw_boxes(
    mut gizmos: Gizmos,
    mut players: Query<(&Position, &mut Transform, &PlayerColor), Without<Confirmed>>,
) {
    for (position, mut transform, color) in players.iter_mut() {
        gizmos.rect(
            Vec3::new(position.x, position.y, 0.0),
            Quat::IDENTITY,
            Vec2::ONE * 50.0,
            color.0,
        );
        transform.translation = Vec3::new(position.x, position.y, 0.0);
    }
}

/// Generate a color from the `ClientId`
pub fn color_from_id(client_id: ClientId) -> Color {
    let h = (((client_id.to_bits().wrapping_mul(30)) % 360) as f32) / 360.0;
    let s = 1.0;
    let l = 0.5;
    Color::hsl(h, s, l)
}


// ################################################################################################

pub struct PlayerServerPlugin;

impl Plugin for PlayerServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_player_state);
        app.add_systems(
            PreUpdate,
            handle_connections
                .after(MainSet::Receive)
                .before(PredictionSet::SpawnPrediction),
        );
        // Re-adding Replicate components to client-replicated entities must be done in this set for proper handling.
        app.add_systems(
            PreUpdate,
            replicate_players.in_set(ServerReplicationSet::ClientReplication),
        );
    }
}

pub(crate) fn update_player_state(
    mut player_query: Query<(&mut PlayerState, &PlayerRoom), Changed<PlayerRoom>>,
) {
    for (mut player_state, player_room) in player_query.iter_mut() {
        player_state.room = player_room.0;
    }
}

/// Server connection system, create a player upon connection
pub(crate) fn handle_connections(
    mut room_manager: ResMut<RoomManager>,
    mut connections: EventReader<lightyear::server::events::ConnectEvent>,
    mut commands: Commands,
) {
    for connection in connections.read() {
        let position = Vec2::ZERO + Vec2::new(100.0, 100.0);
        let client_id = connection.client_id;
        let entity = commands.spawn(
            PlayerBundle::new(client_id, position)
        ).id();
        let animation_entity = commands.spawn(
            AnimationBundle::new(client_id, entity)
        ).id();

        let room_id = get_room_id_from_grid_position(position);
        room_manager.add_entity(entity, room_id);
    }
}

// Replicate the pre-predicted entities back to the client
pub(crate) fn replicate_players(
    global: Res<Global>,
    mut commands: Commands,
    query: Query<(Entity, &Replicated), (Added<Replicated>, With<PlayerId>)>,
) {
    for (entity, replicated) in query.iter() {
        let client_id = replicated.client_id();
        info!("received player spawn event from client {client_id:?}");

        // for all player entities we have received, add a Replicate component so that we can start replicating it
        // to other clients
        if let Some(mut e) = commands.get_entity(entity) {
            // we want to replicate back to the original client, since they are using a pre-predicted entity
            let mut sync_target = SyncTarget::default();

            // if global.predict_all {
                sync_target.prediction = NetworkTarget::All;
            // } else {
            //     // we want the other clients to apply interpolation for the player
            //     sync_target.interpolation = NetworkTarget::AllExceptSingle(client_id);
            // }
            let replicate = Replicate {
                sync: sync_target,
                controlled_by: ControlledBy {
                    target: NetworkTarget::Single(client_id),
                    ..default()
                },
                // make sure that all entities that are predicted are part of the same replication group
                group: REPLICATION_GROUP,
                ..default()
            };
            e.insert((
                replicate,
                // if we receive a pre-predicted entity, only send the prepredicted component back
                // to the original client
                OverrideTargetComponent::<PrePredicted>::new(NetworkTarget::Single(client_id)),
                // not all physics components are replicated over the network, so add them on the server as well
                PhysicsBundle::player(),
            ));
        }
    }
}

// ################################################################################################

pub struct PlayerClientPlugin;

impl Plugin for PlayerClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                player_state_updated,
                add_input_map,
                handle_new_player,
                camera_movement,
                animate_sprite,
            ),
        );
        app.add_systems(PreUpdate, handle_connection.after(MainSet::Receive));
        app.add_api_provider::<LuaScriptHost<()>>(Box::new(PlayerAPI));
    }
}

pub(crate) fn player_state_updated(
    player_query: Query<&PlayerState, (With<Predicted>, Changed<PlayerState>)>,
) {
    for player_state in player_query.iter() {
        info!(?player_state, "Player state updated");
    }
}

// System to receive messages on the client
pub(crate) fn add_input_map(
    mut commands: Commands,
    predicted_players: Query<Entity, (Added<PlayerId>, With<Predicted>)>,
) {
    // we don't want to replicate the ActionState from the server to client, because if we have an ActionState
    // on the Confirmed player it will keep getting replicated to Predicted and will interfere with our inputs
    for player_entity in predicted_players.iter() {
        commands.entity(player_entity).insert((
            PlayerBundle::get_input_map(),
            ActionState::<Inputs>::default(),
        ));
    }
}

/// Listen for events to know when the client is connected, and spawn a text entity
/// to display the client id
pub(crate) fn handle_connection(
    mut commands: Commands,
    mut connection_event: EventReader<lightyear::client::events::ConnectEvent>,
) {
    for event in connection_event.read() {
        let client_id = event.client_id();
        commands.spawn(TextBundle::from_section(
            format!("Client {}", client_id),
            TextStyle {
                font_size: 30.0,
                color: Color::WHITE,
                ..default()
            },
        ));
    }
}

/// Decorate newly connecting players with physics components
/// ..and if it's our own player, set up input stuff
#[allow(clippy::type_complexity)]
fn handle_new_player(
    connection: Res<ClientConnection>,
    mut commands: Commands,
    mut parent_query: Query<Entity>,
    mut character_query: Query<
        (Entity, Has<Controlled>, &PlayerParent, &AnimationTimer, &AnimationIndices, &AnimationSpriteBundle, &PlayerTextureAtlasLayout),
        (Added<Predicted>, With<PlayerId>)
    >,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    for (entity, is_controlled, parent, animation_timer, animation_indices, animation_sprite_bundle, atlas_layout) in &mut character_query {
        let parent_entity = parent_query
            .get_mut(parent.0)
            .expect("Tail entity has no parent entity!");
        // spawn extra sprites, etc.
        let texture = asset_server.load(animation_sprite_bundle.texture.0.clone());
        let layout = TextureAtlasLayout::from_grid(atlas_layout.0.tile_size, atlas_layout.0.columns, atlas_layout.0.rows, None, atlas_layout.0.offset);
        let texture_atlas_layout = texture_atlas_layouts.add(layout);
        let atlas = TextureAtlas {
            layout: texture_atlas_layout.clone(),
            index: animation_indices.first,
        };

        let client_id = connection.id();
        info!(?parent, ?client_id, "Adding animation to character");
        commands.entity(parent_entity).insert((
            animation_timer.clone(),
            animation_indices.clone(),
            SpriteBundle {
                transform: Transform::from_xyz(0., 0., 17.).with_scale(Vec3::splat(2.0)),
                texture: texture.clone(),
                ..default()
            },
            atlas
        ));

        // is this our own entity?
        if is_controlled {
            info!("Own player replicated to us, adding inputmap {entity:?}");
            commands.entity(entity).insert(PlayerBundle::get_input_map());
        } else {
            info!("Remote player replicated to us: {entity:?}");
        }
        let client_id = connection.id();
        info!(?entity, ?client_id, "adding physics to predicted player");
        commands.entity(entity).insert(PhysicsBundle::player());
    }
}

fn camera_movement(
    mut camera: Query<&mut Transform, With<Camera>>,
    player: Query<&Position, With<Predicted>>
) {
    for mut transform in &mut camera {
        for player_transform in &player {
            transform.translation.x = player_transform.x;
            transform.translation.y = player_transform.y;
        }
    }
}

fn animate_sprite(
    time: Res<Time>,
    mut query: Query<(&AnimationIndices, &mut AnimationTimer, &mut TextureAtlas)>,
) {
    for (indices, mut timer, mut atlas) in &mut query {
        timer.tick(time.delta());
        if timer.just_finished() {
            atlas.index = if atlas.index == indices.last {
                indices.first
            } else {
                atlas.index + 1
            };
        }
    }
}