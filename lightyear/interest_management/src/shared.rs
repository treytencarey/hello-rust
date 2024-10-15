use std::ops::{Add, Mul};

use bevy::{prelude::*, render::RenderPlugin};
use client::Confirmed;
use leafwing_input_manager::{prelude::ActionState, Actionlike};
use lightyear::prelude::*;

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Deref, DerefMut)]
pub struct Position(pub Vec2);

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerId(pub ClientId);

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Deref, DerefMut)]
pub struct LastPosition(pub Option<Vec2>);

impl Add for Position {
    type Output = Position;
    #[inline]
    fn add(self, rhs: Position) -> Position {
        Position(self.0.add(rhs.0))
    }
}

impl Mul<f32> for &Position {
    type Output = Position;

    fn mul(self, rhs: f32) -> Self::Output {
        Position(self.0 * rhs)
    }
}

// Inputs

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Reflect, Clone, Copy, Actionlike)]
pub enum Inputs {
    Up,
    Down,
    Left,
    Right,
    Delete,
    Spawn,
}

#[derive(Clone)]
pub struct SharedPlugin;

impl Plugin for SharedPlugin {
    fn build(&self, app: &mut App) {
        if app.is_plugin_added::<RenderPlugin>() {
            app.add_systems(Startup, init);
            app.add_systems(Update, draw_boxes);
        }
    }
}

fn init(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
}

// This system defines how we update the player's positions when we receive an input
pub(crate) fn shared_movement_behaviour(position: &mut Position, input: &ActionState<Inputs>) {
    const MOVE_SPEED: f32 = 10.0;
    if input.pressed(&Inputs::Up) {
        position.y += MOVE_SPEED;
    }
    if input.pressed(&Inputs::Down) {
        position.y -= MOVE_SPEED;
    }
    if input.pressed(&Inputs::Left) {
        position.x -= MOVE_SPEED;
    }
    if input.pressed(&Inputs::Right) {
        position.x += MOVE_SPEED;
    }
}

/// System that draws the boxed of the player positions.
/// The components should be replicated from the server to the client
/// This time we will only draw the predicted/interpolated entities
pub(crate) fn draw_boxes(
    mut gizmos: Gizmos,
    mut players: Query<(&Position, &mut Transform, &PlayerId), Without<Confirmed>>,
) {
    for (position, mut transform, client_id) in players.iter_mut() {
        gizmos.rect(
            Vec3::new(position.x, position.y, 0.0),
            Quat::IDENTITY,
            Vec2::ONE * 50.0,
            Color::linear_rgb(255.0, 0.0, 0.0),
        );
        transform.translation = Vec3::new(position.x, position.y, 0.0);
    }
}