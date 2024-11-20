use avian2d::prelude::LinearVelocity;
use bevy::prelude::*;

use leafwing_input_manager::prelude::ActionState;
pub use lightyear::prelude::client::*;
use lightyear::prelude::is_host_server;

use crate::shared::{shared_movement_behaviour, Inputs, PhysicsBundle, PlayerId};

pub struct ExampleClientPlugin;

impl Plugin for ExampleClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init);
        app.add_systems(FixedUpdate, movement.run_if(not(is_host_server)));
        app.add_systems(Update, add_player_physics);
    }
}

/// Startup system for the client
pub(crate) fn init(mut commands: Commands) {
    commands.connect_client();
}

// The client input only gets applied to predicted entities that we own
// This works because we only predict the user's controlled entity.
// If we were predicting more entities, we would have to only apply movement to the player owned one.
pub(crate) fn movement(
    // TODO: maybe make prediction mode a separate component!!!
    mut position_query: Query<(&mut LinearVelocity, &ActionState<Inputs>), With<Predicted>>,
) {
    for (velocity, input) in position_query.iter_mut() {
        shared_movement_behaviour(velocity, input);
    }
}

/// When we receive other players (whether they are predicted or interpolated), we want to add the physics components
/// so that our predicted entities can predict collisions with them correctly
fn add_player_physics(
    connection: Res<ClientConnection>,
    mut commands: Commands,
    mut player_query: Query<
        (Entity, &PlayerId),
        (
            // insert the physics components on the player that is displayed on screen
            // (either interpolated or predicted)
            Or<(Added<Interpolated>, Added<Predicted>)>,
        ),
    >,
) {
    let client_id = connection.id();
    for (entity, player_id) in player_query.iter_mut() {
        if player_id.0 == client_id {
            // only need to do this for other players' entities
            debug!(
                ?entity,
                ?player_id,
                "we only want to add physics to other player! Skip."
            );
            continue;
        }
        info!(?entity, ?player_id, "adding physics to predicted player");
        commands.entity(entity).insert(PhysicsBundle::player());
    }
}