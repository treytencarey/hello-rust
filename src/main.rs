use bevy::prelude::*;

use crate::network_main::NetworkMain;
use crate::script_main::ScriptMain;
mod network_main;
mod script_main;

mod client;
mod protocol;
mod server;
mod settings;
mod shared;

fn main() {
    let mut app = App::new()
        .add_plugins((NetworkMain, ScriptMain))
        .run();
}