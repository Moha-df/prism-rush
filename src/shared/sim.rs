//! The full per-tick player simulation = movement + main weapon.
//!
//! `PlayerSim` is the predicted / reconciled state. It is a plain struct (not
//! an ECS component) so it can be copied, compared and replayed freely.

use serde::{Deserialize, Serialize};

use super::build::MainParams;
use super::items::PlayerStats;
use super::movement::{simulate_movement, MoveEvents, MoveState, PlayerInput};
use super::weapons::{simulate_gun, Gun, WeaponEvents};
use super::worldgen::Terrain;
use bevy::math::Vec3;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlayerSim {
    pub mv: MoveState,
    pub gun: Gun,
    /// Last input sequence number applied by the server (acknowledgment).
    pub last_seq: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TickEvents {
    pub mv: MoveEvents,
    pub weapons: WeaponEvents,
}

impl PlayerSim {
    pub fn spawn_at(pos: Vec3) -> Self {
        Self { mv: MoveState::spawn_at(pos), gun: Gun::default(), last_seq: 0 }
    }

    pub fn eye(&self) -> Vec3 {
        self.mv.pos + Vec3::Y * self.mv.eye_height()
    }
}

/// Advances a player by one tick.
pub fn simulate_player(
    sim: &mut PlayerSim,
    input: &PlayerInput,
    stats: &PlayerStats,
    gun: &MainParams,
    terrain: &Terrain,
    downed: bool,
    dt: f32,
) -> TickEvents {
    let eye = sim.eye();
    let can_shoot = !downed && sim.mv.dash_timer <= 0.0;
    let weapons = simulate_gun(&mut sim.gun, input, gun, eye, can_shoot, dt);
    let mv = simulate_movement(&mut sim.mv, input, &stats.movement, terrain, downed, dt);
    sim.last_seq = input.seq;
    TickEvents { mv, weapons }
}
