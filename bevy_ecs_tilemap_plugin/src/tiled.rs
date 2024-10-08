use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

use crate::helpers::tiled;

pub struct TilesPlugin;

impl Plugin for TilesPlugin {
    fn build(&self, app: &mut App) {
        // Add your network systems, resources, etc. here
        app.add_plugins(TilemapPlugin)
            .add_plugins(tiled::TiledMapPlugin);
    }
}