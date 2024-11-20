use avian2d::{prelude::{Collider, ColliderDensity, Gravity, LinearVelocity, Physics, PhysicsSet, Position, RigidBody}, sync::SyncPlugin, PhysicsPlugins};
use bevy::{prelude::*, render::RenderPlugin};
use client::Confirmed;
use leafwing_input_manager::{prelude::ActionState, Actionlike};
use lightyear::prelude::*;
use lightyear_examples_common::shared::FIXED_TIMESTEP_HZ;

const MAX_VELOCITY: f32 = 200.0;

#[derive(Bundle)]
pub struct PhysicsBundle {
    pub collider: Collider,
    pub collider_density: ColliderDensity,
    pub rigid_body: RigidBody,
}

impl PhysicsBundle {
    pub fn player() -> Self {
        Self {
            collider: Collider::rectangle(32.0, 32.0),
            collider_density: ColliderDensity(0.2),
            rigid_body: RigidBody::Dynamic,
        }
    }
}

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerId(pub ClientId);

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct PlayerRoom(pub u64);

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Deref, DerefMut)]
pub struct LastPosition(pub Option<Vec2>);

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

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum FixedSet {
    // main fixed update systems (handle inputs)
    Main,
    // apply physics steps
    Physics,
}

#[derive(Clone)]
pub struct SharedPlugin;

impl Plugin for SharedPlugin {
    fn build(&self, app: &mut App) {
        if app.is_plugin_added::<RenderPlugin>() {
            app.add_systems(Startup, init);
            app.add_systems(Update, draw_boxes);
        }
        
        // Physics
        //
        // we use Position and Rotation as primary source of truth, so no need to sync changes
        // from Transform->Pos, just Pos->Transform.
        app.insert_resource(avian2d::sync::SyncConfig {
            transform_to_position: false,
            position_to_transform: true,
        });
        // We change SyncPlugin to PostUpdate, because we want the visually interpreted values
        // synced to transform every time, not just when Fixed schedule runs.
        app.add_plugins(
            PhysicsPlugins::new(FixedUpdate)
                .build()
                .disable::<SyncPlugin>(),
        )
        .add_plugins(SyncPlugin::new(PostUpdate));

        app.insert_resource(Time::new_with(Physics::fixed_once_hz(FIXED_TIMESTEP_HZ)));
        app.insert_resource(Gravity(Vec2::ZERO));

        app.configure_sets(
            FixedUpdate,
            (
                // make sure that any physics simulation happens after the Main SystemSet
                // (where we apply user's actions)
                (
                    PhysicsSet::Prepare,
                    PhysicsSet::StepSimulation,
                    PhysicsSet::Sync,
                )
                    .in_set(FixedSet::Physics),
                (FixedSet::Main, FixedSet::Physics).chain(),
            ),
        );
    }
}

fn init(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
}

// This system defines how we update the player's positions when we receive an input
pub(crate) fn shared_movement_behaviour(mut velocity: Mut<LinearVelocity>, input: &ActionState<Inputs>) {
    const MOVE_SPEED: f32 = 10.0;
    if input.pressed(&Inputs::Up) {
        velocity.y += MOVE_SPEED;
    }
    if input.pressed(&Inputs::Down) {
        velocity.y -= MOVE_SPEED;
    }
    if input.pressed(&Inputs::Left) {
        velocity.x -= MOVE_SPEED;
    }
    if input.pressed(&Inputs::Right) {
        velocity.x += MOVE_SPEED;
    }
    *velocity = LinearVelocity(velocity.clamp_length_max(MAX_VELOCITY));
}

/// System that draws the boxed of the player positions.
/// The components should be replicated from the server to the client
/// This time we will only draw the predicted/interpolated entities
pub(crate) fn draw_boxes(
    mut gizmos: Gizmos,
    mut players: Query<&Position, Without<Confirmed>>,
) {
    for position in players.iter_mut() {
        gizmos.rect(
            Vec3::new(position.x, position.y, 0.0),
            Quat::IDENTITY,
            Vec2::ONE * 50.0,
            Color::linear_rgb(255.0, 0.0, 0.0),
        );
    }
}