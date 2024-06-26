use bevy::prelude::*;

use crate::network_main::NetworkMain;
use crate::script_main::ScriptMain;
use crate::tiles_main::TilesMain;
mod network_main;
mod script_main;
mod tiles_main;

mod client;
mod protocol;
mod server;
mod settings;
mod shared;
mod helpers;
mod player;

fn main() {
    let mut app = App::new()
        .add_plugins((NetworkMain, ScriptMain, TilesMain))
        .run();
}