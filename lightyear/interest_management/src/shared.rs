use avian2d::{math::Vector, prelude::{LinearVelocity, Position}};
use bevy::{prelude::*, render::RenderPlugin};
use client::Confirmed;
use leafwing_input_manager::{prelude::ActionState, Actionlike};
use lightyear::prelude::*;

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerId(pub ClientId);

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Deref, DerefMut)]
pub struct LastPosition(pub Option<Vector>);

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
        }
    }
}

fn init(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
}