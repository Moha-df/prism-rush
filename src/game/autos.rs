//! Automatic weapons (server): they act on their own, the player never
//! aims them. Aura, sky lightning, orbiting orbs, thrown axes, mines,
//! tornados, poison pools and a fire trail.
//!
//! Each player has an `AutoState` with one cooldown per weapon. Areas left
//! on the ground (mines, pools, flames, tornados) are replicated `Hazard`
//! entities, so clients can draw them; their damage is applied here.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy_replicon::prelude::*;

use super::combat::{DamageMob, Explode, ProjectileServer};
use super::world::TerrainRes;
use super::{ServerSet, SimTick};
use crate::net::protocol::*;
use crate::shared::build::{auto_params, AutoParams, AutoWeapon, Delivery, MainParams, MainWeapon};
use crate::shared::items::PlayerStats;
use crate::shared::movement::DT;
use crate::shared::rng::{hash_combine, Rng};

/// Monsters farther than this are ignored by automatic weapons.
const AUTO_RANGE: f32 = 28.0;
/// An orb hits the same monster at most this often (seconds).
const ORB_HIT_COOLDOWN: f32 = 0.35;
/// Radius of one orb.
pub const ORB_RADIUS: f32 = 0.45;
/// Area weapons (pools, flames, tornados) hurt this often (seconds).
const AREA_TICK: f32 = 0.4;
/// A mine explodes when a monster is this close.
const MINE_TRIGGER: f32 = 1.4;

/// Per-player state of the automatic weapons.
#[derive(Component, Default, Debug)]
pub struct AutoState {
    cooldowns: HashMap<AutoWeapon, f32>,
    /// Orbs: last time each monster was hit.
    orb_hits: HashMap<Entity, f32>,
    /// Fire trail: where the last flame was dropped.
    last_flame: Option<Vec3>,
    clock: f32,
}

/// Server data of a player's area weapon (mine, pool, flames, tornado).
#[derive(Component, Debug)]
pub struct PlayerHazard {
    pub by: Entity,
    pub damage: f32,
    pub timer: f32,
}

pub struct AutosPlugin;

impl Plugin for AutosPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, (run_autos, update_player_hazards).chain().in_set(ServerSet::Combat));
    }
}

/// Position of orb `i` of `count` around a player, at time `t` (seconds).
/// Shared with the client renderer so both agree.
pub fn orb_offset(i: u32, count: u32, radius: f32, speed: f32, t: f32) -> Vec3 {
    let a = t * speed + i as f32 / count.max(1) as f32 * std::f32::consts::TAU;
    Vec3::new(a.cos() * radius, 1.0, a.sin() * radius)
}

type AutoQuery<'a> = (Entity, &'a Player, &'a PlayerState, &'a Health, &'a Items, &'a Gear, &'a mut AutoState);

#[allow(clippy::too_many_arguments)]
fn run_autos(
    mut commands: Commands,
    mut players: Query<AutoQuery>,
    mobs: Query<(Entity, &Mob, &MobState)>,
    terrain: Res<TerrainRes>,
    tick: Res<SimTick>,
    mut damage: MessageWriter<DamageMob>,
    mut fx: MessageWriter<ToClients<Fx>>,
) {
    let time = tick.0 as f32 * DT;
    for (entity, player, state, health, items, gear, mut st) in &mut players {
        if health.downed {
            continue;
        }
        st.clock += DT;
        let clock = st.clock;
        st.orb_hits.retain(|_, t| clock - *t < 2.0);
        let stats = PlayerStats::from_inventory(&items.0);
        let me = state.sim.mv.pos;
        let mut rng = Rng::new(hash_combine(&[u64::from(tick.0), u64::from(player.slot), 0xA070]));
        // Monsters in range, with their center.
        let near: Vec<(Entity, Vec3, f32)> = mobs
            .iter()
            .map(|(e, m, s)| (e, s.pos + Vec3::Y * m.kind.def().height * 0.5, m.kind.def().radius))
            .filter(|(_, c, _)| c.distance(me) < AUTO_RANGE)
            .collect();

        for (kind, up) in &gear.0.autos {
            let p = auto_params(*kind, up, &stats);
            // Orbs touch every tick (with a per-monster cooldown).
            if *kind == AutoWeapon::Orbs {
                for i in 0..p.count {
                    let orb = me + orb_offset(i, p.count, p.size, p.speed, time);
                    for (e, c, r) in &near {
                        let hit_recently = st.orb_hits.get(e).is_some_and(|t| clock - t < ORB_HIT_COOLDOWN);
                        if !hit_recently && orb.distance(*c) < r + ORB_RADIUS + 0.3 {
                            st.orb_hits.insert(*e, clock);
                            damage.write(DamageMob { target: *e, amount: p.damage, crit: false, pos: orb, by: Some(entity) });
                        }
                    }
                }
                continue;
            }
            let cd = st.cooldowns.entry(*kind).or_insert(0.0);
            *cd -= DT;
            if *cd > 0.0 {
                continue;
            }
            let fired = fire_auto(*kind, &p, entity, player.slot, me, state.yaw, &near, &terrain, tick.0, &mut st.last_flame, &mut rng, &mut commands, &mut damage, &mut fx);
            // Nothing to shoot at: try again soon instead of waiting a full cooldown.
            let cd = st.cooldowns.entry(*kind).or_insert(0.0);
            *cd = if fired { p.cooldown } else { 0.2 };
        }
    }
}

/// Fires one automatic weapon. Returns false when it had nothing to do.
#[allow(clippy::too_many_arguments)]
fn fire_auto(
    kind: AutoWeapon,
    p: &AutoParams,
    by: Entity,
    slot: u8,
    me: Vec3,
    yaw: f32,
    near: &[(Entity, Vec3, f32)],
    terrain: &TerrainRes,
    tick: u32,
    last_flame: &mut Option<Vec3>,
    rng: &mut Rng,
    commands: &mut Commands,
    damage: &mut MessageWriter<DamageMob>,
    fx: &mut MessageWriter<ToClients<Fx>>,
) -> bool {
    let ground = |x: f32, z: f32| terrain.wgen.ground_height(x, z);
    let hazard = |kind: HazardKind, pos: Vec3, vel: Vec3, life: f32| {
        (
            Replicated,
            Hazard { kind, pos, spawn_tick: tick, radius: p.size, life, vel },
            PlayerHazard { by, damage: p.damage, timer: 0.0 },
        )
    };
    match kind {
        AutoWeapon::Aura => {
            let mut any = false;
            for (e, c, r) in near {
                let flat = Vec2::new(c.x - me.x, c.z - me.z).length();
                if flat < p.size + r && (c.y - me.y).abs() < 3.0 {
                    damage.write(DamageMob { target: *e, amount: p.damage, crit: false, pos: *c, by: Some(by) });
                    any = true;
                }
            }
            if any {
                fx.write(ToClients { targets: SendTargets::All, message: Fx::AuraPulse { slot } });
            }
            any
        }
        AutoWeapon::Lightning => {
            if near.is_empty() {
                return false;
            }
            for _ in 0..p.count {
                let (_, target, _) = *rng.pick(near);
                let pos = Vec3::new(target.x, ground(target.x, target.z), target.z);
                for (e, c, r) in near {
                    if c.distance(target) < p.size + r {
                        damage.write(DamageMob { target: *e, amount: p.damage, crit: false, pos: *c, by: Some(by) });
                    }
                }
                fx.write(ToClients { targets: SendTargets::All, message: Fx::Lightning { pos } });
            }
            true
        }
        AutoWeapon::Axes => {
            // Thrown in an arc toward monsters (or forward), they fall back down.
            let forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
            for i in 0..p.count {
                let dir = if near.is_empty() { forward } else { (rng.pick(near).1 - me).with_y(0.0).normalize_or_zero() };
                let spread = (i as f32 - (p.count as f32 - 1.0) / 2.0) * 0.35;
                let dir = Quat::from_rotation_y(spread) * dir;
                let params = MainParams {
                    kind: MainWeapon::FireStaff,
                    delivery: Delivery::Projectile,
                    damage: p.damage,
                    interval: p.cooldown,
                    count: 1,
                    spread_deg: 0.0,
                    random_spread: false,
                    speed: 0.0,
                    size: p.size,
                    range: 0.0,
                    bounces: 0,
                    pierce: u32::MAX,
                    splash: 0.0,
                    gravity: 22.0,
                    crit_chance: 0.0,
                    crit_mult: 1.0,
                    recoil: 0.0,
                };
                commands.spawn((
                    Replicated,
                    Projectile {
                        kind: ProjectileKind::Axe,
                        origin: me + Vec3::Y * 1.4,
                        vel: dir * 9.0 * p.speed + Vec3::Y * 11.0,
                        gravity: 22.0,
                        spawn_tick: tick,
                        owner_slot: slot,
                        size: p.size,
                        life: 0.0,
                    },
                    ProjectileServer::player(&params, by, 2.5, hash_combine(&[u64::from(tick), u64::from(i)])),
                ));
            }
            true
        }
        AutoWeapon::Mines => {
            for _ in 0..p.count {
                let a = rng.range(0.0, std::f32::consts::TAU);
                let d = rng.range(0.5, 3.0);
                let (x, z) = (me.x + a.cos() * d, me.z + a.sin() * d);
                commands.spawn(hazard(HazardKind::Mine, Vec3::new(x, ground(x, z), z), Vec3::ZERO, p.duration));
            }
            true
        }
        AutoWeapon::Tornado => {
            for _ in 0..p.count {
                let a = rng.range(0.0, std::f32::consts::TAU);
                let vel = Vec3::new(a.cos(), 0.0, a.sin()) * p.speed;
                commands.spawn(hazard(HazardKind::Tornado, Vec3::new(me.x, ground(me.x, me.z), me.z), vel, p.duration));
            }
            true
        }
        AutoWeapon::Poison => {
            if near.is_empty() {
                return false;
            }
            for _ in 0..p.count {
                let (_, c, _) = *rng.pick(near);
                commands.spawn(hazard(HazardKind::Poison, Vec3::new(c.x, ground(c.x, c.z), c.z), Vec3::ZERO, p.duration));
            }
            true
        }
        AutoWeapon::FireTrail => {
            let feet = Vec3::new(me.x, ground(me.x, me.z), me.z);
            if last_flame.is_some_and(|l| l.distance(feet) < p.size * 0.8) {
                return false;
            }
            *last_flame = Some(feet);
            commands.spawn(hazard(HazardKind::Fire, feet, Vec3::ZERO, p.duration));
            true
        }
        AutoWeapon::Orbs => false,
    }
}

/// Mines explode on contact; pools, flames and tornados hurt what is inside.
fn update_player_hazards(
    mut commands: Commands,
    mut hazards: Query<(Entity, &Hazard, &mut PlayerHazard)>,
    mobs: Query<(Entity, &Mob, &MobState)>,
    tick: Res<SimTick>,
    mut damage: MessageWriter<DamageMob>,
    mut explode: MessageWriter<Explode>,
) {
    let now = f64::from(tick.0);
    for (entity, hazard, mut data) in &mut hazards {
        if !hazard.is_player_weapon() {
            continue;
        }
        let age = hazard.age(now);
        if age > hazard.life {
            commands.entity(entity).try_despawn();
            continue;
        }
        let pos = hazard.pos_at(now);
        if hazard.kind == HazardKind::Mine {
            // Arms after half a second.
            let triggered = age > 0.5 && mobs.iter().any(|(_, m, s)| s.pos.distance(pos) < MINE_TRIGGER + m.kind.def().radius);
            if triggered {
                explode.write(Explode { pos: pos + Vec3::Y * 0.4, radius: hazard.radius, damage: data.damage, hurts_players: false, hurts_mobs: true, by: Some(data.by) });
                commands.entity(entity).try_despawn();
            }
            continue;
        }
        data.timer -= DT;
        if data.timer > 0.0 {
            continue;
        }
        data.timer = AREA_TICK;
        for (e, m, s) in &mobs {
            let d = Vec2::new(s.pos.x - pos.x, s.pos.z - pos.z).length();
            let h = s.pos.y - pos.y;
            if d < hazard.radius + m.kind.def().radius && h > -1.5 && h < 4.0 {
                let center = s.pos + Vec3::Y * m.kind.def().height * 0.5;
                damage.write(DamageMob { target: e, amount: data.damage, crit: false, pos: center, by: Some(data.by) });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbs_are_evenly_spread() {
        let a = orb_offset(0, 2, 2.0, 1.0, 0.0);
        let b = orb_offset(1, 2, 2.0, 1.0, 0.0);
        assert!((a + b).with_y(0.0).length() < 1e-4, "opposite sides");
        assert!((a.with_y(0.0).length() - 2.0).abs() < 1e-4);
    }
}
