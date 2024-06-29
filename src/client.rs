use bevy::prelude::*;
use leafwing_input_manager::prelude::*;
pub use lightyear::prelude::client::*;
use lightyear::prelude::*;

use crate::player::{self, AnimationIndices, AnimationTimer, PlayerBundle};
use crate::protocol::*;
use crate::shared::shared_movement_behaviour;

pub struct ExampleClientPlugin;

impl Plugin for ExampleClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(LeafwingInputPlugin::<MyProtocol, Inputs>::default());
        app.init_resource::<ActionState<Inputs>>();
        app.add_systems(Startup, init);
        app.add_systems(PreUpdate, handle_connection.after(MainSet::Receive));
        app.add_systems(FixedUpdate, (movement, spawn_player));
        app.add_systems(
            Update,
            (
                add_input_map,
                camera_movement
            ),
        );
        app.add_plugins(player::PlayerPlugin);
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

/// Startup system for the client
pub(crate) fn init(mut commands: Commands) {
    commands.connect_client();
}

/// Listen for events to know when the client is connected, and spawn a text entity
/// to display the client id
pub(crate) fn handle_connection(
    mut commands: Commands,
    mut connection_event: EventReader<ConnectEvent>,
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

// The client input only gets applied to predicted entities that we own
// This works because we only predict the user's controlled entity.
// If we were predicting more entities, we would have to only apply movement to the player owned one.
pub(crate) fn movement(
    // TODO: maybe make prediction mode a separate component!!!
    mut position_queries: ParamSet<(
        Query<(&mut Position, &ActionState<Inputs>), With<Predicted>>,
        Query<(&Position, &mut Transform), With<PlayerId>>,
    )>,
) {
    // if we are not doing prediction, no need to read inputs
    if <Components as SyncMetadata<Position>>::mode() != ComponentSyncMode::Full {
        return;
    }
    for (position, input) in position_queries.p0().iter_mut() {
        shared_movement_behaviour(position, input);
    }
    for (position, mut transform) in position_queries.p1().iter_mut() {
        transform.translation = Vec3::new(position.x, position.y, transform.translation.z);
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

fn spawn_player(
    mut commands: Commands,
    players: Query<(&PlayerId, Entity), (Added<PlayerId>, Without<Confirmed>)>,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    for (player_id, entity) in players.iter() {
        let texture = asset_server.load("Universal-LPC-spritesheet-master/Universal-LPC-spritesheet-master/body/male/light.png");
        const columns: usize = 13;
        let layout = TextureAtlasLayout::from_grid(Vec2::new(64.0, 64.0), columns, 21, None, None);
        let texture_atlas_layout = texture_atlas_layouts.add(layout);
        // Use only the subset of sprites in the sheet that make up the run animation
        let animation_indices = AnimationIndices { first: columns * 2, last: columns * 2 };
        let atlas = TextureAtlas {
            layout: texture_atlas_layout.clone(),
            index: animation_indices.first,
        };
        info!("Spawning sprite");
        commands.entity(entity).insert((
            AnimationTimer(Timer::from_seconds(0.3, TimerMode::Repeating)),
            animation_indices,
            SpriteSheetBundle {
                transform: Transform::from_xyz(0., 0., 17.).with_scale(Vec3::splat(1.0)),
                texture: texture.clone(),
                atlas,
                ..default()
            },
            // IMPORTANT: this lets the server know that the entity is pre-predicted
            // when the server replicates this entity; we will get a Confirmed entity which will use this entity
            // as the Predicted version
            ShouldBePredicted::default(),
        ));
    }
}
