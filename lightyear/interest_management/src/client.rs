use bevy::prelude::*;

use leafwing_input_manager::prelude::ActionState;
pub use lightyear::prelude::client::*;

use crate::shared::{shared_movement_behaviour, Inputs, PlayerId, Position};

pub struct ExampleClientPlugin;

impl Plugin for ExampleClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init);
        app.add_systems(FixedUpdate, movement);
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
    mut position_query: Query<(Mut<Position>, Mut<Transform>, &ActionState<Inputs>), (Without<Confirmed>, With<PlayerId>)>,
) {
    for (mut position, mut transform, input) in position_query.iter_mut() {
        shared_movement_behaviour(&mut position, input);
        transform.translation = position.0.extend(0.0);
    }
}
