use bevy::{ecs::entity::MapEntities, prelude::*, render::RenderPlugin};
use client::{ComponentSyncMode, Confirmed};
use leafwing_input_manager::action_state::ActionState;
use leafwing_input_manager::input_map::InputMap;
use interest_management::{client::{ClientConnection, Interpolated, NetClient, Predicted}, server::get_room_id_from_grid_position, shared::{Inputs, LastPosition, PlayerId, Position}};
use lightyear::prelude::ReplicationGroup;
use lightyear::prelude::server::{ControlledBy, Replicate, SyncTarget};
use lightyear::prelude::*;
use server::RoomManager;

// For prediction, we want everything entity that is predicted to be part of the same replication group
// This will make sure that they will be replicated in the same message and that all the entities in the group
// will always be consistent (= on the same tick)
pub const REPLICATION_GROUP: ReplicationGroup = ReplicationGroup::new_id(1);

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
            .add_linear_interpolation_fn();

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
        app.add_systems(Update, handle_connections);
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

// ################################################################################################

pub struct PlayerClientPlugin;

impl Plugin for PlayerClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                add_input_map,
                player_spawn,
                camera_movement,
                animate_sprite,
            ),
        );
        app.add_systems(PreUpdate, handle_connection.after(MainSet::Receive));
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

fn player_spawn(
    connection: Res<ClientConnection>,
    mut commands: Commands,
    mut parent_query: Query<Entity>,
    mut character_query: Query<
        (&PlayerParent, &AnimationTimer, &AnimationIndices, &AnimationSpriteBundle, &PlayerTextureAtlasLayout),
        (Or<(Added<Predicted>, Added<Interpolated>)>),
    >,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    for (parent, animation_timer, animation_indices, animation_sprite_bundle, atlas_layout) in &mut character_query {
        let (parent_entity) = parent_query
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
            atlas,
        ));
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