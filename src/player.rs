use std::time::Duration;

use avian2d::{parry::shape::SharedShape, prelude::{AngularVelocity, Collider, ColliderDensity, LinearVelocity, Position, RigidBody, Rotation}};
use bevy::{ecs::{entity::MapEntities, query::QueryData}, prelude::*, render::RenderPlugin};
use bevy_ecs_tilemap_plugin::helpers::tiled;
use client::{ComponentSyncMode, Confirmed, PredictionSet, VisualInterpolateStatus, VisualInterpolationPlugin};
use leafwing_input_manager::action_state::ActionState;
use leafwing_input_manager::input_map::InputMap;
use interest_management::{client::{ClientConnection, Interpolated, NetClient, Predicted}, server::get_room_id_from_grid_position, shared::{Inputs, LastPosition, PlayerId}};
use lightyear::{prelude::ReplicationGroup, shared::replication::components::Controlled};
use lightyear::prelude::server::{ControlledBy, Replicate, SyncTarget};
use lightyear::prelude::*;
use lightyear::utils::avian2d::*;
use server::RoomManager;

// For prediction, we want everything entity that is predicted to be part of the same replication group
// This will make sure that they will be replicated in the same message and that all the entities in the group
// will always be consistent (= on the same tick)
pub const REPLICATION_GROUP: ReplicationGroup = ReplicationGroup::new_id(1);

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum FixedSet {
    // main fixed update systems (handle inputs)
    Main,
    // apply physics steps
    Physics,
}

// Player
#[derive(Bundle)]
pub(crate) struct PlayerBundle {
    id: PlayerId,
    position: Position,
    last_position: LastPosition, // used for checking if the position has crossed a grid boundary
    color: PlayerColor,
    replicate: Replicate,
    action_state: ActionState<Inputs>,
}

#[derive(Bundle)]
pub(crate) struct PhysicsBundle {
    collider: Collider,
    collider_density: ColliderDensity,
    rigid_body: RigidBody
}

impl PhysicsBundle {
    pub(crate) fn player() -> Self {
        // Note: due to a bug in older (?) versions of bevy_xpbd, using a triangle collider here
        // sometimes caused strange behaviour. Unsure if this is fixed now.
        // Also, counter-clockwise ordering of points was required for convex hull creation (?)
        Self {
            collider: Collider::rectangle(32.0, 32.0),
            collider_density: ColliderDensity(1.0),
            rigid_body: RigidBody::Dynamic,
        }
    }
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
            group: ReplicationGroup::default(),
            ..default()
        };

        // Use only the subset of sprites in the sheet that make up the run animation
        Self {
            id: PlayerId(id),
            position: Position(position),
            last_position: LastPosition(None),
            color: PlayerColor(color),
            replicate,
            action_state: ActionState::default(),
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

// Components
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct Player {
    pub client_id: ClientId,
    pub nickname: String,
    pub rtt: Duration,
    pub jitter: Duration,
}

impl Player {
    pub fn new(client_id: ClientId, nickname: String) -> Self {
        Self {
            client_id,
            nickname,
            rtt: Duration::ZERO,
            jitter: Duration::ZERO,
        }
    }
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct ApplyInputsQuery {
    pub lin_vel: &'static mut LinearVelocity,
    pub ang_vel: &'static mut AngularVelocity,
    pub player: &'static Player,
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
            app.add_systems(Update, (draw_boxes, draw_colliders));

            // set up visual interp plugins for Position and Rotation.
            // this doesn't do anything until you add VisualInterpolationStatus components to entities.
            app.add_plugins(VisualInterpolationPlugin::<Position>::default());
            // app.add_plugins(VisualInterpolationPlugin::<Rotation>::default());

            // observers that add VisualInterpolationStatus components to entities which receive
            // a Position or Rotation component.
            app.observe(add_visual_interpolation_components::<Position>);
            // app.observe(add_visual_interpolation_components::<Rotation>);
        }

        // inputs
        app.add_plugins(LeafwingInputPlugin::<Inputs>::default());
        // components
        // Player is synced as Simple, because we periodically update rtt ping stats
        app.register_component::<Player>(ChannelDirection::ServerToClient)
            .add_prediction(ComponentSyncMode::Simple);

        app.register_component::<PlayerId>(ChannelDirection::ServerToClient)
            .add_prediction(ComponentSyncMode::Once)
            .add_interpolation(ComponentSyncMode::Once);

        app.register_component::<Position>(ChannelDirection::ServerToClient)
            .add_prediction(ComponentSyncMode::Full)
            .add_interpolation_fn(position::lerp)
            .add_correction_fn(position::lerp);

        app.register_component::<LinearVelocity>(ChannelDirection::ServerToClient)
            .add_prediction(ComponentSyncMode::Full);

        app.register_component::<AngularVelocity>(ChannelDirection::ServerToClient)
            .add_prediction(ComponentSyncMode::Full);

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

        app.register_component::<PlayerParent>(ChannelDirection::ServerToClient)
            .add_map_entities()
            .add_prediction(ComponentSyncMode::Once)
            .add_interpolation(ComponentSyncMode::Once);
        // channels
        app.add_channel::<Channel1>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        });
    }
}

// Non-wall entities get some visual interpolation by adding the lightyear
// VisualInterpolateStatus component
//
// We query filter With<Predicted> so that the correct client entities get visual-interpolation.
// We don't want to visually interpolate the client's Confirmed entities, since they are not rendered.
//
// If your game uses Interpolated entities as well as Predicted, change the filter to:
//
//   Or<(With<Predicted>, With<Interpolated>)>
//
// We must trigger change detection so that the SyncPlugin will detect and sync changes
// from Position/Rotation to Transform.
//
// Without syncing interpolated pos/rot to transform, things like sprites, meshes, and text which
// render based on the *Transform* component (not avian's Position) will be stuttery.
//
// (Note also that we've configured avian's SyncPlugin to run in PostUpdate)
fn add_visual_interpolation_components<T: Component>(
    trigger: Trigger<OnAdd, T>,
    q: Query<Entity, (With<T>, With<Predicted>)>, // TODO TC - Without<Wall>
    mut commands: Commands,
) {
    if !q.contains(trigger.entity()) {
        return;
    }
    debug!("Adding visual interp component to {:?}", trigger.entity());
    commands
        .entity(trigger.entity())
        .insert(VisualInterpolateStatus::<T> {
            trigger_change_detection: true,
            ..default()
        });
}

/// System that draws the boxed of the player positions.
/// The components should be replicated from the server to the client
/// This time we will only draw the predicted/interpolated entities
pub(crate) fn draw_boxes(
    mut gizmos: Gizmos,
    mut players: Query<(&Position, &mut Transform), With<Player>>,
) {
    for (position, mut transform) in players.iter_mut() {
        gizmos.rect(
            Vec3::new(position.x, position.y, 0.0),
            Quat::IDENTITY,
            Vec2::ONE * 50.0,
            Color::linear_rgb(1.0, 0.0, 0.0)
        );
        transform.translation = Vec3::new(position.x, position.y, 0.0);
    }
}

fn draw_colliders(
    mut gizmos: Gizmos,
    query: Query<(&Transform, &Collider)>,
) {
    for (transform, collider) in query.iter() {
        gizmos.rect(
            Vec3::new(transform.translation.x, transform.translation.y, 0.0),
            Quat::IDENTITY,
            Vec2::new(32.0, 32.0),
            Color::linear_rgb(0.0, 1.0, 0.0)
        );
    }
}

/// Generate a color from the `ClientId`
pub fn color_from_id(client_id: ClientId) -> Color {
    let h = (((client_id.to_bits().wrapping_mul(30)) % 360) as f32) / 360.0;
    let s = 1.0;
    let l = 0.5;
    Color::hsl(h, s, l)
}

// This system defines how we update the player's positions when we receive an input
pub fn shared_movement_behaviour(
    aiq: &mut ApplyInputsQueryItem,
    action: &ActionState<Inputs>,
) {
    const MOVE_SPEED: f32 = 500.0;
    let velocity = &mut aiq.lin_vel;
    velocity.x = 0.0;
    velocity.y = 0.0;
    if action.pressed(&Inputs::Up) {
        velocity.y += MOVE_SPEED;
    }
    if action.pressed(&Inputs::Down) {
        velocity.y -= MOVE_SPEED;
    }
    if action.pressed(&Inputs::Left) {
        velocity.x -= MOVE_SPEED;
    }
    if action.pressed(&Inputs::Right) {
        velocity.x += MOVE_SPEED;
    }
    // *velocity = LinearVelocity(velocity.clamp_length_max(50.0));
}


// ################################################################################################

pub struct PlayerServerPlugin;

impl Plugin for PlayerServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_connections);
        // the physics/FixedUpdates systems that consume inputs should be run in this set
        app.add_systems(
            FixedUpdate,
            (movement)
                .chain()
                .in_set(FixedSet::Main),
        );
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

        // replicate newly connected clients to all players
        let replicate = Replicate {
            sync: SyncTarget {
                prediction: NetworkTarget::All,
                ..default()
            },
            controlled_by: ControlledBy {
                target: NetworkTarget::Single(client_id),
                ..default()
            },
            // make sure that all entities that are predicted are part of the same replication group
            group: REPLICATION_GROUP,
            ..default()
        };
        let entity = commands.spawn((
            Player::new(client_id, "Player".to_string()),
            PlayerId(client_id),
            ActionState::<Inputs>::default(),
            Position(position),
            LastPosition(None),
            replicate,
            PhysicsBundle::player(),
        )).id();
        let animation_entity = commands.spawn(
            AnimationBundle::new(client_id, entity)
        ).id();

        let room_id = get_room_id_from_grid_position(position);
        room_manager.add_entity(entity, room_id);
    }
}

// ################################################################################################

pub struct PlayerClientPlugin;

impl Plugin for PlayerClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                player_spawn,
                handle_new_player,
                camera_movement,
                animate_sprite,
            ),
        );
        // all actions related-system that can be rolled back should be in FixedUpdate schedule
        app.add_systems(
            FixedUpdate,
            (
                movement.run_if(not(is_host_server))
            )
                .chain()
                .in_set(FixedSet::Main),
        );
        app.add_systems(
            PreUpdate,
            handle_connection
                .after(MainSet::Receive)
                .before(PredictionSet::SpawnPrediction),
        );
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
    mut player_query: Query<(Entity, Has<Controlled>), (Added<Predicted>, With<Player>)>,
) {
    for (entity, is_controlled) in player_query.iter_mut() {
        // is this our own entity?
        if is_controlled {
            info!("Own player replicated to us, adding inputmap {entity:?}");
            commands.entity(entity).insert(InputMap::new([
                (Inputs::Up, KeyCode::ArrowUp),
                (Inputs::Down, KeyCode::ArrowDown),
                (Inputs::Left, KeyCode::ArrowLeft),
                (Inputs::Right, KeyCode::ArrowRight),
                (Inputs::Up, KeyCode::KeyW),
                (Inputs::Down, KeyCode::KeyS),
                (Inputs::Left, KeyCode::KeyA),
                (Inputs::Right, KeyCode::KeyD),
            ]));
        } else {
            info!("Remote player replicated to us: {entity:?}");
        }
        let client_id = connection.id();
        info!(?entity, ?client_id, "adding physics to predicted player");
        commands.entity(entity).insert(PhysicsBundle::player());
    }
}

fn player_spawn(
    connection: Res<ClientConnection>,
    mut commands: Commands,
    mut parent_query: Query<Entity>,
    mut character_query: Query<
        (&PlayerParent, &AnimationTimer, &AnimationIndices, &AnimationSpriteBundle, &PlayerTextureAtlasLayout),
        Or<(Added<Predicted>, Added<Interpolated>)>,
    >,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    // for (parent, animation_timer, animation_indices, animation_sprite_bundle, atlas_layout) in &mut character_query {
    //     let parent_entity = parent_query
    //         .get_mut(parent.0)
    //         .expect("Tail entity has no parent entity!");
    //     // spawn extra sprites, etc.
    //     let texture = asset_server.load(animation_sprite_bundle.texture.0.clone());
    //     let layout = TextureAtlasLayout::from_grid(atlas_layout.0.tile_size, atlas_layout.0.columns, atlas_layout.0.rows, None, atlas_layout.0.offset);
    //     let texture_atlas_layout = texture_atlas_layouts.add(layout);
    //     let atlas = TextureAtlas {
    //         layout: texture_atlas_layout.clone(),
    //         index: animation_indices.first,
    //     };

    //     let client_id = connection.id();
    //     info!(?parent, ?client_id, "Adding animation to character");
    //     commands.entity(parent_entity).insert((
    //         animation_timer.clone(),
    //         animation_indices.clone(),
    //         SpriteBundle {
    //             transform: Transform::from_xyz(0., 0., 17.).with_scale(Vec3::splat(2.0)),
    //             texture: texture.clone(),
    //             ..default()
    //         },
    //         atlas,
    //     ));
    // }
}

// The client input only gets applied to predicted entities that we own
// This works because we only predict the user's controlled entity.
// If we were predicting more entities, we would have to only apply movement to the player owned one.
pub(crate) fn movement(
    // TODO: maybe make prediction mode a separate component!!!
    mut position_query: Query<(&ActionState<Inputs>, ApplyInputsQuery), With<Player>>,
) {
    for (input, mut aiq) in position_query.iter_mut() {
        shared_movement_behaviour(&mut aiq, input);
        // transform.translation = position.0.extend(0.0);
    }
}

fn camera_movement(
    mut camera: Query<&mut Transform, With<Camera>>,
    player: Query<(&Position, Has<Controlled>), With<Player>>
) {
    for mut transform in &mut camera {
        for (player_transform, is_controlled) in &player {
            if is_controlled {
                transform.translation.x = player_transform.x;
                transform.translation.y = player_transform.y;
            }
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