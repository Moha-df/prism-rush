//! Server side of players: receiving inputs, authoritative simulation,
//! regeneration and revives.

use std::collections::VecDeque;

use bevy::prelude::*;
use bevy_replicon::prelude::*;

use super::world::TerrainRes;
use super::{ServerSet, SimTick};
use crate::net::protocol::*;
use crate::shared::items::PlayerStats;
use crate::shared::movement::{buttons, PlayerInput, DT};
use crate::shared::sim::simulate_player;
use crate::shared::weapons::Shot;

/// Who controls this player: a network client (or the host), or nobody (bot).
#[derive(Component, Clone, Copy, Debug)]
pub struct Owner {
    pub client: Option<ClientId>,
}

/// Inputs received but not simulated yet.
#[derive(Component, Default, Debug)]
pub struct InputQueue {
    pub queue: VecDeque<PlayerInput>,
    pub last_received: u32,
    /// Latest tick the client was rendering (lag compensation).
    pub view_tick: u32,
}

impl InputQueue {
    /// Adds inputs newer than anything received so far (the client resends
    /// recent inputs for redundancy, so duplicates are normal).
    pub fn push_new(&mut self, inputs: &[PlayerInput]) {
        for input in inputs {
            if input.seq > self.last_received {
                self.queue.push_back(*input);
                self.last_received = input.seq;
            }
        }
        // A client that stalled must not build an unbounded backlog.
        while self.queue.len() > 20 {
            self.queue.pop_front();
        }
    }
}

/// What the player wants to interact with this tick (chests, revives).
#[derive(Component, Default, Debug)]
pub struct Intent {
    pub interact_pressed: bool,
    pub interact_held: bool,
}

/// Per-run statistics, for the end screen.
#[derive(Component, Default, Debug, Clone)]
pub struct PlayerScore {
    pub kills: u32,
    pub damage: f32,
    pub gold_earned: u32,
    pub chests: u32,
    pub revives: u32,
    pub downs: u32,
}

/// Seconds since the player was last hurt (energy shield recharge delay).
#[derive(Component, Default, Debug)]
pub struct HurtClock(pub f32);

/// Shield starts recharging after this many seconds without damage.
pub const SHIELD_DELAY: f32 = 4.0;

/// A shot fired this tick, waiting to be resolved by the combat systems.
pub struct PendingShot {
    pub shooter: Entity,
    pub slot: u8,
    pub owner: Option<ClientId>,
    pub shot: Shot,
    pub view_tick: u32,
}

#[derive(Resource, Default)]
pub struct PendingShots(pub Vec<PendingShot>);

pub const REVIVE_RANGE: f32 = 2.6;
pub const REVIVE_TIME: f32 = 2.5;
pub const REVIVE_HP_FRACTION: f32 = 0.4;

pub struct PlayersPlugin;

impl Plugin for PlayersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingShots>()
            .add_systems(
                PreUpdate,
                receive_inputs.after(ServerSystems::Receive).run_if(in_state(ServerState::Running)),
            )
            .add_systems(FixedUpdate, simulate_players.in_set(ServerSet::Players))
            .add_systems(FixedUpdate, (regenerate, revive).in_set(ServerSet::World));
    }
}

fn receive_inputs(mut batches: MessageReader<FromClient<InputBatch>>, mut players: Query<(&Owner, &mut InputQueue)>) {
    for batch in batches.read() {
        if let Some((_, mut queue)) = players.iter_mut().find(|(o, _)| o.client == Some(batch.client_id)) {
            queue.push_new(&batch.inputs);
            queue.view_tick = batch.view_tick;
        }
    }
}

type SimQuery<'a> = (
    Entity,
    &'a Player,
    &'a Owner,
    &'a mut PlayerState,
    &'a mut InputQueue,
    &'a mut Intent,
    &'a Health,
    &'a Items,
);

/// Consumes queued inputs and runs the authoritative simulation.
fn simulate_players(
    mut players: Query<SimQuery>,
    terrain: Res<TerrainRes>,
    tick: Res<SimTick>,
    mut shots: ResMut<PendingShots>,
) {
    for (entity, player, owner, mut state, mut queue, mut intent, health, items) in &mut players {
        intent.interact_pressed = false;
        // Normally one input per tick. If the client is ahead (backlog after a
        // network hiccup), catch up a bit faster.
        let steps = if queue.queue.len() > 4 { 2 } else { 1 };
        let stats = PlayerStats::from_inventory(&items.0);
        for _ in 0..steps {
            let Some(input) = queue.queue.pop_front() else { break };
            let prev = state.sim.mv.prev_buttons;
            intent.interact_pressed |= input.held(buttons::INTERACT) && prev & buttons::INTERACT == 0;
            intent.interact_held = input.held(buttons::INTERACT);

            let state = &mut *state;
            let events = simulate_player(&mut state.sim, &input, &stats, &terrain, health.downed, DT);
            state.yaw = input.yaw;
            state.pitch = input.pitch;
            state.tick = tick.0;
            for shot in events.weapons.shots {
                shots.0.push(PendingShot {
                    shooter: entity,
                    slot: player.slot,
                    owner: owner.client,
                    shot,
                    view_tick: queue.view_tick,
                });
            }
        }
    }
}

fn regenerate(
    mut players: Query<(&mut Health, &Items, &mut HurtClock)>,
    info: Single<&MatchInfo>,
    mut last_level: Local<u32>,
) {
    // Medkit heals at every team level-up.
    let wave_cleared = info.team_level > *last_level && *last_level > 0;
    *last_level = info.team_level;
    for (mut health, items, mut clock) in &mut players {
        let stats = PlayerStats::from_inventory(&items.0);
        clock.0 += DT;
        // Energy shield.
        health.shield_max = stats.shield_max;
        if clock.0 > SHIELD_DELAY {
            health.shield = (health.shield + 25.0 * DT).min(health.shield_max);
        }
        health.shield = health.shield.min(health.shield_max);
        // Medkit: heal at the end of each wave.
        if wave_cleared && !health.downed && stats.wave_heal > 0.0 {
            health.hp = (health.hp + health.max * stats.wave_heal).min(stats.max_hp);
        }
        // Max HP can change when picking a Vital Heart: keep the ratio sane.
        if health.max != stats.max_hp {
            let gained = (stats.max_hp - health.max).max(0.0);
            health.max = stats.max_hp;
            if !health.downed {
                health.hp += gained;
            }
        }
        if !health.downed && health.hp < health.max {
            health.hp = (health.hp + stats.regen * DT).min(health.max);
        }
    }
}

/// Holding "interact" next to a downed teammate fills their revive bar.
fn revive(
    mut players: Query<(Entity, &Player, &PlayerState, &mut Health, &Intent)>,
    mut scores: Query<&mut PlayerScore>,
    mut events: MessageWriter<ToClients<GameEvent>>,
) {
    // Collect helpers first (immutable pass), then update the downed ones.
    let helpers: Vec<(Entity, Vec3)> = players
        .iter()
        .filter(|(_, _, _, h, i)| !h.downed && i.interact_held)
        .map(|(e, _, s, _, _)| (e, s.sim.mv.pos))
        .collect();

    for (_, player, state, mut health, _) in &mut players {
        if !health.downed {
            continue;
        }
        let helper = helpers.iter().find(|(_, p)| p.distance(state.sim.mv.pos) < REVIVE_RANGE);
        if let Some((helper_entity, _)) = helper {
            health.revive += DT / REVIVE_TIME;
            if health.revive >= 1.0 {
                health.downed = false;
                health.revive = 0.0;
                health.hp = health.max * REVIVE_HP_FRACTION;
                if let Ok(mut score) = scores.get_mut(*helper_entity) {
                    score.revives += 1;
                }
                info!("player {} revived", player.name);
                events.write(ToClients { targets: SendTargets::All, message: GameEvent::Revived { slot: player.slot } });
            }
        } else {
            health.revive = (health.revive - DT * 0.5).max(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(seq: u32) -> PlayerInput {
        PlayerInput { seq, ..Default::default() }
    }

    #[test]
    fn queue_ignores_duplicates_and_old_inputs() {
        let mut q = InputQueue::default();
        q.push_new(&[input(1), input(2), input(3)]);
        q.push_new(&[input(2), input(3), input(4)]); // redundant resend
        let seqs: Vec<u32> = q.queue.iter().map(|i| i.seq).collect();
        assert_eq!(seqs, vec![1, 2, 3, 4]);
        q.push_new(&[input(1)]); // late packet
        assert_eq!(q.queue.len(), 4);
    }

    #[test]
    fn queue_is_bounded() {
        let mut q = InputQueue::default();
        let many: Vec<PlayerInput> = (1..100).map(input).collect();
        q.push_new(&many);
        assert_eq!(q.queue.len(), 20);
        assert_eq!(q.queue.back().unwrap().seq, 99);
    }
}
