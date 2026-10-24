//! Combat resolution (server): main weapon volleys (projectiles that pierce,
//! bounce, explode or come back; beams; chain lightning), explosions,
//! damage, deaths and rewards.
//!
//! Systems communicate through Bevy *messages* (`DamageMob`, `DamagePlayer`,
//! `Explode`): the code that decides "something gets hurt" does not need
//! mutable access to everything that can be hurt.

use std::collections::VecDeque;

use bevy::prelude::*;
use bevy_replicon::prelude::*;

use super::players::{HurtClock, PendingShots, PlayerScore};
use super::world::TerrainRes;
use super::{ServerSet, SimTick};
use crate::net::protocol::*;
use crate::shared::economy::{self, ChestKind};
use crate::shared::items::PlayerStats;
use crate::shared::mobs::{self as mobdefs, MobKind, ZoneKind, TOPPLE_TIME};
use crate::shared::movement::{DT, PLAYER_HEIGHT};
use crate::shared::build::{Delivery, MainParams};
use crate::shared::rng::Rng;

/// Max lag compensation window, in ticks (~330 ms).
pub const MAX_REWIND_TICKS: u32 = 20;

#[derive(Message, Clone, Copy, Debug)]
pub struct DamageMob {
    pub target: Entity,
    pub amount: f32,
    pub crit: bool,
    pub pos: Vec3,
    /// Player entity responsible (for kill credit, lifesteal).
    pub by: Option<Entity>,
    /// Hit zone of a direct hit (weak point, armor, leg); `None` = body or area.
    pub zone: Option<ZoneKind>,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct DamagePlayer {
    pub target: Entity,
    pub amount: f32,
    /// Monster that hit in melee (for Thorns), if any.
    pub attacker: Option<Entity>,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct Explode {
    pub pos: Vec3,
    pub radius: f32,
    pub damage: f32,
    pub hurts_players: bool,
    pub hurts_mobs: bool,
    pub by: Option<Entity>,
}

/// A shaman heals the monsters around it.
#[derive(Message, Clone, Copy, Debug)]
pub struct HealMobs {
    pub pos: Vec3,
    pub radius: f32,
    /// Fraction of max HP healed.
    pub fraction: f32,
}

/// Server data of a monster attack area.
#[derive(Component, Debug, Default)]
pub struct HazardServer {
    pub damage: f32,
    /// Players already hit (a shockwave hits each player once).
    pub hit: Vec<Entity>,
    /// Pools: time until they hurt again.
    pub next_pulse: f32,
}

impl HazardServer {
    pub fn new(damage: f32) -> Self {
        Self { damage, ..default() }
    }
}

/// Recent positions of a monster, to check hits where the *shooter saw it*.
#[derive(Component, Default, Debug)]
pub struct MobHistory {
    samples: VecDeque<(u32, Vec3)>,
}

impl MobHistory {
    pub fn record(&mut self, tick: u32, pos: Vec3) {
        self.samples.push_back((tick, pos));
        while self.samples.len() > MAX_REWIND_TICKS as usize + 4 {
            self.samples.pop_front();
        }
    }

    /// Interpolated position at `tick` (clamped to the recorded window).
    pub fn at(&self, tick: u32, fallback: Vec3) -> Vec3 {
        let mut before: Option<(u32, Vec3)> = None;
        for &(t, p) in &self.samples {
            if t == tick {
                return p;
            }
            if t > tick {
                return match before {
                    Some((t0, p0)) => p0.lerp(p, (tick - t0) as f32 / (t - t0) as f32),
                    None => p,
                };
            }
            before = Some((t, p));
        }
        before.map(|(_, p)| p).unwrap_or(fallback)
    }
}

/// Server-only data of a projectile.
#[derive(Component, Debug, Clone)]
pub struct ProjectileServer {
    pub damage: f32,
    pub splash: f32,
    pub by: Option<Entity>,
    pub hurts_players: bool,
    pub ttl: f32,
    /// Monsters it can still go through (u32::MAX = all).
    pub pierce: u32,
    /// Times it can still jump to another monster after a hit.
    pub bounces: u32,
    pub crit_chance: f32,
    pub crit_mult: f32,
    /// Monsters already hit (a piercing projectile hurts each one once).
    pub hit: Vec<Entity>,
    pub seed: u64,
    /// Position at the previous tick (hits are swept along the segment, so
    /// fast bullets cannot tunnel through monsters).
    pub prev: Option<Vec3>,
    /// Boomerang: already on its way back.
    pub returning: bool,
}

impl ProjectileServer {
    /// A monster projectile.
    pub fn enemy(damage: f32, splash: f32, ttl: f32) -> Self {
        Self {
            damage,
            splash,
            by: None,
            hurts_players: true,
            ttl,
            pierce: 0,
            bounces: 0,
            crit_chance: 0.0,
            crit_mult: 1.0,
            hit: Vec::new(),
            seed: 0,
            prev: None,
            returning: false,
        }
    }

    /// A projectile of a player's weapon.
    pub fn player(p: &MainParams, by: Entity, ttl: f32, seed: u64) -> Self {
        Self {
            damage: p.damage,
            splash: p.splash,
            by: Some(by),
            hurts_players: false,
            ttl,
            pierce: p.pierce,
            bounces: p.bounces,
            crit_chance: p.crit_chance,
            crit_mult: p.crit_mult,
            hit: Vec::new(),
            seed,
            prev: None,
            returning: false,
        }
    }
}

/// The monster query shared by the combat systems.
type MobQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Mob, &'static MobState, &'static MobHistory, Option<&'static MobParts>, &'static Health)>;

fn mob_center(mob: &Mob, state: &MobState) -> Vec3 {
    state.pos + Vec3::Y * mob.kind.def().height * 0.5
}

/// Damage of one hit, with a possible critical.
fn roll_hit(base: f32, crit_chance: f32, crit_mult: f32, rng: &mut Rng) -> (f32, bool) {
    if rng.chance(crit_chance) { (base * crit_mult, true) } else { (base, false) }
}

/// Item effects on a direct hit: Tesla coil arcs and unstable explosions.
#[allow(clippy::too_many_arguments)]
fn item_procs(
    stats: &PlayerStats,
    rng: &mut Rng,
    target: Entity,
    at: Vec3,
    base: f32,
    by: Entity,
    mobs: &MobQuery,
    damage: &mut MessageWriter<DamageMob>,
    explode: &mut MessageWriter<Explode>,
    fx: &mut MessageWriter<ToClients<Fx>>,
) {
    if rng.chance(stats.tesla_chance) {
        let mut others: Vec<(Entity, Vec3)> = mobs
            .iter()
            .filter(|(e, ..)| *e != target)
            .map(|(e, m, st, ..)| (e, mob_center(m, st)))
            .filter(|(_, pos)| pos.distance(at) < 9.0)
            .collect();
        others.sort_by(|a, b| a.1.distance(at).total_cmp(&b.1.distance(at)));
        for (e, pos) in others.into_iter().take(2) {
            damage.write(DamageMob { target: e, amount: base * 0.6, crit: false, pos, by: Some(by), zone: None });
            fx.write(ToClients { targets: SendTargets::All, message: Fx::Arc { from: at, to: pos } });
        }
    }
    if rng.chance(stats.explode_chance) {
        explode.write(Explode { pos: at, radius: 3.0, damage: base * 0.6, hurts_players: false, hurts_mobs: true, by: Some(by) });
    }
}

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<DamageMob>()
            .add_message::<HealMobs>()
            .add_message::<DamagePlayer>()
            .add_message::<Explode>()
            .add_systems(FixedUpdate, (resolve_shots, update_projectiles, update_hazards).chain().in_set(ServerSet::Combat))
            .add_systems(
                FixedUpdate,
                (apply_explosions, apply_heals, apply_mob_damage, apply_player_damage).chain().in_set(ServerSet::Damage),
            );
    }
}

/// Distance along a ray to a vertical capsule (segment `a`-`b`, radius `r`).
pub fn ray_capsule(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, r: f32) -> Option<f32> {
    // Closest points between the ray and the segment (classic formulation).
    let u = dir;
    let v = b - a;
    let w = origin - a;
    let (uu, uv, vv, uw, vw) = (u.dot(u), u.dot(v), v.dot(v), u.dot(w), v.dot(w));
    let denom = uu * vv - uv * uv;
    // Parameter on the segment of the closest approach, clamped to the segment.
    // Degenerate cases must be handled explicitly: a zero-length segment (the
    // capsule is a sphere, e.g. small monsters) would give 0/0 = NaN, and NaN
    // makes every comparison below false => a "hit" on every such monster.
    let s = if vv < 1e-8 {
        0.0
    } else if denom.abs() < 1e-6 {
        (vw / vv).clamp(0.0, 1.0)
    } else {
        ((uu * vw - uv * uw) / denom).clamp(0.0, 1.0)
    };
    // ...then the matching (non-negative) distance along the ray.
    let t = ((a + v * s - origin).dot(u) / uu).max(0.0);
    let closest_ray = origin + u * t;
    let closest_seg = a + v * s;
    let d2 = closest_ray.distance_squared(closest_seg);
    if !d2.is_finite() || d2 > r * r {
        return None;
    }
    // Step back to the entry point of the sphere around the closest point.
    Some((t - (r * r - d2).sqrt()).max(0.0))
}

/// Hit volume of a monster whose feet are at `pos`.
#[cfg(test)]
fn mob_capsule(kind: MobKind, pos: Vec3) -> (Vec3, Vec3, f32) {
    capsule_squashed(kind, pos, 1.0)
}

fn capsule_squashed(kind: MobKind, pos: Vec3, squash: f32) -> (Vec3, Vec3, f32) {
    let d = kind.def();
    let h = d.height * squash;
    let r = d.radius.min(h * 0.5);
    (pos + Vec3::Y * r, pos + Vec3::Y * (h - r).max(r), r)
}

/// Where to aim at a monster (bots): a leg still standing, else a weak
/// point visible from the front, else the middle of the body.
pub fn aim_point(mob: &Mob, state: &MobState, parts: Option<&MobParts>) -> Vec3 {
    let broken = parts.map(|p| p.broken).unwrap_or(0);
    let toppled = state.anim == MobAnim::Toppled;
    let squash = if toppled { mob.kind.topple_squash() } else { 1.0 };
    let usable = |z: &&mobdefs::HitZone| match z.kind {
        ZoneKind::Leg(i) => !toppled && broken & (1 << i) == 0,
        ZoneKind::Weak(_) => z.offset.z <= 0.0,
        ZoneKind::Armor(_) => false,
    };
    match mob.kind.zones().iter().find(usable) {
        Some(z) => mobdefs::zone_center(z, state.pos, state.yaw, squash),
        None => state.pos + Vec3::Y * mob.kind.def().height * squash * 0.5,
    }
}

/// A zone counts if the ray enters it at most this much after the body:
/// weak points are a bit forgiving.
const ZONE_SLACK: f32 = 0.3;

/// Direct hit test against a monster at `pos`: its body capsule and its hit
/// zones (weak points, shields, legs). Returns the distance along the ray
/// (`dir` is a unit vector) and the zone hit (`None` = the body).
/// Burrowed monsters cannot be hit.
pub fn ray_mob(
    mob: &Mob,
    state: &MobState,
    parts: Option<&MobParts>,
    pos: Vec3,
    origin: Vec3,
    dir: Vec3,
    pad: f32,
) -> Option<(f32, Option<ZoneKind>)> {
    if state.anim == MobAnim::Burrowed {
        return None;
    }
    let toppled = state.anim == MobAnim::Toppled;
    let squash = if toppled { mob.kind.topple_squash() } else { 1.0 };
    // Elites are drawn 1.3x bigger: their zones follow.
    let scale = if mob.elite { 1.3 } else { 1.0 };
    let (a, b, r) = capsule_squashed(mob.kind, pos, squash);
    let body = ray_capsule(origin, dir, a, b, r + pad);
    let broken = parts.map(|p| p.broken).unwrap_or(0);
    let mut best: Option<(f32, ZoneKind)> = None;
    for z in mob.kind.zones() {
        if let ZoneKind::Leg(i) = z.kind
            && (toppled || broken & (1 << i) != 0)
        {
            continue;
        }
        let center = pos + mobdefs::zone_center(z, Vec3::ZERO, state.yaw, squash) * scale;
        if let Some(t) = mobdefs::ray_sphere(origin, dir, center, z.radius * scale + pad)
            && best.is_none_or(|(bt, _)| t < bt)
        {
            best = Some((t, z.kind));
        }
    }
    match (body, best) {
        (Some(bt), Some((zt, kind))) if zt <= bt + ZONE_SLACK => Some((zt, Some(kind))),
        (Some(bt), _) => Some((bt, None)),
        (None, Some((zt, kind))) => Some((zt, Some(kind))),
        (None, None) => None,
    }
}

/// Turns the volleys fired this tick into projectiles, beams and chains.
#[allow(clippy::too_many_arguments)]
fn resolve_shots(
    mut pending: ResMut<PendingShots>,
    mut commands: Commands,
    mobs: MobQuery,
    terrain: Res<TerrainRes>,
    tick: Res<SimTick>,
    shooters: Query<(&Items, &Health), With<Player>>,
    mut damage: MessageWriter<DamageMob>,
    mut explode: MessageWriter<Explode>,
    mut fx: MessageWriter<ToClients<Fx>>,
) {
    for p in pending.0.drain(..) {
        let shot = p.shot;
        let prm = p.params;
        let Ok((items, own)) = shooters.get(p.shooter) else { continue };
        let stats = PlayerStats::from_inventory(&items.0);
        let own_health = own.hp / own.max.max(1.0);
        let mut rng = Rng::new(shot.seed ^ 0x5EED_CAFE);
        // Everyone but the shooter, who predicted the visual locally.
        let others = match p.owner {
            Some(id) => SendTargets::AllExcept(id),
            None => SendTargets::All,
        };
        let health_of = |e: Entity| mobs.get(e).map(|(.., h)| h.hp / h.max.max(1.0)).unwrap_or(1.0);

        match prm.delivery {
            Delivery::Projectile | Delivery::Boomerang => {
                let kind = ProjectileKind::of_main(prm.kind);
                let origin = shot.origin + shot.dir * 0.6 + Vec3::NEG_Y * 0.15;
                let (vel, life, ttl) = if prm.delivery == Delivery::Boomerang {
                    // `speed` is a multiplier of the round-trip pace.
                    let life = 1.3 / prm.speed.max(0.2);
                    (shot.dir * prm.range, life, life)
                } else {
                    (shot.dir * prm.speed, 0.0, prm.range / prm.speed.max(1.0))
                };
                commands.spawn((
                    Replicated,
                    Projectile {
                        kind,
                        origin,
                        vel,
                        gravity: prm.gravity,
                        spawn_tick: tick.0,
                        owner_slot: p.slot,
                        size: prm.size,
                        life,
                    },
                    ProjectileServer::player(&prm, p.shooter, ttl, shot.seed),
                ));
            }
            Delivery::Beam => {
                // Lag compensation: test against monsters where the shooter saw them.
                let view_tick = p.view_tick.clamp(tick.0.saturating_sub(MAX_REWIND_TICKS), tick.0);
                let wall = terrain.raycast(shot.origin, shot.dir, prm.range).unwrap_or(prm.range);
                fx.write(ToClients {
                    targets: others,
                    message: Fx::Tracer { slot: p.slot, weapon: prm.kind, from: shot.origin, to: shot.origin + shot.dir * wall, size: prm.size },
                });
                for (entity, mob, state, history, parts, _) in &mobs {
                    let at = history.at(view_tick, state.pos);
                    if let Some((t, zone)) = ray_mob(mob, state, parts, at, shot.origin, shot.dir, prm.size * 0.5)
                        && t < wall
                    {
                        let pos = shot.origin + shot.dir * t;
                        let (amount, crit) = roll_hit(prm.damage, prm.crit_chance, prm.crit_mult, &mut rng);
                        let amount = amount * stats.conditional_damage(own_health, health_of(entity));
                        damage.write(DamageMob { target: entity, amount, crit, pos, by: Some(p.shooter), zone });
                    }
                }
            }
            Delivery::Chain => {
                let view_tick = p.view_tick.clamp(tick.0.saturating_sub(MAX_REWIND_TICKS), tick.0);
                let wall = terrain.raycast(shot.origin, shot.dir, prm.range).unwrap_or(prm.range);
                // First target: on the aim line, or the monster closest to it
                // (a little aim assist: lightning "seeks" its target).
                let mut first: Option<(Entity, f32)> = None;
                for (entity, mob, state, history, parts, _) in &mobs {
                    if state.anim == MobAnim::Burrowed {
                        continue;
                    }
                    let pos = history.at(view_tick, state.pos);
                    let center = pos + Vec3::Y * mob.kind.def().height * 0.5;
                    let to = center - shot.origin;
                    let dist = to.length();
                    if dist > wall.max(4.0) + 1.0 || dist < 0.1 {
                        continue;
                    }
                    let angle = to.angle_between(shot.dir);
                    let on_line = ray_mob(mob, state, parts, pos, shot.origin, shot.dir, prm.size * 0.4).is_some();
                    let score = if on_line { 0.0 } else { angle };
                    if (on_line || angle < 0.14) && first.is_none_or(|(_, s)| score < s) {
                        first = Some((entity, score));
                    }
                }
                let Some((mut target, _)) = first else {
                    let to = shot.origin + shot.dir * wall.min(12.0);
                    fx.write(ToClients { targets: SendTargets::All, message: Fx::Arc { from: shot.origin + Vec3::NEG_Y * 0.3, to } });
                    continue;
                };
                let mut from = shot.origin + Vec3::NEG_Y * 0.3;
                let mut hit = vec![target];
                let reach = 8.0 * prm.size.max(0.3);
                for jump in 0..=prm.bounces {
                    let Ok((_, mob, state, ..)) = mobs.get(target) else { break };
                    let at = mob_center(mob, state);
                    fx.write(ToClients { targets: SendTargets::All, message: Fx::Arc { from, to: at } });
                    let (amount, crit) = roll_hit(prm.damage, prm.crit_chance, prm.crit_mult, &mut rng);
                    let amount = amount * stats.conditional_damage(own_health, health_of(target));
                    damage.write(DamageMob { target, amount, crit, pos: at, by: Some(p.shooter), zone: None });
                    if jump == 0 {
                        item_procs(&stats, &mut rng, target, at, prm.damage, p.shooter, &mobs, &mut damage, &mut explode, &mut fx);
                    }
                    // Next: the nearest monster not hit yet.
                    let next = mobs
                        .iter()
                        .filter(|(e, ..)| !hit.contains(e))
                        .map(|(e, m, st, ..)| (e, mob_center(m, st).distance(at)))
                        .filter(|(_, d)| *d < reach)
                        .min_by(|a, b| a.1.total_cmp(&b.1));
                    let Some((n, _)) = next else { break };
                    hit.push(n);
                    from = at;
                    target = n;
                }
            }
        }
    }
}

/// Is `pos` inside a solid block or under the ground?
fn in_terrain(terrain: &TerrainRes, pos: Vec3) -> bool {
    if pos.y <= terrain.wgen.ground_height(pos.x, pos.z) {
        return true;
    }
    let mut solid = false;
    terrain.cache.for_each_solid_near(pos, |s| {
        let (bottom, top) = s.y_range();
        if !solid && pos.y >= bottom && pos.y <= top && s.push_out_xz(Vec2::new(pos.x, pos.z), 0.15).is_some() {
            solid = true;
        }
    });
    solid
}

#[allow(clippy::too_many_arguments)]
fn update_projectiles(
    mut commands: Commands,
    mut projectiles: Query<(Entity, &Projectile, &mut ProjectileServer)>,
    mobs: MobQuery,
    players: Query<(Entity, &PlayerState, &Health)>,
    shooters: Query<(&Items, &Health), With<Player>>,
    terrain: Res<TerrainRes>,
    tick: Res<SimTick>,
    mut damage_mob: MessageWriter<DamageMob>,
    mut damage_player: MessageWriter<DamagePlayer>,
    mut explode: MessageWriter<Explode>,
    mut fx: MessageWriter<ToClients<Fx>>,
) {
    for (entity, proj, mut data) in &mut projectiles {
        data.ttl -= DT;
        let now = f64::from(tick.0);
        let pos = proj.pos_at(now);
        let prev = data.prev.unwrap_or_else(|| proj.pos_at((now - 1.0).max(f64::from(proj.spawn_tick))));
        data.prev = Some(pos);
        let blocked = !proj.kind.ghostly() && in_terrain(&terrain, pos);

        // --- Monster projectiles: simple contact test against players ---
        if data.hurts_players {
            let mut direct: Option<Entity> = None;
            for (pe, ps, health) in &players {
                let center = ps.sim.mv.pos + Vec3::Y * (PLAYER_HEIGHT * 0.5);
                if !health.downed && (pos - center).length() < proj.size + 0.6 {
                    direct = Some(pe);
                    break;
                }
            }
            if direct.is_none() && !blocked && data.ttl > 0.0 {
                continue;
            }
            commands.entity(entity).try_despawn();
            if data.splash > 0.0 {
                explode.write(Explode { pos, radius: data.splash, damage: data.damage, hurts_players: true, hurts_mobs: false, by: None });
            } else if let Some(target) = direct {
                damage_player.write(DamagePlayer { target, amount: data.damage, attacker: None });
            }
            continue;
        }

        // --- Player projectiles: swept test along this tick's segment ---
        if proj.kind == ProjectileKind::Banana && !data.returning && proj.age(now) > proj.life * 0.5 {
            // On the way back it can hit everything once more.
            data.returning = true;
            data.hit.clear();
        }
        let seg = pos - prev;
        let len = seg.length();
        let dir = if len > 1e-4 { seg / len } else { Vec3::Y };
        let mut hits: Vec<(f32, Entity, Option<ZoneKind>)> = mobs
            .iter()
            .filter(|(e, ..)| !data.hit.contains(e))
            .filter_map(|(e, mob, state, _, parts, _)| {
                ray_mob(mob, state, parts, state.pos, prev, dir, proj.size)
                    .filter(|(t, _)| *t <= len.max(proj.size))
                    .map(|(t, zone)| (t, e, zone))
            })
            .collect();
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));

        let by = data.by;
        let shooter = by.and_then(|b| shooters.get(b).ok());
        let stats = shooter.map(|(i, _)| PlayerStats::from_inventory(&i.0));
        let own_health = shooter.map(|(_, h)| h.hp / h.max.max(1.0)).unwrap_or(1.0);
        let mut rng = Rng::new(data.seed ^ u64::from(tick.0));
        let mut done = false;
        for (t, target, zone) in hits {
            let at = prev + dir * t;
            data.hit.push(target);
            if data.splash > 0.0 {
                explode.write(Explode { pos: at, radius: data.splash, damage: data.damage, hurts_players: false, hurts_mobs: true, by });
                done = true;
                break;
            }
            let target_health = mobs.get(target).map(|(.., h)| h.hp / h.max.max(1.0)).unwrap_or(1.0);
            let (amount, crit) = roll_hit(data.damage, data.crit_chance, data.crit_mult, &mut rng);
            let amount = amount * stats.map(|s| s.conditional_damage(own_health, target_health)).unwrap_or(1.0);
            damage_mob.write(DamageMob { target, amount, crit, pos: at, by, zone });
            if let (Some(s), Some(b)) = (stats.as_ref(), by) {
                item_procs(s, &mut rng, target, at, data.damage, b, &mobs, &mut damage_mob, &mut explode, &mut fx);
            }
            if data.bounces > 0 {
                // Ricochet toward the nearest monster not hit yet.
                let next = mobs
                    .iter()
                    .filter(|(e, ..)| !data.hit.contains(e))
                    .map(|(_, m, st, ..)| mob_center(m, st))
                    .filter(|c| c.distance(at) < 18.0)
                    .min_by(|a, b| a.distance(at).total_cmp(&b.distance(at)));
                if let Some(c) = next {
                    let speed = proj.vel.length().max(20.0);
                    let mut bounced = data.clone();
                    bounced.bounces -= 1;
                    bounced.prev = None;
                    bounced.ttl = 1.0;
                    commands.spawn((
                        Replicated,
                        Projectile { origin: at, vel: (c - at).normalize_or_zero() * speed, gravity: 0.0, spawn_tick: tick.0, ..*proj },
                        bounced,
                    ));
                }
                done = true;
                break;
            }
            if data.pierce == 0 {
                done = true;
                break;
            }
            data.pierce = data.pierce.saturating_sub(1);
        }
        if done || blocked || data.ttl <= 0.0 {
            if !done && blocked && data.splash > 0.0 {
                explode.write(Explode { pos, radius: data.splash, damage: data.damage, hurts_players: false, hurts_mobs: true, by });
            }
            commands.entity(entity).try_despawn();
        }
    }
}

/// Boss attack areas: expanding shockwaves (jump over them) and delayed
/// lightning strikes (get out of the marked circle).
fn update_hazards(
    mut commands: Commands,
    mut hazards: Query<(Entity, &Hazard, &mut HazardServer)>,
    players: Query<(Entity, &PlayerState, &Health)>,
    terrain: Res<TerrainRes>,
    tick: Res<SimTick>,
    mut damage: MessageWriter<DamagePlayer>,
    mut explode: MessageWriter<Explode>,
    mut fx: MessageWriter<ToClients<Fx>>,
) {
    for (entity, hazard, mut data) in &mut hazards {
        let age = tick.0.saturating_sub(hazard.spawn_tick) as f32 * DT;
        match hazard.kind {
            // Player weapons: see `autos.rs`.
            HazardKind::Mine | HazardKind::Poison | HazardKind::Fire | HazardKind::Tornado => {}
            HazardKind::Shockwave => {
                let radius = age * SHOCKWAVE_SPEED;
                if radius > SHOCKWAVE_MAX_RADIUS {
                    commands.entity(entity).try_despawn();
                    continue;
                }
                for (pe, ps, health) in &players {
                    if health.downed || data.hit.contains(&pe) {
                        continue;
                    }
                    let p = ps.sim.mv.pos;
                    let d = Vec2::new(p.x - hazard.pos.x, p.z - hazard.pos.z).length();
                    // Only hurts players touching the ground: a jump dodges it.
                    let height = p.y - terrain.wgen.ground_height(p.x, p.z);
                    if (d - radius).abs() < 1.3 && height < 1.0 {
                        data.hit.push(pe);
                        damage.write(DamagePlayer { target: pe, amount: data.damage, attacker: None });
                    }
                }
            }
            HazardKind::Blast => {
                if age >= hazard.life {
                    explode.write(Explode {
                        pos: hazard.pos + Vec3::Y * 0.5,
                        radius: hazard.radius,
                        damage: data.damage,
                        hurts_players: true,
                        hurts_mobs: false,
                        by: None,
                    });
                    commands.entity(entity).try_despawn();
                }
            }
            HazardKind::AcidPool | HazardKind::LavaPool => {
                if age >= hazard.life {
                    commands.entity(entity).try_despawn();
                    continue;
                }
                data.next_pulse -= DT;
                if data.next_pulse > 0.0 {
                    continue;
                }
                data.next_pulse = 0.5;
                for (pe, ps, health) in &players {
                    let p = ps.sim.mv.pos;
                    let d = Vec2::new(p.x - hazard.pos.x, p.z - hazard.pos.z).length();
                    if !health.downed && d < hazard.radius && (p.y - hazard.pos.y).abs() < 2.0 {
                        damage.write(DamagePlayer { target: pe, amount: data.damage, attacker: None });
                    }
                }
            }
            HazardKind::Lightning => {
                if age >= LIGHTNING_DELAY {
                    explode.write(Explode {
                        pos: hazard.pos + Vec3::Y * 0.5,
                        radius: LIGHTNING_RADIUS,
                        damage: data.damage,
                        hurts_players: true,
                        hurts_mobs: false,
                        by: None,
                    });
                    fx.write(ToClients { targets: SendTargets::All, message: Fx::Lightning { pos: hazard.pos } });
                    commands.entity(entity).try_despawn();
                }
            }
        }
    }
}

fn apply_heals(mut heals: MessageReader<HealMobs>, mut mobs: Query<(&MobState, &mut Health), With<Mob>>) {
    for h in heals.read() {
        for (state, mut health) in &mut mobs {
            if health.hp > 0.0 && state.pos.distance(h.pos) < h.radius {
                health.hp = (health.hp + health.max * h.fraction).min(health.max);
            }
        }
    }
}

fn apply_explosions(
    mut explosions: MessageReader<Explode>,
    mobs: Query<(Entity, &Mob, &MobState)>,
    players: Query<(Entity, &PlayerState, &Health)>,
    mut damage_mob: MessageWriter<DamageMob>,
    mut damage_player: MessageWriter<DamagePlayer>,
    mut fx: MessageWriter<ToClients<Fx>>,
) {
    for ex in explosions.read() {
        fx.write(ToClients { targets: SendTargets::All, message: Fx::Explosion { pos: ex.pos, radius: ex.radius } });
        // Linear falloff: full damage at the center, 30% at the edge.
        let scale = |d: f32| 1.0 - 0.7 * (d / ex.radius).clamp(0.0, 1.0);
        if ex.hurts_mobs {
            for (entity, mob, state) in &mobs {
                if state.anim == MobAnim::Burrowed {
                    continue;
                }
                let center = state.pos + Vec3::Y * mob.kind.def().height * 0.5;
                let d = (center - ex.pos).length() - mob.kind.def().radius;
                if d < ex.radius {
                    damage_mob.write(DamageMob {
                        target: entity,
                        amount: ex.damage * scale(d.max(0.0)),
                        crit: false,
                        pos: center,
                        by: ex.by,
                        zone: None,
                    });
                }
            }
        }
        if ex.hurts_players {
            for (entity, state, health) in &players {
                if health.downed {
                    continue;
                }
                let d = (state.sim.mv.pos + Vec3::Y * 0.9 - ex.pos).length();
                if d < ex.radius {
                    damage_player.write(DamagePlayer { target: entity, amount: ex.damage * scale(d), attacker: None });
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_mob_damage(
    mut hits: MessageReader<DamageMob>,
    mut commands: Commands,
    mut mobs: Query<(&Mob, &mut Health, &MobState, &mut super::mobs::MobAi, Option<&mut MobParts>), Without<Player>>,
    mut players: Query<(&Player, &mut Health, &mut Wallet, &Items, &mut PlayerScore, &mut PlayerState), Without<Mob>>,
    mut info: Single<&mut MatchInfo>,
    tick: Res<SimTick>,
    gems: Query<(), With<XpGem>>,
    mut fx: MessageWriter<ToClients<Fx>>,
    mut events: MessageWriter<ToClients<GameEvent>>,
) {
    let gem_count = gems.iter().count();
    for hit in hits.read() {
        let Ok((mob, mut health, mob_state, mut ai, parts)) = mobs.get_mut(hit.target) else { continue };
        if health.hp <= 0.0 || mob_state.anim == MobAnim::Burrowed {
            continue; // already dead this tick, or underground
        }
        // Hit zones: weak points, shells and shields, breakable legs.
        let toppled = mob_state.anim == MobAnim::Toppled;
        let body = mob.kind.body_mult(toppled);
        let mult = match hit.zone {
            None => body,
            Some(ZoneKind::Weak(m)) => m.max(body),
            Some(ZoneKind::Armor(m)) => m,
            Some(ZoneKind::Leg(i)) => {
                if let Some(mut parts) = parts
                    && !toppled
                    && parts.broken & (1 << i) == 0
                {
                    let leg = &mut ai.leg_hp[i as usize];
                    *leg -= hit.amount;
                    if *leg <= 0.0 {
                        parts.broken |= 1 << i;
                        fx.write(ToClients { targets: SendTargets::All, message: Fx::PartBroken { pos: hit.pos } });
                        if parts.broken == 0b1111 {
                            // All legs broken: the boss collapses, no resistance left.
                            ai.down_timer = TOPPLE_TIME;
                            events.write(ToClients { targets: SendTargets::All, message: GameEvent::BossToppled { kind: mob.kind } });
                            info!("{:?} toppled!", mob.kind);
                        }
                    }
                }
                // Hitting a leg still hurts the boss a little.
                0.4
            }
        };
        let amount = hit.amount * mult;
        let zone_code = if mult > 1.01 { 1 } else if mult < 0.5 { 2 } else { 0 };
        let dealt = amount.min(health.hp);
        health.hp -= amount;
        let killed = health.hp <= 0.0;

        let mut by_slot = 255;
        if let Some(by) = hit.by
            && let Ok((player, mut ph, _, items, mut score, mut state)) = players.get_mut(by)
        {
            by_slot = player.slot;
            score.damage += dealt;
            // Lifesteal.
            let s = PlayerStats::from_inventory(&items.0);
            if s.lifesteal > 0.0 && !ph.downed {
                ph.hp = (ph.hp + dealt * s.lifesteal).min(ph.max);
            }
            if killed {
                score.kills += 1;
                if s.bloodlust > 0.0 {
                    state.sim.mv.haste = state.sim.mv.haste.max(s.bloodlust);
                }
            }
        }
        fx.write(ToClients {
            targets: SendTargets::All,
            message: Fx::Hit { pos: hit.pos, amount: amount.round() as u16, crit: hit.crit, killed, by_slot, zone: zone_code },
        });

        if killed {
            commands.entity(hit.target).try_despawn();
            // Every player gets paid (see economy.rs for why).
            for (_, _, mut wallet, items, mut score, _) in &mut players {
                let s = PlayerStats::from_inventory(&items.0);
                let elite = if mob.elite { crate::game::mobs::ELITE_HP } else { 1.0 };
                let gold = economy::kill_reward(mob.kind.def().gold * elite, info.difficulty, s.gold_mult);
                wallet.gold += gold;
                score.gold_earned += gold;
            }
            let mut drop_rng = Rng::new(u64::from(tick.0) ^ hit.target.to_bits());
            crate::game::pickups::maybe_drop_powerup(&mut commands, mob_state.pos, mob.elite, mob.kind.is_boss(), &mut drop_rng);
            // XP gem for the team (elites and bosses are worth more).
            let xp = mob.kind.def().gold * if mob.elite { 3.0 } else { 1.0 } * if mob.kind.is_boss() { 5.0 } else { 1.0 };
            crate::game::progress::drop_gem(&mut commands, gem_count, mob_state.pos, xp, &mut info);
            // Splits, acid pools, delayed explosions...
            crate::game::mobs::on_mob_death(&mut commands, mob, mob_state.pos, tick.0);
            if mob.kind.is_boss() {
                // Bosses leave a legendary chest behind.
                crate::game::chests::spawn_loot_chest(&mut commands, ChestKind::Legendary, mob_state.pos, u64::from(tick.0));
                info.boss_alive = false;
                events.write(ToClients { targets: SendTargets::All, message: GameEvent::BossKilled });
                info!("boss killed!");
            }
        }
    }
}

fn apply_player_damage(
    mut hits: MessageReader<DamagePlayer>,
    mut players: Query<(&Player, &mut Health, &mut Items, &mut PlayerScore, &PlayerState, &mut HurtClock)>,
    mut damage_mob: MessageWriter<DamageMob>,
    mut fx: MessageWriter<ToClients<Fx>>,
    mut events: MessageWriter<ToClients<GameEvent>>,
) {
    for hit in hits.read() {
        let Ok((player, mut health, mut items, mut score, state, mut clock)) = players.get_mut(hit.target) else { continue };
        if health.downed {
            continue;
        }
        let stats = PlayerStats::from_inventory(&items.0);
        // Phase step: untouchable while dashing.
        if stats.dash_invulnerable && state.sim.mv.dash_timer > 0.0 {
            continue;
        }
        clock.0 = 0.0;
        let mut amount = stats.mitigate(hit.amount);
        // Thorns: melee attackers take part of the damage back.
        if let Some(attacker) = hit.attacker
            && stats.thorns > 0.0
        {
            damage_mob.write(DamageMob {
                target: attacker,
                amount: hit.amount * stats.thorns,
                crit: false,
                pos: state.sim.mv.pos + Vec3::Y,
                by: None,
                zone: None,
            });
        }
        // The energy shield absorbs first.
        let absorbed = amount.min(health.shield);
        health.shield -= absorbed;
        amount -= absorbed;
        health.hp -= amount;
        fx.write(ToClients {
            targets: SendTargets::All,
            message: Fx::PlayerHurt { slot: player.slot, amount: (amount + absorbed).round() as u16 },
        });
        if health.hp <= 0.0 {
            // An Ankh saves you once.
            if items.0.consume(crate::shared::items::ItemKind::Ankh) {
                health.hp = health.max * 0.5;
                info!("{} was saved by an Ankh", player.name);
                events.write(ToClients { targets: SendTargets::All, message: GameEvent::AnkhUsed { slot: player.slot } });
                continue;
            }
            health.hp = 0.0;
            health.downed = true;
            health.revive = 0.0;
            score.downs += 1;
            info!("player {} is down!", player.name);
            events.write(ToClients { targets: SendTargets::All, message: GameEvent::Downed { slot: player.slot } });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_hits_capsule_in_front() {
        let t = ray_capsule(Vec3::ZERO, Vec3::NEG_Z, Vec3::new(0.0, -1.0, -10.0), Vec3::new(0.0, 1.0, -10.0), 0.5);
        let t = t.expect("should hit");
        assert!((t - 9.5).abs() < 0.05, "{t}");
    }

    #[test]
    fn ray_misses_capsule_on_the_side_or_behind() {
        assert!(ray_capsule(Vec3::ZERO, Vec3::NEG_Z, Vec3::new(2.0, -1.0, -10.0), Vec3::new(2.0, 1.0, -10.0), 0.5).is_none());
        assert!(ray_capsule(Vec3::ZERO, Vec3::NEG_Z, Vec3::new(0.0, -1.0, 10.0), Vec3::new(0.0, 1.0, 10.0), 0.5).is_none());
    }

    #[test]
    fn ray_vs_sphere_capsule() {
        // Regression: a capsule whose two ends are the same point (a sphere)
        // used to produce NaN and count as a hit from anywhere.
        let p = Vec3::new(30.0, 0.0, -5.0);
        assert!(ray_capsule(Vec3::ZERO, Vec3::NEG_Z, p, p, 0.4).is_none());
        let t = ray_capsule(Vec3::ZERO, Vec3::NEG_Z, Vec3::new(0.0, 0.0, -5.0), Vec3::new(0.0, 0.0, -5.0), 0.4);
        assert!((t.expect("should hit") - 4.6).abs() < 0.01);
    }

    #[test]
    fn every_mob_capsule_is_hittable_only_in_front() {
        for kind in MobKind::SPAWNABLE.into_iter().chain([MobKind::Colossus]) {
            let (a, b, r) = mob_capsule(kind, Vec3::new(0.0, 0.0, -10.0));
            assert!(ray_capsule(Vec3::new(0.0, 0.5, 0.0), Vec3::NEG_Z, a, b, r).is_some(), "{kind:?}");
            let (a, b, r) = mob_capsule(kind, Vec3::new(40.0, 0.0, -10.0));
            assert!(ray_capsule(Vec3::new(0.0, 0.5, 0.0), Vec3::NEG_Z, a, b, r).is_none(), "{kind:?}");
        }
    }

    #[test]
    fn history_interpolates() {
        let mut h = MobHistory::default();
        h.record(10, Vec3::ZERO);
        h.record(12, Vec3::new(2.0, 0.0, 0.0));
        assert_eq!(h.at(11, Vec3::ONE), Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(h.at(5, Vec3::ONE), Vec3::ZERO); // before window: oldest
        assert_eq!(h.at(50, Vec3::ONE), Vec3::new(2.0, 0.0, 0.0)); // after: newest
    }

    fn mob_at(kind: MobKind, yaw: f32, anim: MobAnim) -> (Mob, MobState) {
        (Mob { kind, level: 1, elite: false }, MobState { pos: Vec3::new(0.0, 0.0, -10.0), yaw, tick: 0, anim })
    }

    #[test]
    fn scarab_shell_in_front_soft_back() {
        // Facing the shooter (yaw PI: it looks toward +Z, where we are).
        let (m, s) = mob_at(MobKind::Scarab, std::f32::consts::PI, MobAnim::Move);
        let hit = ray_mob(&m, &s, None, s.pos, Vec3::new(0.0, 0.5, 0.0), Vec3::NEG_Z, 0.1).unwrap();
        assert!(matches!(hit.1, Some(ZoneKind::Armor(_))));
        // Turned away (yaw 0: looks toward -Z): we hit its back.
        let (m, s) = mob_at(MobKind::Scarab, 0.0, MobAnim::Move);
        let hit = ray_mob(&m, &s, None, s.pos, Vec3::new(0.0, 0.55, 0.0), Vec3::NEG_Z, 0.1).unwrap();
        assert!(matches!(hit.1, Some(ZoneKind::Weak(_))));
    }

    #[test]
    fn burrowed_monsters_cannot_be_hit() {
        let (m, s) = mob_at(MobKind::Burrower, 0.0, MobAnim::Burrowed);
        assert!(ray_mob(&m, &s, None, s.pos, Vec3::new(0.0, 0.4, 0.0), Vec3::NEG_Z, 0.5).is_none());
    }

    #[test]
    fn broodmother_legs_break_one_by_one() {
        let (m, s) = mob_at(MobKind::Broodmother, 0.0, MobAnim::Move);
        let knee = crate::shared::mobs::BROOD_LEGS[0] + s.pos;
        // From the front (it faces -Z): the front-right knee is the first thing hit.
        let origin = knee + Vec3::new(0.0, 0.0, -10.0);
        let hit = ray_mob(&m, &s, None, s.pos, origin, Vec3::Z, 0.1).unwrap();
        assert_eq!(hit.1, Some(ZoneKind::Leg(0)));
        // Once broken, the shot goes through where the knee was.
        let parts = MobParts { broken: 1 };
        let hit = ray_mob(&m, &s, Some(&parts), s.pos, origin, Vec3::Z, 0.1);
        assert!(hit.is_none_or(|h| h.1 != Some(ZoneKind::Leg(0))));
    }
}
