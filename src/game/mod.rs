//! Server-authoritative gameplay systems.
//!
//! Everything here runs only on the machine that owns the game (host or
//! dedicated server), except `world` which also runs on clients (terrain is
//! needed for prediction).
//!
//! One server tick (FixedUpdate, 60 Hz) runs these sets in order:
//! `Input -> Players -> Mobs -> Combat -> Damage -> World`.

pub mod autos;
pub mod chests;
pub mod combat;
pub mod lobby;
pub mod mobs;
pub mod pickups;
pub mod players;
pub mod progress;
pub mod world;

use bevy::prelude::*;

use crate::net::is_authority;
use crate::net::protocol::{MatchInfo, Phase};

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ServerSet {
    Input,
    Players,
    Mobs,
    Combat,
    Damage,
    World,
}

/// Server simulation tick counter.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct SimTick(pub u32);

/// Run condition: a run is in progress (works on server and clients).
pub fn match_playing(info: Query<&MatchInfo>) -> bool {
    info.iter().any(|m| m.phase == Phase::Playing)
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SimTick>()
            .configure_sets(
                FixedUpdate,
                (
                    ServerSet::Input,
                    ServerSet::Players,
                    ServerSet::Mobs,
                    ServerSet::Combat,
                    ServerSet::Damage,
                    ServerSet::World,
                )
                    .chain()
                    .run_if(is_authority)
                    .run_if(match_playing),
            )
            .add_plugins((
                world::WorldPlugin,
                lobby::LobbyPlugin,
                players::PlayersPlugin,
                combat::CombatPlugin,
                autos::AutosPlugin,
                mobs::MobsPlugin,
                chests::ChestsPlugin,
                progress::ProgressPlugin,
                pickups::PickupsPlugin,
            ));
    }
}
