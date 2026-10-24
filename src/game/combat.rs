//! Combat resolution (server): hitscan shots with lag compensation,
//! projectiles, explosions, damage, deaths and rewards.
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
use crate::shared::mobs::MobKind;
use crate::shared::movement::{DT, PLAYER_HEIGHT};
use crate::shared::rng::Rng;
use crate::shared::weapons::{ProjectileType, WeaponKind};

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

/// Server data of a boss attack area.
#[derive(Component, Debug)]
pub struct HazardServer {
    pub damage: f32,
    /// Players already hit (a shockwave hits each player once).
    pub hit: Vec<Entity>,
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
#[derive(Component, Debug)]
pub struct ProjectileServer {
    pub damage: f32,
    pub splash: f32,
    pub by: Option<Entity>,
    pub hurts_players: bool,
    pub ttl: f32,
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
fn mob_capsule(kind: MobKind, pos: Vec3) -> (Vec3, Vec3, f32) {
    let d = kind.def();
    let r = d.radius.min(d.height * 0.5);
    (pos + Vec3::Y * r, pos + Vec3::Y * (d.height - r).max(r), r)
}

#[allow(clippy::too_many_arguments)]
fn resolve_shots(
    mut pending: ResMut<PendingShots>,
    mut commands: Commands,
    mobs: Query<(Entity, &Mob, &MobState, &MobHistory, &Health)>,
    terrain: Res<TerrainRes>,
    tick: Res<SimTick>,
    shooters: Query<(&Items, &Health), With<Player>>,
    mut damage: MessageWriter<DamageMob>,
    mut explode: MessageWriter<Explode>,
    mut fx: MessageWriter<ToClients<Fx>>,
) {
    for p in pending.0.drain(..) {
        let shot = p.shot;
        let def = shot.weapon.def();
        let shooter = shooters.get(p.shooter).ok();
        let player_stats = shooter.map(|(i, _)| PlayerStats::from_inventory(&i.0));
        let own_health = shooter.map(|(_, h)| h.hp / h.max.max(1.0)).unwrap_or(1.0);
        let mut rng = Rng::new(shot.seed ^ 0x5EED_CAFE);

        // Projectile weapons spawn an entity instead.
        if let Some(projectile) = def.projectile {
            let speed = def.projectile_speed;
            let kind = match projectile {
                ProjectileType::Rocket => ProjectileKind::Rocket,
                ProjectileType::Grenade => ProjectileKind::Grenade,
                ProjectileType::Plasma => ProjectileKind::Plasma,
            };
            commands.spawn((
                Replicated,
                Projectile {
                    kind,
                    origin: shot.origin + shot.dir * 0.8,
                    vel: shot.dir * speed,
                    gravity: def.projectile_gravity,
                    spawn_tick: tick.0,
                    owner_slot: p.slot,
                },
                ProjectileServer {
                    damage: shot.damage,
                    splash: def.splash_radius,
                    by: Some(p.shooter),
                    hurts_players: false,
                    ttl: def.range / speed + 1.0,
                },
            ));
            continue;
        }

        // Lag compensation: test against monsters where the shooter saw them.
        let view_tick = p.view_tick.clamp(tick.0.saturating_sub(MAX_REWIND_TICKS), tick.0);
        let wall = terrain.raycast(shot.origin, shot.dir, def.range).unwrap_or(def.range);
        let mut best: Option<(Entity, f32)> = None;
        let mut pierced: Vec<(Entity, f32)> = Vec::new();
        for (entity, mob, state, history, _) in &mobs {
            let pos = history.at(view_tick, state.pos);
            let (a, b, r) = mob_capsule(mob.kind, pos);
            if let Some(t) = ray_capsule(shot.origin, shot.dir, a, b, r)
                && t < wall
            {
                pierced.push((entity, t));
                if best.is_none_or(|(_, bt)| t < bt) {
                    best = Some((entity, t));
                }
            }
        }
        // Railgun: every monster on the line takes the hit; the beam stops at the wall.
        if def.pierce {
            let end = shot.origin + shot.dir * wall;
            let targets = match p.owner {
                Some(id) => SendTargets::AllExcept(id),
                None => SendTargets::All,
            };
            fx.write(ToClients { targets, message: Fx::Tracer { slot: p.slot, weapon: shot.weapon, from: shot.origin, to: end } });
            for (target, t) in pierced {
                let crit = player_stats.is_some_and(|s| rng.chance(s.crit_chance));
                let crit_mult = if crit { player_stats.map(|s| s.crit_mult).unwrap_or(2.0) } else { 1.0 };
                let pos = shot.origin + shot.dir * t;
                damage.write(DamageMob { target, amount: shot.damage * crit_mult, crit, pos, by: Some(p.shooter) });
            }
            continue;
        }

        let end_dist = best.map(|(_, t)| t).unwrap_or(wall);
        let end = shot.origin + shot.dir * end_dist;
        // Tracer for everyone except the shooter (who predicted it locally).
        let targets = match p.owner {
            Some(id) => SendTargets::AllExcept(id),
            None => SendTargets::All,
        };
        // Only one tracer per shotgun volley would look wrong: send all, they are tiny.
        fx.write(ToClients { targets, message: Fx::Tracer { slot: p.slot, weapon: shot.weapon, from: shot.origin, to: end } });

        if let Some((target, t)) = best {
            // Damage falloff for shotgun pellets at long range.
            let falloff = if shot.weapon == WeaponKind::Shotgun { (1.0 - (t - 10.0).max(0.0) / 40.0).max(0.3) } else { 1.0 };
            let crit = player_stats.is_some_and(|s| rng.chance(s.crit_chance));
            let crit_mult = if crit { player_stats.map(|s| s.crit_mult).unwrap_or(2.0) } else { 1.0 };
            let target_health = mobs.get(target).map(|(.., h)| h.hp / h.max.max(1.0)).unwrap_or(1.0);
            let conditional = player_stats.map(|s| s.conditional_damage(own_health, target_health)).unwrap_or(1.0);
            let amount = shot.damage * falloff * crit_mult * conditional;
            damage.write(DamageMob { target, amount, crit, pos: end, by: Some(p.shooter) });
            // Tesla coil: an electric arc jumps to the 2 nearest other monsters.
            if let Some(s) = player_stats
                && rng.chance(s.tesla_chance)
            {
                let mut others: Vec<(Entity, Vec3)> = mobs
                    .iter()
                    .filter(|(e, ..)| *e != target)
                    .map(|(e, m, st, ..)| (e, st.pos + Vec3::Y * m.kind.def().height * 0.5))
                    .filter(|(_, pos)| pos.distance(end) < 9.0)
                    .collect();
                others.sort_by(|a, b| a.1.distance(end).total_cmp(&b.1.distance(end)));
                for (e, pos) in others.into_iter().take(2) {
                    damage.write(DamageMob { target: e, amount: shot.damage * 0.6, crit: false, pos, by: Some(p.shooter) });
                    fx.write(ToClients { targets: SendTargets::All, message: Fx::Arc { from: end, to: pos } });
                }
            }
            if let Some(s) = player_stats
                && rng.chance(s.explode_chance)
            {
                explode.write(Explode {
                    pos: end,
                    radius: 3.0,
                    damage: shot.damage * 0.6,
                    hurts_players: false,
                    hurts_mobs: true,
                    by: Some(p.shooter),
                });
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn update_projectiles(
    mut commands: Commands,
    mut projectiles: Query<(Entity, &Projectile, &mut ProjectileServer)>,
    mobs: Query<(Entity, &Mob, &MobState)>,
    players: Query<(Entity, &PlayerState, &Health)>,
    terrain: Res<TerrainRes>,
    tick: Res<SimTick>,
    mut damage_mob: MessageWriter<DamageMob>,
    mut damage_player: MessageWriter<DamagePlayer>,
    mut explode: MessageWriter<Explode>,
) {
    for (entity, proj, mut data) in &mut projectiles {
        data.ttl -= DT;
        let pos = proj.pos_at(f64::from(tick.0));
        let ground = terrain.wgen.ground_height(pos.x, pos.z);
        let mut solid_hit = false;
        terrain.cache.for_each_solid_near(pos, |s| {
            let (bottom, top) = s.y_range();
            if !solid_hit && pos.y >= bottom && pos.y <= top && s.push_out_xz(Vec2::new(pos.x, pos.z), 0.15).is_some() {
                solid_hit = true;
            }
        });
        let mut hit = solid_hit || pos.y <= ground || data.ttl <= 0.0;

        let radius = match proj.kind {
            ProjectileKind::BossOrb => 0.9,
            ProjectileKind::Rocket | ProjectileKind::Grenade => 0.4,
            _ => 0.35,
        };
        let mut direct: Option<Entity> = None;
        if data.hurts_players {
            for (pe, ps, health) in &players {
                if health.downed {
                    continue;
                }
                let center = ps.sim.mv.pos + Vec3::Y * (PLAYER_HEIGHT * 0.5);
                if (pos - center).length() < radius + 0.6 {
                    direct = Some(pe);
                    hit = true;
                    break;
                }
            }
        } else {
            for (me, mob, ms) in &mobs {
                let (a, b, r) = mob_capsule(mob.kind, ms.pos);
                let closest = a + (b - a) * ((pos - a).dot(b - a) / (b - a).length_squared().max(1e-4)).clamp(0.0, 1.0);
                if closest.distance(pos) < r + radius {
                    direct = Some(me);
                    hit = true;
                    break;
                }
            }
        }

        if !hit {
            continue;
        }
        commands.entity(entity).try_despawn();
        if data.splash > 0.0 {
            explode.write(Explode {
                pos,
                radius: data.splash,
                damage: data.damage,
                hurts_players: data.hurts_players,
                hurts_mobs: !data.hurts_players,
                by: data.by,
            });
        } else if let Some(target) = direct {
            if data.hurts_players {
                damage_player.write(DamagePlayer { target, amount: data.damage, attacker: None });
            } else {
                damage_mob.write(DamageMob { target, amount: data.damage, crit: false, pos, by: data.by });
            }
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
                let center = state.pos + Vec3::Y * mob.kind.def().height * 0.5;
                let d = (center - ex.pos).length() - mob.kind.def().radius;
                if d < ex.radius {
                    damage_mob.write(DamageMob {
                        target: entity,
                        amount: ex.damage * scale(d.max(0.0)),
                        crit: false,
                        pos: center,
                        by: ex.by,
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
    mut mobs: Query<(&Mob, &mut Health, &MobState), Without<Player>>,
    mut players: Query<(&Player, &mut Health, &mut Wallet, &Items, &mut PlayerScore, &mut PlayerState), Without<Mob>>,
    mut info: Single<&mut MatchInfo>,
    tick: Res<SimTick>,
    mut fx: MessageWriter<ToClients<Fx>>,
    mut events: MessageWriter<ToClients<GameEvent>>,
) {
    for hit in hits.read() {
        let Ok((mob, mut health, mob_state)) = mobs.get_mut(hit.target) else { continue };
        if health.hp <= 0.0 {
            continue; // already dead this tick
        }
        let dealt = hit.amount.min(health.hp);
        health.hp -= hit.amount;
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
            message: Fx::Hit { pos: hit.pos, amount: hit.amount.round() as u16, crit: hit.crit, killed, by_slot },
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
            // A jelly splits into two small ones.
            if mob.kind == MobKind::Jelly {
                for side in [-1.0f32, 1.0] {
                    let pos = mob_state.pos + Vec3::new(side * 0.8, 0.3, 0.0);
                    crate::game::mobs::spawn_mob(&mut commands, MobKind::Jellet, pos, mob.level, tick.0, side, false);
                }
            }
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
}
