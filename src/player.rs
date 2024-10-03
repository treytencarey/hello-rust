use bevy::prelude::*;
use interest_management::{client::{ClientConnection, Interpolated, NetClient, Predicted}, protocol::{AnimationIndices, AnimationTimer, Position}};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                player_spawn,
                camera_movement,
            ),
        );
    }
}

fn player_spawn(
    connection: Res<ClientConnection>,
    mut commands: Commands,
    mut character_query: Query<
        (Entity),
        (Or<(Added<Predicted>, Added<Interpolated>)>),
    >,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    for (entity) in &mut character_query {
        // spawn extra sprites, etc.
        let texture = asset_server.load("EPIC RPG World - Ancient Ruins V 1.9.1/ERW - Ancient Ruins V 1.9.1/Characters/silly luck creature-idle.png");
        let layout = TextureAtlasLayout::from_grid(UVec2::new(96, 85), 4, 1, None, None);
        let texture_atlas_layout = texture_atlas_layouts.add(layout);
        // Use only the subset of sprites in the sheet that make up the run animation
        let animation_indices = AnimationIndices { first: 0, last: 3 };
        let atlas = TextureAtlas {
            layout: texture_atlas_layout.clone(),
            index: animation_indices.first,
        };

        let client_id = connection.id();
        info!(?entity, ?client_id, "Adding animation to character");
        commands.entity(entity).insert((
            AnimationTimer(Timer::from_seconds(0.3, TimerMode::Repeating)),
            animation_indices,
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