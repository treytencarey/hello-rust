use bevy::{ecs::entity::MapEntities, prelude::*};
use bevy_ecs_tilemap_plugin::helpers::tiled;
use bevy_mod_scripting::prelude::{CodeAsset, LuaFile, Script, ScriptCollection};
use interest_management::client::{ComponentSyncMode, ConnectionManager, Interpolated, Predicted};
use lightyear::{prelude::{server::{Replicate, SyncTarget}, AppComponentExt, ChannelDirection, NetworkRelevanceMode, ReplicationGroup}, shared::replication::network_target::NetworkTarget};
use serde::{Deserialize, Serialize};

use crate::remote_file::remotefile_modified;

// Script
#[derive(Bundle)]
pub(crate) struct ScriptBundle {
    parent: ScriptParent,
    replicate: Replicate,
    filename: ScriptFileName,
}

impl ScriptBundle {
    pub(crate) fn new(filename: String, parent: Entity) -> Self {
        let sync_target = SyncTarget {
            prediction: NetworkTarget::All,
            ..default()
        };
        let replicate = Replicate {
            sync: sync_target,
            relevance_mode: NetworkRelevanceMode::InterestManagement,
            // replicate this entity within the same replication group as the parent
            group: ReplicationGroup::default().set_id(parent.to_bits()),
            ..default()
        };
        Self {
            parent: ScriptParent(parent),
            replicate,
            filename: ScriptFileName(filename)
        }
    }
}

// 
#[derive(Default, Component, Serialize, Deserialize, Debug, PartialEq, Clone, Reflect)]
pub struct ScriptFileName(pub String);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ScriptFileHash {
    hash: String,
    file_name: ScriptFileName,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ScriptFile {
    data: Vec<u8>,
    file_name: ScriptFileName,
}

impl Default for ScriptFile {
    fn default() -> Self {
        ScriptFile {
            data: Vec::new(),
            file_name: ScriptFileName::default(),
        }
    }
}

// and deriving the `MapEntities` trait for the component.
#[derive(Component, Deserialize, Serialize, Clone, Debug, PartialEq, Reflect)]
pub struct ScriptParent(pub Entity);

impl MapEntities for ScriptParent {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.0 = entity_mapper.map_entity(self.0);
    }
}

// ################################################################################################

#[derive(Clone)]
pub struct ScriptSharedPlugin;

impl Plugin for ScriptSharedPlugin {
    fn build(&self, app: &mut App) {
        app.register_component::<ScriptFileName>(ChannelDirection::ServerToClient)
            .add_prediction(ComponentSyncMode::Once)
            .add_interpolation(ComponentSyncMode::Once);

        app.register_component::<ScriptParent>(ChannelDirection::ServerToClient)
            .add_map_entities()
            .add_prediction(ComponentSyncMode::Once)
            .add_interpolation(ComponentSyncMode::Once);
    }
}

// ################################################################################################

pub struct ScriptServerPlugin;

impl Plugin for ScriptServerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConnectionManager>();
    }
}

// ################################################################################################

pub struct ScriptClientPlugin;

impl Plugin for ScriptClientPlugin {
    fn build(&self, app: &mut App) {
        app
        .add_systems(
            Update,
        (script_spawn, remotefile_modified::<LuaFile>)
        );
    }
}

fn script_spawn(
    mut commands: Commands,
    mut script_query: Query<
        (&ScriptParent, &ScriptFileName),
        Or<(Added<Interpolated>, Added<Predicted>)>,
    >,
    asset_server: Res<AssetServer>,
) {
    for (parent, script_file_name) in &mut script_query {
        info!("Spawning script: {:?}", script_file_name.0);
        
        let handle = asset_server.load::<LuaFile>(script_file_name.0.clone());
        let script = Script::<LuaFile>::new(script_file_name.0.clone(), handle);

        commands.entity(parent.0).insert(ScriptCollection::<LuaFile> {
            scripts: vec![script],
        });
    }
}