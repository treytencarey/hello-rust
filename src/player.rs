use bevy::prelude::*;
use interest_management::{client::{ClientConnection, Interpolated, NetClient, Predicted}, protocol::{AnimationIndices, AnimationSpriteBundle, AnimationTimer, PlayerTexture, PlayerTextureAtlasLayout, Position}};

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
        (Entity, &AnimationTimer, &AnimationIndices, &AnimationSpriteBundle, &PlayerTextureAtlasLayout),
        (Or<(Added<Predicted>, Added<Interpolated>)>),
    >,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    for (entity, animation_timer, animation_indices, animation_sprite_bundle, atlas_layout) in &mut character_query {
        // spawn extra sprites, etc.
        let texture = asset_server.load(animation_sprite_bundle.texture.0.clone());
        let layout = TextureAtlasLayout::from_grid(atlas_layout.0.tile_size, atlas_layout.0.columns, atlas_layout.0.rows, None, atlas_layout.0.offset);
        let texture_atlas_layout = texture_atlas_layouts.add(layout);
        let atlas = TextureAtlas {
            layout: texture_atlas_layout.clone(),
            index: animation_indices.first,
        };

        let client_id = connection.id();
        info!(?entity, ?client_id, "Adding animation to character");
        commands.entity(entity).insert((
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