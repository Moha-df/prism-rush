//! Monsters (server): the wave *director* and per-type AI.
//!
//! Each wave has a spawn budget. The director spends it in small packs around
//! the players (like Risk of Rain 2's director, but per wave). When the
//! budget is spent and the last monster dies, the wave is cleared, a short
//! break follows, then the next wave starts. Every 5th wave adds a boss.

use bevy::prelude::*;
use bevy_replicon::prelude::*;

use super::combat::{DamagePlayer, Explode, HazardServer, HealMobs, MobHistory, ProjectileServer};
use super::world::TerrainRes;
use super::{ServerSet, SimTick};
use crate::net::protocol::*;
use crate::shared::economy;
use crate::shared::mobs::MobKind;
use crate::shared::movement::{move_body, DT, GRAVITY, PLAYER_HEIGHT};
use crate::shared::rng::Rng;

/// Wave state that only the server needs.
#[derive(Resource)]
pub struct Director {
    /// Spawn budget left for the current wave.
    pub budget: f32,
    pub spawn_timer: f32,
    pub boss_pending: bool,
    /// Seconds since every monster of the wave has been spawned.
    pub clearing_time: f32,
    pub rng: Rng,
    pub spawned: u32,
    /// Set when a wave is cleared (consumed by the Medkit healing).
    pub just_cleared: bool,
}

impl Default for Director {
    fn default() -> Self {
        Self { budget: 0.0, spawn_timer: 2.0, boss_pending: false, clearing_time: 0.0, rng: Rng::new(0xD1EC), spawned: 0, just_cleared: false }
    }
}

/// Server-only AI state of a monster.
#[derive(Component, Debug, Default)]
pub struct MobAi {
    pub vel: Vec3,
    pub grounded: bool,
    pub cooldown: f32,
    /// Remaining wind-up time (> 0 while telegraphing an attack).
    pub windup: f32,
    /// Remaining "attack" animation time.
    pub attack_anim: f32,
    pub target: Option<Entity>,
    pub retarget: f32,
    pub phase: f32,
    pub stuck: f32,
    /// Charger: remaining charge time and locked direction.
    pub charge_timer: f32,
    pub charge_dir: Vec3,
    pub charge_hits: Vec<Entity>,
    /// Number of attacks done (bosses alternate their attacks).
    pub attacks: u32,
}

const ACID_GRAVITY: f32 = 18.0;

pub struct MobsPlugin;

impl Plugin for MobsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Director>().add_systems(
            FixedUpdate,
            (director, bring_back_stragglers, mob_ai, separate_mobs).chain().in_set(ServerSet::Mobs),
        );
    }
}

/// Elites have this many times the HP (and pay this many times the gold).
pub const ELITE_HP: f32 = 3.0;
pub const ELITE_DAMAGE: f32 = 1.5;

pub fn spawn_mob(commands: &mut Commands, kind: MobKind, pos: Vec3, level: u32, tick: u32, phase: f32, elite: bool) -> Entity {
    let hp = kind.def().hp * economy::monster_hp_mult(level) * if elite { ELITE_HP } else { 1.0 };
    commands
        .spawn((
            Replicated,
            Mob { kind, level, elite },
            MobState { pos, yaw: 0.0, tick, anim: MobAnim::Move },
            Health::new(hp),
            MobAi { phase, cooldown: 1.0, ..default() },
            MobHistory::default(),
        ))
        .id()
}

/// Boss of a boss wave: Colossus, Queen, Titan, then again (stronger).
pub fn boss_for_wave(wave: u32) -> MobKind {
    MobKind::BOSSES[((wave / 5).max(1) as usize - 1) % MobKind::BOSSES.len()]
}

/// Chance that a spawned monster is an elite.
pub fn elite_chance(wave: u32) -> f32 {
    if wave < 3 { 0.0 } else { (0.03 * (wave - 2) as f32).min(0.35) }
}

/// A random point on a ring around one of `anchors`, on the ground.
fn ring_point(terrain: &TerrainRes, rng: &mut Rng, anchors: &[Vec3], min: f32, max: f32) -> Vec3 {
    let anchor = *rng.pick(anchors);
    let angle = rng.range(0.0, std::f32::consts::TAU);
    let d = rng.range(min, max);
    let x = anchor.x + angle.cos() * d;
    let z = anchor.z + angle.sin() * d;
    Vec3::new(x, terrain.wgen.ground_height(x, z), z)
}

fn director(
    mut commands: Commands,
    mut dir: ResMut<Director>,
    mut info: Single<&mut MatchInfo>,
    players: Query<(&PlayerState, &Health)>,
    mobs: Query<&Mob>,
    terrain: Res<TerrainRes>,
    tick: Res<SimTick>,
    mut events: MessageWriter<ToClients<GameEvent>>,
) {
    let alive: Vec<Vec3> = players.iter().filter(|(_, h)| !h.downed).map(|(s, _)| s.sim.mv.pos).collect();
    if alive.is_empty() {
        return;
    }
    let n_players = players.iter().count() as u32;
    let alive_mobs = mobs.iter().count();
    info.enemies_left = alive_mobs as u16;

    match info.wave_phase {
        WavePhase::Break => {
            info.wave_timer -= DT;
            if info.wave_timer > 0.0 {
                return;
            }
            // --- Start the next wave.
            info.wave += 1;
            info.wave_phase = WavePhase::Fighting;
            let wave = info.wave;
            dir.budget = economy::wave_budget(wave, n_players);
            dir.boss_pending = economy::is_boss_wave(wave);
            dir.clearing_time = 0.0;
            dir.spawn_timer = 1.0;
            info.wave_spawning = true;
            info!("=== wave {wave} (budget {:.0}{}) ===", dir.budget, if dir.boss_pending { ", BOSS" } else { "" });
            events.write(ToClients {
                targets: SendTargets::All,
                message: GameEvent::WaveStarted { wave, boss: dir.boss_pending },
            });
        }
        WavePhase::Fighting => {
            let wave = info.wave;
            let level = economy::monster_level(wave);
            dir.spawn_timer -= DT;

            // The boss of a boss wave arrives a few seconds in.
            if dir.boss_pending && dir.spawn_timer <= 0.0 {
                dir.boss_pending = false;
                let pos = ring_point(&terrain, &mut dir.rng, &alive, 35.0, 45.0);
                spawn_mob(&mut commands, boss_for_wave(wave), pos, level, tick.0, 0.0, false);
                info.boss_alive = true;
                events.write(ToClients { targets: SendTargets::All, message: GameEvent::BossSpawned });
                info!("BOSS spawned at {pos:?}");
            }

            info.wave_spawning = dir.budget > 0.0 || dir.boss_pending;
            if !info.wave_spawning {
                dir.clearing_time += DT;
                if alive_mobs == 0 {
                    // --- Wave cleared!
                    info.wave_phase = WavePhase::Break;
                    info.wave_timer = economy::WAVE_BREAK;
                    dir.just_cleared = true;
                    info!("wave {wave} cleared");
                    events.write(ToClients { targets: SendTargets::All, message: GameEvent::WaveCleared { wave } });
                }
                return;
            }
            if dir.budget <= 0.0 || dir.spawn_timer > 0.0 || alive_mobs >= economy::max_alive_monsters(n_players, wave) {
                return;
            }
            dir.spawn_timer = economy::spawn_interval(wave) * dir.rng.range(0.7, 1.3);

            // Choose a monster type among those unlocked at this wave.
            let options: Vec<MobKind> = MobKind::SPAWNABLE.into_iter().filter(|k| k.def().unlock_wave <= wave).collect();
            let weights: Vec<f32> = options.iter().map(|k| k.def().spawn_weight).collect();
            let kind = options[dir.rng.weighted_index(&weights)];
            let cost = kind.def().cost;
            // A small pack of the same type (the last pack may overspend a bit).
            let pack = dir.rng.range_u32(1, 5).min((dir.budget / cost).ceil() as u32).max(1);
            let center = ring_point(&terrain, &mut dir.rng, &alive, 28.0, 45.0);
            for i in 0..pack {
                let offset = Vec3::new(dir.rng.range(-3.0, 3.0), 0.0, dir.rng.range(-3.0, 3.0));
                let mut pos = center + offset;
                pos.y = terrain.wgen.ground_height(pos.x, pos.z) + if kind.def().flying { 6.0 } else { 0.2 };
                let phase = dir.rng.range(0.0, 100.0);
                let elite = dir.rng.chance(elite_chance(wave));
                spawn_mob(&mut commands, kind, pos, level, tick.0, phase + i as f32, elite);
                dir.budget -= cost;
                dir.spawned += 1;
            }
        }
    }
}

/// Monsters too far from every player, or the last ones of a wave that
/// survive too long, are brought back next to the players: the wave can
/// always be finished, nobody has to hunt a lost monster across the map.
fn bring_back_stragglers(
    mut dir: ResMut<Director>,
    info: Single<&MatchInfo>,
    players: Query<(&PlayerState, &Health)>,
    mut mobs: Query<(&Mob, &mut MobState, &mut MobAi)>,
    terrain: Res<TerrainRes>,
    mut fx: MessageWriter<ToClients<Fx>>,
) {
    let alive: Vec<Vec3> = players.iter().filter(|(_, h)| !h.downed).map(|(s, _)| s.sim.mv.pos).collect();
    if alive.is_empty() || info.wave_phase != WavePhase::Fighting {
        return;
    }
    // Time rule only once the whole wave has spawned. Re-armed every 10 s so
    // a fleeing monster is not warped every single tick.
    let too_long = !info.wave_spawning && dir.clearing_time > economy::STRAGGLER_TIME;
    if too_long {
        dir.clearing_time = economy::STRAGGLER_TIME - 10.0;
    }
    for (mob, mut state, mut ai) in &mut mobs {
        let nearest = alive.iter().map(|p| p.distance(state.pos)).fold(f32::INFINITY, f32::min);
        if nearest > economy::STRAGGLER_DISTANCE || (too_long && nearest > 15.0) {
            let from = state.pos;
            let mut to = ring_point(&terrain, &mut dir.rng, &alive, 16.0, 24.0);
            if mob.kind.def().flying {
                to.y += 5.0;
            }
            state.pos = to;
            ai.vel = Vec3::ZERO;
            fx.write(ToClients { targets: SendTargets::All, message: Fx::Warp { from, to } });
        }
    }
}

/// Yaw that faces `dir` (Bevy forward is -Z).
pub fn yaw_toward(dir: Vec3) -> f32 {
    f32::atan2(-dir.x, -dir.z)
}

type MobQuery<'a> = (Entity, &'a Mob, &'a mut MobState, &'a mut MobAi, &'a mut MobHistory, &'a Health);

#[allow(clippy::too_many_arguments)]
fn mob_ai(
    mut commands: Commands,
    mut mobs: Query<MobQuery>,
    players: Query<(Entity, &PlayerState, &Health)>,
    terrain: Res<TerrainRes>,
    tick: Res<SimTick>,
    mut damage: MessageWriter<DamagePlayer>,
    mut explode: MessageWriter<Explode>,
    mut heal: MessageWriter<HealMobs>,
    mut fx: MessageWriter<ToClients<Fx>>,
) {
    let targets: Vec<(Entity, Vec3, Vec3)> = players
        .iter()
        .filter(|(_, _, h)| !h.downed)
        .map(|(e, s, _)| (e, s.sim.mv.pos, s.sim.mv.vel))
        .collect();

    for (entity, mob, mut state, mut ai, mut history, health) in &mut mobs {
        if health.hp <= 0.0 {
            continue;
        }
        let def = mob.kind.def();
        let dmg_mult = economy::monster_damage_mult(mob.level) * if mob.elite { ELITE_DAMAGE } else { 1.0 };
        ai.phase += DT;
        ai.cooldown -= DT;
        ai.attack_anim = (ai.attack_anim - DT).max(0.0);
        ai.retarget -= DT;

        // Pick the nearest standing player.
        if ai.retarget <= 0.0 || ai.target.is_none_or(|t| !targets.iter().any(|(e, _, _)| *e == t)) {
            ai.retarget = 0.5;
            ai.target = targets
                .iter()
                .min_by(|a, b| a.1.distance_squared(state.pos).total_cmp(&b.1.distance_squared(state.pos)))
                .map(|t| t.0);
        }
        let target = ai.target.and_then(|t| targets.iter().find(|(e, _, _)| *e == t)).copied();

        let mut want;
        if let Some((target_entity, tpos, tvel)) = target {
            let chest = tpos + Vec3::Y * (PLAYER_HEIGHT * 0.6);
            let to = tpos - state.pos;
            let flat = Vec3::new(to.x, 0.0, to.z);
            let dist = flat.length();
            let dir = flat.normalize_or_zero();
            let side = Vec3::new(-dir.z, 0.0, dir.x);
            state.yaw = yaw_toward(dir);

            // --- Movement intention per type.
            want = match mob.kind {
                MobKind::Skitter => (dir + side * (ai.phase * 3.0).sin() * 0.5).normalize_or_zero() * def.speed,
                MobKind::Spitter => {
                    if dist > 22.0 {
                        dir * def.speed
                    } else if dist < 11.0 {
                        -dir * def.speed
                    } else {
                        side * (ai.phase * 0.4).sin().signum() * def.speed * 0.6
                    }
                }
                MobKind::Wisp => {
                    // Circle around the target, a few meters above.
                    let angle = ai.phase * 0.5;
                    let goal = tpos + Vec3::new(angle.cos() * 11.0, 5.5 + (ai.phase * 1.3).sin(), angle.sin() * 11.0);
                    (goal - state.pos).clamp_length_max(def.speed)
                }
                MobKind::Shaman => {
                    // Keeps its distance, floating low, behind the other monsters.
                    let ground = terrain.wgen.ground_height(state.pos.x, state.pos.z);
                    let radial = if dist > 24.0 { dir } else if dist < 15.0 { -dir } else { side * 0.5 };
                    let lift = Vec3::Y * ((ground + 2.5 + (ai.phase * 2.0).sin() * 0.4) - state.pos.y);
                    (radial * def.speed + lift).clamp_length_max(def.speed)
                }
                MobKind::HiveQueen => {
                    // Circles high above the players.
                    let angle = ai.phase * 0.35;
                    let goal = tpos + Vec3::new(angle.cos() * 18.0, 9.0 + (ai.phase * 0.8).sin() * 1.5, angle.sin() * 18.0);
                    (goal - state.pos).clamp_length_max(def.speed)
                }
                MobKind::StormTitan if dist < 12.0 => Vec3::ZERO,
                MobKind::Charger if dist < 6.0 => side * def.speed,
                _ => dir * def.speed,
            };

            // --- Attacks: wind-up (telegraph) then strike.
            let in_range = if def.ranged { dist < def.attack_range } else { to.length() < def.attack_range };
            if ai.windup <= 0.0 && ai.cooldown <= 0.0 && ai.charge_timer <= 0.0 && in_range {
                ai.windup = def.windup;
                // The charger aims when it starts snorting: dodge sideways!
                ai.charge_dir = dir;
                fx.write(ToClients { targets: SendTargets::All, message: Fx::MobAttack { pos: state.pos, kind: mob.kind } });
            }
            if ai.windup > 0.0 {
                want *= 0.15;
                ai.windup -= DT;
                if ai.windup <= 0.0 {
                    ai.cooldown = def.attack_cooldown;
                    ai.attack_anim = 0.35;
                    let damage_amount = def.damage * dmg_mult;
                    let eye = state.pos + Vec3::Y * def.height * 0.7;
                    ai.attacks += 1;
                    match mob.kind {
                        MobKind::Charger => {
                            ai.charge_timer = 1.1;
                            ai.charge_hits.clear();
                        }
                        MobKind::Jelly | MobKind::Jellet => {
                            if to.length() < def.attack_range * 1.5 {
                                damage.write(DamagePlayer { target: target_entity, amount: damage_amount, attacker: Some(entity) });
                            }
                        }
                        MobKind::Shaman => {
                            heal.write(HealMobs { pos: state.pos, radius: 14.0, fraction: 0.2 });
                            fx.write(ToClients { targets: SendTargets::All, message: Fx::Heal { pos: state.pos, radius: 14.0 } });
                            let vel = (chest - eye).normalize_or_zero() * 13.0;
                            spawn_enemy_projectile(&mut commands, ProjectileKind::Bolt, eye, vel, 0.0, damage_amount, 0.0, tick.0);
                        }
                        MobKind::HiveQueen => {
                            if ai.attacks.is_multiple_of(3) {
                                // Summon a swarm around her.
                                for i in 0..4 {
                                    let a = i as f32 * std::f32::consts::FRAC_PI_2 + ai.phase;
                                    let x = state.pos.x + a.cos() * 4.0;
                                    let z = state.pos.z + a.sin() * 4.0;
                                    let pos = Vec3::new(x, terrain.wgen.ground_height(x, z) + 0.3, z);
                                    spawn_mob(&mut commands, MobKind::Skitter, pos, mob.level, tick.0, a, false);
                                }
                            } else {
                                for i in -2..=2 {
                                    let aim = chest + side * (i as f32 * 2.5) + tvel * 0.4;
                                    let vel = (aim - eye).normalize_or_zero() * 22.0;
                                    spawn_enemy_projectile(&mut commands, ProjectileKind::Bolt, eye, vel, 0.0, damage_amount, 0.0, tick.0);
                                }
                            }
                        }
                        MobKind::StormTitan => {
                            if ai.attacks.is_multiple_of(2) {
                                // Ring on the ground: jump over it!
                                commands.spawn((
                                    Replicated,
                                    Hazard { kind: HazardKind::Shockwave, pos: state.pos, spawn_tick: tick.0 },
                                    HazardServer { damage: damage_amount, hit: Vec::new() },
                                ));
                            } else {
                                // Marked lightning strikes around the target.
                                for i in 0..4 {
                                    let a = i as f32 * 1.7 + ai.phase;
                                    let r = if i == 0 { 0.0 } else { 4.0 + i as f32 * 1.5 };
                                    let x = tpos.x + tvel.x * 0.8 + a.cos() * r;
                                    let z = tpos.z + tvel.z * 0.8 + a.sin() * r;
                                    let pos = Vec3::new(x, terrain.wgen.ground_height(x, z), z);
                                    commands.spawn((
                                        Replicated,
                                        Hazard { kind: HazardKind::Lightning, pos, spawn_tick: tick.0 },
                                        HazardServer { damage: damage_amount, hit: Vec::new() },
                                    ));
                                }
                            }
                        }
                        MobKind::Skitter => {
                            if to.length() < def.attack_range * 1.5 {
                                damage.write(DamagePlayer { target: target_entity, amount: damage_amount, attacker: Some(entity) });
                            }
                        }
                        MobKind::Brute => {
                            explode.write(Explode {
                                pos: state.pos + dir * 2.0,
                                radius: 4.5,
                                damage: damage_amount,
                                hurts_players: true,
                                hurts_mobs: false,
                                by: None,
                            });
                        }
                        MobKind::Bomber => {
                            explode.write(Explode {
                                pos: state.pos + Vec3::Y * 0.6,
                                radius: 4.5,
                                damage: damage_amount,
                                hurts_players: true,
                                hurts_mobs: false,
                                by: None,
                            });
                            commands.entity(entity).try_despawn();
                            continue;
                        }
                        MobKind::Spitter => {
                            let flight = (dist / 16.0).clamp(0.6, 1.6);
                            let aim = chest + tvel * flight * 0.5;
                            let vel = (aim - eye) / flight + Vec3::Y * (0.5 * ACID_GRAVITY * flight);
                            spawn_enemy_projectile(&mut commands, ProjectileKind::Acid, eye, vel, ACID_GRAVITY, damage_amount, 1.5, tick.0);
                        }
                        MobKind::Wisp => {
                            let vel = (chest - eye).normalize_or_zero() * 24.0;
                            spawn_enemy_projectile(&mut commands, ProjectileKind::Bolt, eye, vel, 0.0, damage_amount, 0.0, tick.0);
                        }
                        MobKind::Colossus => {
                            if dist < 9.0 {
                                explode.write(Explode {
                                    pos: state.pos,
                                    radius: 9.0,
                                    damage: damage_amount,
                                    hurts_players: true,
                                    hurts_mobs: false,
                                    by: None,
                                });
                            } else {
                                for i in -2..=2 {
                                    let flight = (dist / 20.0).clamp(0.8, 2.0);
                                    let spread = side * (i as f32 * 3.0);
                                    let aim = tpos + spread + tvel * flight * 0.6;
                                    let vel = (aim - eye) / flight + Vec3::Y * (0.5 * ACID_GRAVITY * flight);
                                    spawn_enemy_projectile(
                                        &mut commands,
                                        ProjectileKind::BossOrb,
                                        eye,
                                        vel,
                                        ACID_GRAVITY,
                                        damage_amount * 0.6,
                                        3.0,
                                        tick.0,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // No target (everyone down): wander slowly.
            want = Vec3::new((ai.phase * 0.3).cos(), 0.0, (ai.phase * 0.3).sin()) * def.speed * 0.3;
        }

        // --- Charger rush: locked direction, very fast, hurts on contact.
        if ai.charge_timer > 0.0 {
            ai.charge_timer -= DT;
            want = ai.charge_dir * 24.0;
            for (pe, ppos, _) in &targets {
                if !ai.charge_hits.contains(pe) && ppos.distance(state.pos) < def.radius + 1.0 {
                    ai.charge_hits.push(*pe);
                    damage.write(DamagePlayer { target: *pe, amount: def.damage * dmg_mult, attacker: Some(entity) });
                }
            }
        }

        // --- Physics.
        if def.flying {
            ai.vel = ai.vel.lerp(want, (4.0 * DT).min(1.0));
            state.pos += ai.vel * DT;
            let ground = terrain.wgen.ground_height(state.pos.x, state.pos.z);
            state.pos.y = state.pos.y.max(ground + 1.5);
        } else {
            let target_vel = Vec2::new(want.x, want.z);
            let cur = Vec2::new(ai.vel.x, ai.vel.z);
            let new = cur + (target_vel - cur).clamp_length_max(30.0 * DT);
            ai.vel.x = new.x;
            ai.vel.z = new.y;
            if !ai.grounded {
                ai.vel.y -= GRAVITY * DT;
            }
            let before = state.pos;
            let (mut pos, mut vel, mut grounded) = (state.pos, ai.vel, ai.grounded);
            move_body(&mut pos, &mut vel, &mut grounded, def.radius, def.height, &terrain, DT);
            state.pos = pos;
            ai.vel = vel;
            ai.grounded = grounded;
            // Blocked by an obstacle? Jump over it.
            let moved = Vec2::new(pos.x - before.x, pos.z - before.z).length();
            if target_vel.length() > 1.0 && moved < target_vel.length() * DT * 0.3 {
                ai.stuck += DT;
            } else {
                ai.stuck = 0.0;
            }
            if ai.stuck > 0.25 && ai.grounded {
                ai.vel.y = 10.0;
                ai.grounded = false;
                ai.stuck = 0.0;
            }
        }

        state.anim = if ai.windup > 0.0 {
            MobAnim::Windup
        } else if ai.attack_anim > 0.0 || ai.charge_timer > 0.0 {
            MobAnim::Attack
        } else {
            MobAnim::Move
        };
        state.tick = tick.0;
        history.record(tick.0, state.pos);
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_enemy_projectile(
    commands: &mut Commands,
    kind: ProjectileKind,
    origin: Vec3,
    vel: Vec3,
    gravity: f32,
    damage: f32,
    splash: f32,
    tick: u32,
) {
    commands.spawn((
        Replicated,
        Projectile { kind, origin, vel, gravity, spawn_tick: tick, owner_slot: 255 },
        ProjectileServer { damage, splash, by: None, hurts_players: true, ttl: 5.0 },
    ));
}

/// Keeps monsters from stacking on top of each other.
fn separate_mobs(mut mobs: Query<(&Mob, &mut MobState)>) {
    let snapshot: Vec<(Vec3, f32)> = mobs.iter().map(|(m, s)| (s.pos, m.kind.def().radius)).collect();
    for (i, (mob, mut state)) in mobs.iter_mut().enumerate() {
        let r = mob.kind.def().radius;
        let mut push = Vec3::ZERO;
        for (j, (p, pr)) in snapshot.iter().enumerate() {
            if i == j {
                continue;
            }
            let d = Vec3::new(state.pos.x - p.x, 0.0, state.pos.z - p.z);
            let min = (r + pr) * 0.9;
            let len = d.length();
            if len < min && len > 1e-4 {
                push += d / len * (min - len) * 0.5;
            }
        }
        if push != Vec3::ZERO {
            state.pos += push;
        }
    }
}
