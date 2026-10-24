//! Client-side netcode for the *local* player:
//!
//! 1. **Input**: every tick we turn the current `InputState` into a numbered
//!    `PlayerInput`, send it to the server (with the few previous ones, for
//!    redundancy) and keep it in a history buffer.
//! 2. **Prediction**: we immediately run the same simulation as the server,
//!    so our movement and shots react instantly, without waiting for the
//!    round trip.
//! 3. **Reconciliation**: when the authoritative state arrives, we restart
//!    from it and re-apply the inputs the server has not processed yet. If the
//!    result differs from what we predicted, the difference is smoothed out
//!    visually instead of snapping the camera.

use std::collections::VecDeque;

use bevy::prelude::*;
use bevy_replicon::prelude::*;

use super::protocol::{Gear, Health, InputBatch, Items, MatchInfo, Player, PlayerState};
use super::{LocalPlayer, NetRole};
use crate::game::match_playing;
use crate::game::world::TerrainRes;
use crate::shared::build::{main_params, MainWeapon};
use crate::shared::items::PlayerStats;
use crate::shared::movement::{PlayerInput, DT, TICK_HZ};
use crate::shared::sim::{simulate_player, PlayerSim, TickEvents};

/// Other entities are displayed this many ticks in the past (100 ms) so we
/// always have two snapshots to interpolate between.
pub const INTERP_TICKS: f64 = 6.0;
/// Inputs resent in every packet.
const REDUNDANCY: usize = 4;

/// Current player intentions, filled every frame by the keyboard/mouse code
/// (render::input) or by a bot.
#[derive(Resource, Debug, Default)]
pub struct InputState {
    pub move_axis: Vec2,
    pub yaw: f32,
    pub pitch: f32,
    /// Buttons currently held down.
    pub held: u16,
    /// Buttons pressed since the last tick: a quick tap between two ticks is never lost.
    pub latched: u16,
    /// Weapon slot to switch to (255 = none), consumed by the next tick.
    pub select_slot: u8,
}

impl InputState {
    pub fn press(&mut self, button: u16) {
        self.latched |= button;
    }
}

#[derive(Resource, Debug, Default)]
pub struct Prediction {
    pub active: bool,
    pub sim: PlayerSim,
    /// Position at the previous tick: rendering interpolates between the two.
    pub prev_pos: Vec3,
    pub history: VecDeque<PlayerInput>,
    pub next_seq: u32,
    /// Visual offset hiding small corrections; decays to zero.
    pub smoothing: Vec3,
    pub corrections: u32,
    /// The input of the last simulated tick (for the camera angles).
    pub last_input: PlayerInput,
    /// Our main weapon (view model, zoom).
    pub main: Option<MainWeapon>,
}

/// What happened in our predicted tick (sounds, recoil, tracers).
#[derive(Message, Clone, Debug)]
pub struct PredictedEvents(pub TickEvents);

/// Estimated current server tick on this machine.
#[derive(Resource, Debug, Default)]
pub struct ServerClock {
    pub estimate: f64,
    pub latest: u32,
    pub synced: bool,
}

impl ServerClock {
    /// The tick at which other entities are rendered.
    pub fn render_tick(&self) -> f64 {
        self.estimate - INTERP_TICKS
    }
}

pub struct ClientNetPlugin;

impl Plugin for ClientNetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputState>()
            .init_resource::<Prediction>()
            .init_resource::<ServerClock>()
            .add_message::<PredictedEvents>()
            .add_systems(PreUpdate, (sync_clock, reconcile).chain().after(ClientSystems::Receive))
            .add_systems(
                FixedUpdate,
                predict_local_player
                    .run_if(|role: Res<NetRole>| role.has_local_player())
                    .run_if(match_playing),
            )
            .add_systems(Update, decay_smoothing);
    }
}

fn sync_clock(info: Query<&MatchInfo>, mut clock: ResMut<ServerClock>, time: Res<Time<Real>>) {
    clock.estimate += time.delta_secs_f64() * TICK_HZ;
    let Ok(info) = info.single() else {
        clock.synced = false;
        return;
    };
    if info.tick > clock.latest || !clock.synced {
        clock.latest = info.tick;
    }
    let error = f64::from(clock.latest) - clock.estimate;
    if !clock.synced || error.abs() > 30.0 {
        clock.estimate = f64::from(clock.latest);
        clock.synced = true;
    } else {
        // Gently pull the estimate toward what we receive (absorbs jitter).
        clock.estimate += error * 0.05;
    }
}

/// Finds our own player entity.
fn local_entity<'a>(
    local: &LocalPlayer,
    players: impl Iterator<Item = (&'a Player, &'a PlayerState, &'a Health, &'a Items, &'a Gear)>,
) -> Option<(&'a PlayerState, &'a Health, &'a Items, &'a Gear)> {
    let slot = local.slot?;
    players.filter(|(p, ..)| p.slot == slot).map(|(_, s, h, i, g)| (s, h, i, g)).next()
}

fn reconcile(
    local: Res<LocalPlayer>,
    players: Query<(&Player, &PlayerState, &Health, &Items, &Gear)>,
    changed: Query<&Player, Changed<PlayerState>>,
    mut pred: ResMut<Prediction>,
    terrain: Res<TerrainRes>,
) {
    let Some((server, health, items, gear)) = local_entity(&local, players.iter()) else {
        pred.active = false;
        return;
    };
    if !pred.active {
        // First state of a run: start predicting from it.
        pred.active = true;
        pred.sim = server.sim.clone();
        pred.prev_pos = server.sim.mv.pos;
        pred.history.clear();
        pred.smoothing = Vec3::ZERO;
        return;
    }
    let Some(slot) = local.slot else { return };
    if !changed.iter().any(|p| p.slot == slot) {
        return;
    }

    // Forget inputs the server has already applied.
    let acked = server.sim.last_seq;
    while pred.history.front().is_some_and(|i| i.seq <= acked) {
        pred.history.pop_front();
    }
    // Replay the rest on top of the authoritative state.
    let stats = PlayerStats::from_inventory(&items.0);
    let params = main_params(gear.0.main, &gear.0.main_up, &stats);
    let mut sim = server.sim.clone();
    for input in &pred.history {
        simulate_player(&mut sim, input, &stats, &params, &terrain, health.downed, DT);
    }
    let error = pred.sim.mv.pos - sim.mv.pos;
    if error.length() > 0.001 {
        pred.corrections += 1;
    }
    if error.length() < 4.0 {
        pred.smoothing += error;
        pred.prev_pos -= error;
    } else {
        // Teleport (respawn, huge desync): snap.
        pred.smoothing = Vec3::ZERO;
        pred.prev_pos = sim.mv.pos;
    }
    pred.sim = sim;
}

#[allow(clippy::too_many_arguments)]
fn predict_local_player(
    mut pred: ResMut<Prediction>,
    mut input_state: ResMut<InputState>,
    local: Res<LocalPlayer>,
    players: Query<(&Player, &PlayerState, &Health, &Items, &Gear)>,
    terrain: Res<TerrainRes>,
    clock: Res<ServerClock>,
    mut batch: MessageWriter<InputBatch>,
    mut events: MessageWriter<PredictedEvents>,
) {
    if !pred.active {
        return;
    }
    let Some((_, health, items, gear)) = local_entity(&local, players.iter()) else { return };

    pred.next_seq += 1;
    let input = PlayerInput {
        seq: pred.next_seq,
        move_axis: input_state.move_axis.clamp_length_max(1.0),
        yaw: input_state.yaw,
        pitch: input_state.pitch,
        buttons: input_state.held | input_state.latched,
        select_slot: input_state.select_slot,
    };
    input_state.latched = 0;
    input_state.select_slot = 255;

    pred.history.push_back(input);
    while pred.history.len() > 180 {
        pred.history.pop_front();
    }
    let start = pred.history.len().saturating_sub(REDUNDANCY);
    batch.write(InputBatch {
        inputs: pred.history.iter().skip(start).copied().collect(),
        view_tick: clock.render_tick().max(0.0) as u32,
    });

    let stats = PlayerStats::from_inventory(&items.0);
    let params = main_params(gear.0.main, &gear.0.main_up, &stats);
    pred.prev_pos = pred.sim.mv.pos;
    let ev = simulate_player(&mut pred.sim, &input, &stats, &params, &terrain, health.downed, DT);
    pred.last_input = input;
    pred.main = Some(gear.0.main);
    events.write(PredictedEvents(ev));
}

fn decay_smoothing(mut pred: ResMut<Prediction>, time: Res<Time>) {
    let k = (-time.delta_secs() * 10.0).exp();
    pred.smoothing *= k;
}
