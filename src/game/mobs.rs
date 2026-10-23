//! Monsters (server): the spawn *director* and per-type AI.
//!
//! The director earns "credits" over time (faster at higher difficulty and
//! with more players) and spends them to spawn monsters around the players,
//! like Risk of Rain 2. Every few minutes a boss appears.

use bevy::prelude::*;
use bevy_replicon::prelude::*;

use super::combat::{DamagePlayer, Explode, MobHistory, ProjectileServer};
use super::world::TerrainRes;
use super::{ServerSet, SimTick};
use crate::net::protocol::*;
use crate::shared::economy;
use crate::shared::mobs::MobKind;
use crate::shared::movement::{move_body, DT, GRAVITY, PLAYER_HEIGHT};
use crate::shared::rng::Rng;

/// Spawn budget and timers.
#[derive(Resource)]
pub struct Director {
    pub credits: f32,
    pub spawn_timer: f32,
    pub boss_timer: f32,
    pub rng: Rng,
    pub spawned: u32,
}

impl Default for Director {
    fn default() -> Self {
        Self { credits: 3.0, spawn_timer: 4.0, boss_timer: economy::BOSS_INTERVAL, rng: Rng::new(0xD1EC), spawned: 0 }
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
}

const DESPAWN_DISTANCE: f32 = 130.0;
const ACID_GRAVITY: f32 = 18.0;

pub struct MobsPlugin;

impl Plugin for MobsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Director>()
            .add_systems(FixedUpdate, (director, mob_ai, separate_mobs).chain().in_set(ServerSet::Mobs));
    }
}

pub fn spawn_mob(commands: &mut Commands, kind: MobKind, pos: Vec3, level: u32, tick: u32, phase: f32) -> Entity {
    let hp = kind.def().hp * economy::monster_hp_mult(level);
    commands
        .spawn((
            Replicated,
            Mob { kind, level },
            MobState { pos, yaw: 0.0, tick, anim: MobAnim::Move },
            Health::new(hp),
            MobAi { phase, cooldown: 1.0, ..default() },
            MobHistory::default(),
        ))
        .id()
}

#[allow(clippy::too_many_arguments)]
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
    let difficulty = info.difficulty;
    let level = info.level;
    dir.credits += economy::director_credit_rate(difficulty, n_players) * DT;
    dir.spawn_timer -= DT;
    dir.boss_timer -= DT;

    let ring_point = |rng: &mut Rng, min: f32, max: f32| -> Vec3 {
        let anchor = *rng.pick(&alive);
        let angle = rng.range(0.0, std::f32::consts::TAU);
        let d = rng.range(min, max);
        let x = anchor.x + angle.cos() * d;
        let z = anchor.z + angle.sin() * d;
        Vec3::new(x, terrain.wgen.ground_height(x, z), z)
    };

    // Boss.
    if dir.boss_timer <= 0.0 && !info.boss_alive {
        dir.boss_timer = economy::BOSS_INTERVAL;
        let pos = ring_point(&mut dir.rng, 35.0, 45.0);
        spawn_mob(&mut commands, MobKind::Colossus, pos, level, tick.0, 0.0);
        info.boss_alive = true;
        events.write(ToClients { targets: SendTargets::All, message: GameEvent::BossSpawned });
        info!("BOSS spawned at {pos:?}");
    }

    let alive_mobs = mobs.iter().count();
    if dir.spawn_timer > 0.0 || alive_mobs >= economy::max_alive_monsters(n_players, difficulty) {
        return;
    }
    dir.spawn_timer = dir.rng.range(1.2, 3.5) / difficulty.sqrt();

    // Choose a monster type among unlocked ones we can afford.
    let elapsed = info.elapsed;
    let options: Vec<MobKind> = MobKind::SPAWNABLE
        .into_iter()
        .filter(|k| k.def().unlock_time <= elapsed && k.def().cost <= dir.credits)
        .collect();
    if options.is_empty() {
        return;
    }
    let weights: Vec<f32> = options
        .iter()
        .map(|k| match k {
            MobKind::Skitter => 3.0,
            MobKind::Spitter => 2.0,
            MobKind::Wisp => 2.0,
            MobKind::Bomber => 1.4,
            MobKind::Brute => 1.0,
            MobKind::Colossus => 0.0,
        })
        .collect();
    let kind = options[dir.rng.weighted_index(&weights)];
    let cost = kind.def().cost;
    // Spawn a small pack of the same type.
    let pack = dir.rng.range_u32(1, 5).min((dir.credits / cost) as u32).max(1);
    let center = ring_point(&mut dir.rng, 28.0, 45.0);
    for i in 0..pack {
        let offset = Vec3::new(dir.rng.range(-3.0, 3.0), 0.0, dir.rng.range(-3.0, 3.0));
        let mut pos = center + offset;
        pos.y = terrain.wgen.ground_height(pos.x, pos.z) + if kind.def().flying { 6.0 } else { 0.2 };
        let phase = dir.rng.range(0.0, 100.0);
        spawn_mob(&mut commands, kind, pos, level, tick.0, phase + i as f32);
        dir.credits -= cost;
        dir.spawned += 1;
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
    mut fx: MessageWriter<ToClients<Fx>>,
) {
    let targets: Vec<(Entity, Vec3, Vec3)> = players
        .iter()
        .filter(|(_, _, h)| !h.downed)
        .map(|(e, s, _)| (e, s.sim.mv.pos, s.sim.mv.vel))
        .collect();
    let all_players: Vec<Vec3> = players.iter().map(|(_, s, _)| s.sim.mv.pos).collect();

    for (entity, mob, mut state, mut ai, mut history, health) in &mut mobs {
        if health.hp <= 0.0 {
            continue;
        }
        let def = mob.kind.def();
        let dmg_mult = economy::monster_damage_mult(mob.level);
        ai.phase += DT;
        ai.cooldown -= DT;
        ai.attack_anim = (ai.attack_anim - DT).max(0.0);
        ai.retarget -= DT;

        // Too far from everyone: despawn (the director will spawn closer ones).
        if !all_players.iter().any(|p| p.distance(state.pos) < DESPAWN_DISTANCE) && !mob.kind.is_boss() {
            commands.entity(entity).try_despawn();
            continue;
        }

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
                _ => dir * def.speed,
            };

            // --- Attacks: wind-up (telegraph) then strike.
            let in_range = if def.ranged { dist < def.attack_range } else { to.length() < def.attack_range };
            if ai.windup <= 0.0 && ai.cooldown <= 0.0 && in_range {
                ai.windup = def.windup;
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
                    match mob.kind {
                        MobKind::Skitter => {
                            if to.length() < def.attack_range * 1.5 {
                                damage.write(DamagePlayer { target: target_entity, amount: damage_amount });
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
        } else if ai.attack_anim > 0.0 {
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
