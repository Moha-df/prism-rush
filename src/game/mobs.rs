//! Monsters (server): the spawn *director* and per-type AI.
//!
//! The director earns a spawn budget that grows faster and faster with time
//! and spends it in packs around the players (Megabonk-like hordes). Each
//! run draws a random *roster* among the monsters of its map (a few per
//! role, see `shared::mobs::roll_roster`) and a random order of the map's
//! bosses, so two runs never look the same. Bosses come at fixed times,
//! then the Final Swarm: white ghosts at 15:00, only black ghosts at 20:00.
//! The difficulty level and the totems scale everything (`shared::rules`).

use std::collections::HashSet;

use bevy::prelude::*;
use bevy_replicon::prelude::*;

use super::combat::{DamagePlayer, Explode, HazardServer, HealMobs, MobHistory, ProjectileServer};
use super::world::TerrainRes;
use super::{ServerSet, SimTick};
use crate::net::protocol::*;
use crate::shared::economy;
use crate::shared::mobs::{roll_bosses, roll_roster, MobKind, Role, LEG_HP};
use crate::shared::movement::{move_body, DT, GRAVITY, PLAYER_HEIGHT};
use crate::shared::rng::Rng;
use crate::shared::rules::MapKind;

/// Spawn state that only the server needs.
#[derive(Resource)]
pub struct Director {
    /// Spawn budget available.
    pub credits: f32,
    pub spawn_timer: f32,
    /// Index of the next boss in `economy::BOSS_TIMES`.
    pub next_boss: usize,
    /// Fractional ghosts waiting to spawn.
    pub ghost_credits: f32,
    pub rng: Rng,
    pub spawned: u32,
    /// Regular monsters of this run (drawn at random per role).
    pub roster: Vec<MobKind>,
    /// Boss order of this run.
    pub bosses: Vec<MobKind>,
    /// Monster types already met this run (a toast presents new ones).
    pub seen: HashSet<MobKind>,
}

impl Director {
    pub fn new(seed: u64, map: MapKind) -> Self {
        let mut rng = Rng::new(seed ^ 0xD1EC);
        let roster = roll_roster(&mut rng, map.monsters());
        let bosses = roll_bosses(&mut rng, map.bosses());
        info!("run roster ({map:?}): {roster:?}, bosses: {bosses:?}");
        Self { credits: 3.0, spawn_timer: 3.0, next_boss: 0, ghost_credits: 0.0, rng, spawned: 0, roster, bosses, seen: HashSet::new() }
    }

    /// The n-th boss of the run (the order repeats after all of them).
    pub fn boss_for_index(&self, i: usize) -> MobKind {
        self.bosses[i % self.bosses.len()]
    }
}

impl Default for Director {
    fn default() -> Self {
        Self::new(0, MapKind::Meadow)
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
    /// Point aimed at (sentinel eye laser, sandworm eruption).
    pub aim: Vec3,
    /// Underground (burrower, sandworm): cannot be hit.
    pub burrowed: bool,
    /// Time left in the current burrowed / surfaced phase.
    pub state_timer: f32,
    /// Sandworm: time until it erupts at `aim` (> 0 while the circle shows).
    pub emerge_timer: f32,
    /// Toppled / kneeling boss: time left on the ground.
    pub down_timer: f32,
    /// Broodmother: HP of each leg.
    pub leg_hp: [f32; 4],
    /// Flea: time before the next hop. Lava boar: time before the next pool.
    pub hop_timer: f32,
}

const ACID_GRAVITY: f32 = 18.0;

pub struct MobsPlugin;

impl Plugin for MobsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Director>().add_systems(
            FixedUpdate,
            (director, apply_rules_to_new_mobs, bring_back_stragglers, mob_ai, separate_mobs).chain().in_set(ServerSet::Mobs),
        );
    }
}

/// Elites have this many times the HP (and pay this many times the gold).
pub const ELITE_HP: f32 = 3.0;
pub const ELITE_DAMAGE: f32 = 1.5;

pub fn spawn_mob(commands: &mut Commands, kind: MobKind, pos: Vec3, level: u32, tick: u32, phase: f32, elite: bool) -> Entity {
    let hp = kind.def().hp * economy::monster_hp_mult(level) * if elite { ELITE_HP } else { 1.0 };
    let mut ai = MobAi { phase, cooldown: 1.0, ..default() };
    match kind {
        MobKind::Burrower => {
            ai.burrowed = true;
            ai.state_timer = 1.0;
        }
        MobKind::Sandworm => {
            ai.burrowed = true;
            ai.state_timer = 4.0;
        }
        MobKind::Broodmother => ai.leg_hp = [hp * LEG_HP; 4],
        _ => {}
    }
    let mut e = commands.spawn((
        Replicated,
        Mob { kind, level, elite },
        MobState { pos, yaw: 0.0, tick, anim: if ai.burrowed { MobAnim::Burrowed } else { MobAnim::Move } },
        Health::new(hp),
        ai,
        MobHistory::default(),
    ));
    if kind == MobKind::Broodmother {
        e.insert(MobParts::default());
    }
    e.id()
}

/// Every new monster gets the HP of the run's difficulty, map and totems.
/// (One place for all spawns: director, splits, summons, statues...)
fn apply_rules_to_new_mobs(info: Single<&MatchInfo>, mut mobs: Query<(&mut Health, &mut MobAi), Added<Mob>>) {
    let m = info.rules.hp_mult();
    if m == 1.0 {
        return;
    }
    for (mut health, mut ai) in &mut mobs {
        health.max *= m;
        health.hp *= m;
        for leg in &mut ai.leg_hp {
            *leg *= m;
        }
    }
}

/// What happens when a monster dies (besides loot): splits, pools, blasts.
/// `dmg_scale`: damage multiplier of the run (`RunRules::damage_mult`).
pub fn on_mob_death(commands: &mut Commands, mob: &Mob, pos: Vec3, tick: u32, dmg_scale: f32) {
    let dmg = mob.kind.def().damage * economy::monster_damage_mult(mob.level) * dmg_scale;
    let split = |commands: &mut Commands, kind: MobKind, spread: f32| {
        for side in [-1.0f32, 1.0] {
            let p = pos + Vec3::new(side * spread, 0.3, 0.0);
            spawn_mob(commands, kind, p, mob.level, tick, side, false);
        }
    };
    match mob.kind {
        // A jelly splits into two small ones, a giant jelly into two jellies.
        MobKind::Jelly => split(commands, MobKind::Jellet, 0.8),
        MobKind::MegaJelly => split(commands, MobKind::Jelly, 1.4),
        MobKind::ToxicJelly => {
            commands.spawn((Replicated, Hazard::enemy(HazardKind::AcidPool, pos, tick, 3.0, 5.0), HazardServer::new(dmg * 0.6)));
        }
        MobKind::Sporeling => {
            commands.spawn((Replicated, Hazard::enemy(HazardKind::AcidPool, pos, tick, 3.5, 5.0), HazardServer::new(dmg * 0.6)));
        }
        // Magma: a marked circle that explodes a moment later.
        MobKind::MagmaJelly => {
            commands.spawn((Replicated, Hazard::enemy(HazardKind::Blast, pos, tick, 3.5, 0.9), HazardServer::new(dmg * 2.5)));
        }
        _ => {}
    }
}

/// Chance that a spawned monster is an elite (after 3 minutes).
pub fn elite_chance(elapsed: f32) -> f32 {
    let minutes = elapsed / 60.0;
    if minutes < 3.0 { 0.0 } else { (0.03 * (minutes - 2.0)).min(0.35) }
}

/// Spawn weight of a monster at a given minute: the first monsters of the
/// run slowly make room for the later ones.
pub fn spawn_weight(kind: MobKind, minutes: f32) -> f32 {
    let def = kind.def();
    let fade = if def.unlock_minute < 2.5 && minutes > 10.0 { 0.4 } else { 1.0 };
    def.spawn_weight * fade
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

/// Presents a monster type the first time it shows up in the run.
fn announce(dir: &mut Director, kind: MobKind, events: &mut MessageWriter<ToClients<GameEvent>>) {
    if !kind.hint().is_empty() && dir.seen.insert(kind) {
        events.write(ToClients { targets: SendTargets::All, message: GameEvent::NewMonster { kind } });
    }
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
    mut gallery: Local<Option<bool>>,
) {
    // Debug monster gallery (screenshots): no other monsters.
    if *gallery.get_or_insert_with(|| std::env::var("PRISM_GALLERY").is_ok()) {
        return;
    }
    let alive: Vec<Vec3> = players.iter().filter(|(_, h)| !h.downed).map(|(s, _)| s.sim.mv.pos).collect();
    if alive.is_empty() {
        return;
    }
    let n_players = players.iter().count() as u32;
    let alive_mobs = mobs.iter().count();
    info.enemies_left = alive_mobs as u16;
    let t = info.elapsed;
    let level = info.level;
    let rules = info.rules;

    // --- Bosses at fixed times (random order each run).
    if let Some(&at) = economy::BOSS_TIMES.get(dir.next_boss)
        && t >= at
    {
        let kind = dir.boss_for_index(dir.next_boss);
        dir.next_boss += 1;
        let pos = ring_point(&terrain, &mut dir.rng, &alive, 35.0, 45.0);
        let pos = pos + Vec3::Y * if kind.def().flying { 8.0 } else { 0.2 };
        spawn_mob(&mut commands, kind, pos, level, tick.0, 0.0, false);
        info.boss_alive = true;
        events.write(ToClients { targets: SendTargets::All, message: GameEvent::BossSpawned });
        announce(&mut dir, kind, &mut events);
        info!("BOSS {kind:?} at {:.0}s", t);
    }

    // --- Final Swarm: ghosts, stronger every minute, forever.
    let swarm = if t >= economy::BLACK_GHOSTS_TIME { 2 } else if t >= economy::FINAL_SWARM_TIME { 1 } else { 0 };
    if swarm > info.swarm {
        info.swarm = swarm;
        events.write(ToClients { targets: SendTargets::All, message: GameEvent::SwarmStarted { black: swarm == 2 } });
        info!("FINAL SWARM phase {swarm} at {:.0}s", t);
    }
    if swarm > 0 && alive_mobs < economy::MAX_MOBS {
        dir.ghost_credits += economy::ghost_rate(t, n_players) * rules.spawn_mult() * DT;
        let mult = economy::swarm_mult(t);
        while dir.ghost_credits >= 1.0 {
            dir.ghost_credits -= 1.0;
            // 15:00-20:00 white ghosts, then only black ones.
            let kind = if swarm == 2 { MobKind::BlackGhost } else { MobKind::WhiteGhost };
            let pos = ring_point(&terrain, &mut dir.rng, &alive, 30.0, 40.0) + Vec3::Y * 2.0;
            let e = spawn_mob(&mut commands, kind, pos, level, tick.0, dir.rng.range(0.0, 100.0), false);
            let hp = kind.def().hp * economy::monster_hp_mult(level) * mult;
            commands.entity(e).insert((GhostScaling(mult), Health::new(hp)));
        }
    }

    // --- Regular monsters: a budget that grows faster and faster (they stop
    // at 20:00, when only black ghosts come).
    if !economy::regular_spawns(t) {
        return;
    }
    dir.credits += economy::spawn_rate(t, n_players) * rules.spawn_mult() * DT;
    dir.spawn_timer -= DT;
    let cap = ((economy::max_alive_time(t, n_players) as f32 * rules.spawn_mult().sqrt()) as usize).min(160);
    if dir.spawn_timer > 0.0 || alive_mobs >= cap {
        return;
    }
    dir.spawn_timer = dir.rng.range(0.5, 1.4);
    let minutes = t / 60.0;
    let options: Vec<MobKind> = dir
        .roster
        .iter()
        .copied()
        .filter(|k| k.def().unlock_minute <= minutes && k.def().cost <= dir.credits)
        .collect();
    if options.is_empty() {
        return;
    }
    let weights: Vec<f32> = options.iter().map(|k| spawn_weight(*k, minutes)).collect();
    let kind = options[dir.rng.weighted_index(&weights)];
    let cost = kind.def().cost;
    // Packs: hordes come in groups (Megabonk-like); big monsters in small groups.
    let max_pack = if kind.role() == Role::Tank { 3 } else { 9 };
    let pack = dir.rng.range_u32(2, max_pack).min((dir.credits / cost) as u32).max(1);
    let center = ring_point(&terrain, &mut dir.rng, &alive, 26.0, 42.0);
    announce(&mut dir, kind, &mut events);
    for i in 0..pack {
        let offset = Vec3::new(dir.rng.range(-3.0, 3.0), 0.0, dir.rng.range(-3.0, 3.0));
        let mut pos = center + offset;
        pos.y = terrain.wgen.ground_height(pos.x, pos.z) + if kind.def().flying { 6.0 } else { 0.2 };
        let elite = dir.rng.chance((elite_chance(t) * rules.elite_mult()).min(0.6));
        let phase = dir.rng.range(0.0, 100.0);
        spawn_mob(&mut commands, kind, pos, level, tick.0, phase + i as f32, elite);
        dir.credits -= cost;
        dir.spawned += 1;
    }
}

/// Debug (screenshots): a monster that stands still, facing the players.
#[derive(Component)]
pub struct Frozen;

/// Final Swarm ghosts carry their strength multiplier (HP set at spawn,
/// speed and damage applied by the AI).
#[derive(Component, Debug, Clone, Copy)]
pub struct GhostScaling(pub f32);

/// Monsters too far from every player are brought back next to them:
/// nobody has to hunt a lost monster across the map.
fn bring_back_stragglers(
    mut dir: ResMut<Director>,
    players: Query<(&PlayerState, &Health)>,
    mut mobs: Query<(&Mob, &mut MobState, &mut MobAi)>,
    terrain: Res<TerrainRes>,
    mut fx: MessageWriter<ToClients<Fx>>,
) {
    let alive: Vec<Vec3> = players.iter().filter(|(_, h)| !h.downed).map(|(s, _)| s.sim.mv.pos).collect();
    if alive.is_empty() {
        return;
    }
    for (mob, mut state, mut ai) in &mut mobs {
        let nearest = alive.iter().map(|p| p.distance(state.pos)).fold(f32::INFINITY, f32::min);
        if nearest > economy::STRAGGLER_DISTANCE {
            let from = state.pos;
            let mut to = ring_point(&terrain, &mut dir.rng, &alive, 25.0, 35.0);
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

/// Initial velocity of a lobbed projectile reaching `to` in `flight` seconds.
fn lob(from: Vec3, to: Vec3, flight: f32, gravity: f32) -> Vec3 {
    (to - from) / flight + Vec3::Y * (0.5 * gravity * flight)
}

fn explode_at(explode: &mut MessageWriter<Explode>, pos: Vec3, radius: f32, damage: f32) {
    explode.write(Explode { pos, radius, damage, hurts_players: true, hurts_mobs: false, by: None });
}

type MobQuery<'a> = (
    Entity,
    &'a Mob,
    &'a mut MobState,
    &'a mut MobAi,
    &'a mut MobHistory,
    &'a Health,
    Option<&'a GhostScaling>,
    Option<&'a mut MobParts>,
    Has<Frozen>,
);

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
    info: Query<&MatchInfo>,
) {
    // Damage of the difficulty, map and totems.
    let rules_dmg = info.single().map(|m| m.rules.damage_mult()).unwrap_or(1.0);
    let targets: Vec<(Entity, Vec3, Vec3)> = players
        .iter()
        .filter(|(_, _, h)| !h.downed)
        .map(|(e, s, _)| (e, s.sim.mv.pos, s.sim.mv.vel))
        .collect();
    let crowd = mobs.iter().count();

    for (entity, mob, mut state, mut ai, mut history, health, ghost, mut parts, frozen) in &mut mobs {
        if health.hp <= 0.0 {
            continue;
        }
        let def = mob.kind.def();
        let ghost_mult = ghost.map(|g| g.0).unwrap_or(1.0);
        let dmg_mult = economy::monster_damage_mult(mob.level) * if mob.elite { ELITE_DAMAGE } else { 1.0 } * ghost_mult * rules_dmg;
        let damage_amount = def.damage * dmg_mult;
        ai.phase += DT;
        ai.cooldown -= DT;
        ai.attack_anim = (ai.attack_anim - DT).max(0.0);
        ai.retarget -= DT;
        ai.hop_timer -= DT;

        // --- Toppled boss / kneeling golem: lies there, vulnerable.
        if ai.down_timer > 0.0 {
            ai.down_timer -= DT;
            ai.windup = 0.0;
            if ai.down_timer <= 0.0 {
                // The legs grow back: break them again!
                if let Some(parts) = parts.as_mut() {
                    parts.broken = 0;
                    ai.leg_hp = [health.max * LEG_HP; 4];
                }
                ai.cooldown = 1.0;
            }
            state.anim = MobAnim::Toppled;
            state.tick = tick.0;
            history.record(tick.0, state.pos);
            continue;
        }

        if frozen {
            if let Some((_, tpos, _)) = targets.first() {
                state.yaw = yaw_toward(Vec3::new(tpos.x - state.pos.x, 0.0, tpos.z - state.pos.z));
            }
            state.anim = MobAnim::Move;
            state.tick = tick.0;
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
            // Keeps a distance band from the target, strafing in between.
            let keep_away = |near: f32, far: f32, speed: f32, phase: f32| {
                if dist > far {
                    dir * speed
                } else if dist < near {
                    -dir * speed
                } else {
                    side * (phase * 0.4).sin().signum() * speed * 0.6
                }
            };

            // --- Movement intention per type.
            want = match mob.kind {
                // Ghosts fly straight at you, through everything, faster every minute.
                MobKind::WhiteGhost | MobKind::BlackGhost => {
                    let wobble = Vec3::new((ai.phase * 2.3).sin(), (ai.phase * 1.7).cos() * 0.5, (ai.phase * 1.9).cos()) * 0.25;
                    ((chest - (state.pos + Vec3::Y * def.height * 0.5)).normalize_or_zero() + wobble).normalize_or_zero()
                        * def.speed
                        * ghost_mult.powf(0.35).min(3.0)
                }
                MobKind::Skitter => (dir + side * (ai.phase * 3.0).sin() * 0.5).normalize_or_zero() * def.speed,
                MobKind::Spitter | MobKind::Thornback => keep_away(10.0, 18.0, def.speed, ai.phase),
                MobKind::Mortar => keep_away(18.0, 30.0, def.speed, ai.phase),
                MobKind::CrystalGolem => keep_away(12.0, 22.0, def.speed, ai.phase),
                MobKind::Wisp => {
                    // Circle around the target, a few meters above.
                    let angle = ai.phase * 0.5;
                    let goal = tpos + Vec3::new(angle.cos() * 11.0, 5.5 + (ai.phase * 1.3).sin(), angle.sin() * 11.0);
                    (goal - state.pos).clamp_length_max(def.speed)
                }
                MobKind::Eye => {
                    // Hovers far away; stays still while charging its laser.
                    let angle = ai.phase * 0.25;
                    let goal = tpos + Vec3::new(angle.cos() * 17.0, 6.0 + (ai.phase * 1.1).sin(), angle.sin() * 17.0);
                    (goal - state.pos).clamp_length_max(def.speed)
                }
                MobKind::SkyJelly => {
                    // Drifts right above you.
                    let goal = tpos + tvel * 0.5 + Vec3::new((ai.phase * 0.7).cos() * 2.0, 9.0, (ai.phase * 0.7).sin() * 2.0);
                    (goal - state.pos).clamp_length_max(def.speed)
                }
                MobKind::FireBat => {
                    let wobble = side * (ai.phase * 5.0).sin() * 0.4;
                    ((chest - state.pos).normalize_or_zero() + wobble).normalize_or_zero() * def.speed
                }
                MobKind::Shaman | MobKind::Summoner => {
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
                MobKind::Broodmother => keep_away(10.0, 16.0, def.speed, ai.phase),
                MobKind::StormTitan | MobKind::AncientGolem if dist < 12.0 => Vec3::ZERO,
                MobKind::Charger | MobKind::LavaBoar if dist < 6.0 => side * def.speed,
                MobKind::Flea => {
                    // Big hops toward the target; keeps its momentum in the air.
                    if ai.grounded && ai.hop_timer <= 0.0 && dist > 1.5 {
                        ai.hop_timer = 0.8;
                        ai.vel = dir * def.speed * 1.9 + Vec3::Y * 8.5;
                        ai.grounded = false;
                    }
                    if ai.grounded { dir * def.speed * 0.4 } else { Vec3::new(ai.vel.x, 0.0, ai.vel.z) }
                }
                MobKind::Burrower => {
                    if ai.burrowed {
                        ai.state_timer -= DT;
                        if dist < 2.4 && ai.state_timer <= 0.0 {
                            // Pops out right next to you.
                            ai.burrowed = false;
                            ai.state_timer = 2.2;
                            ai.cooldown = 0.0;
                        }
                        dir * def.speed
                    } else {
                        ai.state_timer -= DT;
                        if ai.state_timer <= 0.0 && ai.windup <= 0.0 {
                            ai.burrowed = true;
                            ai.state_timer = 1.2;
                        }
                        dir * def.speed * 0.3
                    }
                }
                MobKind::Sandworm => {
                    if ai.emerge_timer > 0.0 {
                        // The circle is on the ground: it is coming up there.
                        ai.emerge_timer -= DT;
                        if ai.emerge_timer <= 0.0 {
                            state.pos = ai.aim;
                            ai.burrowed = false;
                            ai.state_timer = 7.0;
                            ai.cooldown = 1.2;
                        }
                        Vec3::ZERO
                    } else if ai.burrowed {
                        ai.state_timer -= DT;
                        if ai.state_timer <= 0.0 {
                            ai.aim = Vec3::new(tpos.x + tvel.x * 0.6, 0.0, tpos.z + tvel.z * 0.6);
                            ai.aim.y = terrain.wgen.ground_height(ai.aim.x, ai.aim.z);
                            ai.emerge_timer = 1.4;
                            commands.spawn((
                                Replicated,
                                Hazard::enemy(HazardKind::Blast, ai.aim, tick.0, 4.5, 1.4),
                                HazardServer::new(damage_amount * 1.3),
                            ));
                        }
                        dir * def.speed
                    } else {
                        ai.state_timer -= DT;
                        if ai.state_timer <= 0.0 && ai.windup <= 0.0 {
                            ai.burrowed = true;
                            ai.state_timer = 3.5;
                        }
                        Vec3::ZERO
                    }
                }
                _ => dir * def.speed,
            };

            // --- Attacks: wind-up (telegraph) then strike.
            let in_range = match mob.kind {
                MobKind::SkyJelly => dist < def.attack_range,
                MobKind::Burrower | MobKind::Sandworm => !ai.burrowed && ai.emerge_timer <= 0.0 && to.length() < def.attack_range,
                _ if def.ranged => dist < def.attack_range,
                _ => to.length() < def.attack_range,
            };
            if ai.windup <= 0.0 && ai.cooldown <= 0.0 && ai.charge_timer <= 0.0 && in_range {
                ai.windup = def.windup;
                // The charger aims when it starts snorting: dodge sideways!
                ai.charge_dir = dir;
                ai.aim = chest;
                fx.write(ToClients { targets: SendTargets::All, message: Fx::MobAttack { pos: state.pos, kind: mob.kind } });
            }
            if ai.windup > 0.0 {
                want *= if mob.kind == MobKind::Flea { 1.0 } else { 0.15 };
                // The eye follows you, then locks its aim just before firing.
                if mob.kind == MobKind::Eye && ai.windup > 0.45 {
                    ai.aim = chest;
                }
                ai.windup -= DT;
                if ai.windup <= 0.0 {
                    ai.cooldown = def.attack_cooldown;
                    ai.attack_anim = 0.35;
                    let eye = state.pos + Vec3::Y * def.height * 0.7;
                    ai.attacks += 1;
                    let melee_hit = |damage: &mut MessageWriter<DamagePlayer>, reach: f32| {
                        if to.length() < def.attack_range * reach {
                            damage.write(DamagePlayer { target: target_entity, amount: damage_amount, attacker: Some(entity) });
                        }
                    };
                    match mob.kind {
                        MobKind::Charger | MobKind::LavaBoar => {
                            ai.charge_timer = 1.1;
                            ai.charge_hits.clear();
                        }
                        MobKind::Skitter
                        | MobKind::WhiteGhost
                        | MobKind::BlackGhost
                        | MobKind::Flea
                        | MobKind::Scarab
                        | MobKind::Burrower
                        | MobKind::Jelly
                        | MobKind::Jellet
                        | MobKind::ToxicJelly
                        | MobKind::MagmaJelly => melee_hit(&mut damage, 1.5),
                        MobKind::ShieldKnight => melee_hit(&mut damage, 1.4),
                        MobKind::MegaJelly => explode_at(&mut explode, state.pos + dir * 1.5, 3.5, damage_amount),
                        MobKind::Shaman => {
                            heal.write(HealMobs { pos: state.pos, radius: 14.0, fraction: 0.2 });
                            fx.write(ToClients { targets: SendTargets::All, message: Fx::Heal { pos: state.pos, radius: 14.0 } });
                            let vel = (chest - eye).normalize_or_zero() * 13.0;
                            spawn_enemy_projectile(&mut commands, ProjectileKind::Bolt, eye, vel, 0.0, damage_amount, 0.0, tick.0);
                        }
                        MobKind::Summoner => {
                            if crowd < 150 {
                                for i in 0..3 {
                                    let a = i as f32 * 2.1 + ai.phase;
                                    let x = state.pos.x + a.cos() * 2.5;
                                    let z = state.pos.z + a.sin() * 2.5;
                                    let pos = Vec3::new(x, terrain.wgen.ground_height(x, z) + 0.3, z);
                                    spawn_mob(&mut commands, MobKind::Flea, pos, mob.level, tick.0, a, false);
                                    fx.write(ToClients { targets: SendTargets::All, message: Fx::Warp { from: eye, to: pos } });
                                }
                            }
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
                                commands.spawn((Replicated, Hazard::boss(HazardKind::Shockwave, state.pos, tick.0), HazardServer::new(damage_amount)));
                            } else {
                                // Marked lightning strikes around the target.
                                for i in 0..4 {
                                    let a = i as f32 * 1.7 + ai.phase;
                                    let r = if i == 0 { 0.0 } else { 4.0 + i as f32 * 1.5 };
                                    let x = tpos.x + tvel.x * 0.8 + a.cos() * r;
                                    let z = tpos.z + tvel.z * 0.8 + a.sin() * r;
                                    let pos = Vec3::new(x, terrain.wgen.ground_height(x, z), z);
                                    commands.spawn((Replicated, Hazard::boss(HazardKind::Lightning, pos, tick.0), HazardServer::new(damage_amount)));
                                }
                            }
                        }
                        MobKind::Brute | MobKind::StoneGolem => explode_at(&mut explode, state.pos + dir * 2.0, 4.5, damage_amount),
                        MobKind::Bomber | MobKind::FireBat => {
                            explode_at(&mut explode, state.pos + Vec3::Y * 0.6, 4.5, damage_amount);
                            commands.entity(entity).try_despawn();
                            continue;
                        }
                        MobKind::Sporeling => {
                            explode_at(&mut explode, state.pos + Vec3::Y * 0.6, 2.5, damage_amount);
                            on_mob_death(&mut commands, mob, state.pos, tick.0, rules_dmg);
                            commands.entity(entity).try_despawn();
                            continue;
                        }
                        MobKind::Spitter => {
                            let flight = (dist / 16.0).clamp(0.6, 1.6);
                            let vel = lob(eye, chest + tvel * flight * 0.5, flight, ACID_GRAVITY);
                            spawn_enemy_projectile(&mut commands, ProjectileKind::Acid, eye, vel, ACID_GRAVITY, damage_amount, 1.5, tick.0);
                        }
                        MobKind::Wisp => {
                            let vel = (chest - eye).normalize_or_zero() * 24.0;
                            spawn_enemy_projectile(&mut commands, ProjectileKind::Bolt, eye, vel, 0.0, damage_amount, 0.0, tick.0);
                        }
                        MobKind::Thornback => {
                            // A ring of thorns all around: jump or find the gap.
                            for i in 0..12 {
                                let a = i as f32 / 12.0 * std::f32::consts::TAU + ai.phase;
                                let vel = Vec3::new(a.cos(), 0.05, a.sin()) * 16.0;
                                spawn_enemy_projectile(&mut commands, ProjectileKind::Thorn, eye, vel, 0.0, damage_amount, 0.0, tick.0);
                            }
                        }
                        MobKind::Mortar => {
                            let mut at = tpos + tvel * 1.2;
                            at.y = terrain.wgen.ground_height(at.x, at.z);
                            commands.spawn((Replicated, Hazard::enemy(HazardKind::Blast, at, tick.0, 3.2, 1.6), HazardServer::new(damage_amount)));
                        }
                        MobKind::Eye => {
                            // Laser along the locked aim: whoever is on the line gets hit.
                            let beam_dir = (ai.aim - eye).normalize_or_zero();
                            let end = eye + beam_dir * 45.0;
                            fx.write(ToClients { targets: SendTargets::All, message: Fx::EnemyBeam { from: eye, to: end } });
                            for (pe, ppos, _) in &targets {
                                let c = *ppos + Vec3::Y * (PLAYER_HEIGHT * 0.5);
                                let along = (c - eye).dot(beam_dir).clamp(0.0, 45.0);
                                if (eye + beam_dir * along).distance(c) < 1.1 {
                                    damage.write(DamagePlayer { target: *pe, amount: damage_amount, attacker: None });
                                }
                            }
                        }
                        MobKind::SkyJelly => {
                            let vel = Vec3::new(tvel.x, 0.0, tvel.z) * 0.6;
                            spawn_enemy_projectile(&mut commands, ProjectileKind::Acid, state.pos, vel, ACID_GRAVITY, damage_amount, 2.5, tick.0);
                        }
                        MobKind::CrystalGolem => {
                            for i in -1..=1 {
                                let aim = chest + side * (i as f32 * 1.8) + tvel * 0.3;
                                let vel = (aim - eye).normalize_or_zero() * 26.0;
                                spawn_enemy_projectile(&mut commands, ProjectileKind::Shard, eye, vel, 0.0, damage_amount, 0.0, tick.0);
                            }
                        }
                        MobKind::Colossus => {
                            if dist < 9.0 {
                                explode_at(&mut explode, state.pos, 9.0, damage_amount);
                            } else {
                                for i in -2..=2 {
                                    let flight = (dist / 20.0).clamp(0.8, 2.0);
                                    let aim = tpos + side * (i as f32 * 3.0) + tvel * flight * 0.6;
                                    let vel = lob(eye, aim, flight, ACID_GRAVITY);
                                    spawn_enemy_projectile(&mut commands, ProjectileKind::BossOrb, eye, vel, ACID_GRAVITY, damage_amount * 0.6, 3.0, tick.0);
                                }
                            }
                        }
                        MobKind::Broodmother => {
                            if ai.attacks.is_multiple_of(3) {
                                // Spiderlings.
                                if crowd < 150 {
                                    for i in 0..5 {
                                        let a = i as f32 * 1.25 + ai.phase;
                                        let x = state.pos.x + a.cos() * 3.5;
                                        let z = state.pos.z + a.sin() * 3.5;
                                        let pos = Vec3::new(x, terrain.wgen.ground_height(x, z) + 0.3, z);
                                        let kind = if i % 2 == 0 { MobKind::Skitter } else { MobKind::Flea };
                                        spawn_mob(&mut commands, kind, pos, mob.level, tick.0, a, false);
                                    }
                                }
                            } else {
                                // Acid rain around the target.
                                for i in 0..6 {
                                    let a = i as f32 * 1.05 + ai.phase;
                                    let r = if i == 0 { 0.0 } else { 3.5 };
                                    let flight = (dist / 18.0).clamp(0.8, 1.8);
                                    let aim = tpos + tvel * flight * 0.5 + Vec3::new(a.cos() * r, 0.0, a.sin() * r);
                                    let vel = lob(eye, aim, flight, ACID_GRAVITY);
                                    spawn_enemy_projectile(&mut commands, ProjectileKind::Acid, eye, vel, ACID_GRAVITY, damage_amount * 0.7, 2.0, tick.0);
                                }
                            }
                        }
                        MobKind::AncientGolem => {
                            if ai.attacks.is_multiple_of(2) {
                                // Slam: shockwave + blast... then it kneels, out of breath.
                                commands.spawn((Replicated, Hazard::boss(HazardKind::Shockwave, state.pos, tick.0), HazardServer::new(damage_amount)));
                                explode_at(&mut explode, state.pos + dir * 3.0, 6.0, damage_amount);
                                ai.down_timer = 3.5;
                            } else {
                                for i in -1..=1 {
                                    let flight = (dist / 20.0).clamp(0.8, 2.0);
                                    let aim = tpos + side * (i as f32 * 4.0) + tvel * flight * 0.6;
                                    let vel = lob(eye, aim, flight, ACID_GRAVITY);
                                    spawn_enemy_projectile(&mut commands, ProjectileKind::BossOrb, eye, vel, ACID_GRAVITY, damage_amount * 0.7, 3.5, tick.0);
                                }
                            }
                        }
                        MobKind::Sandworm => {
                            let mouth = state.pos + Vec3::Y * def.height * 0.9;
                            if ai.attacks.is_multiple_of(2) {
                                for i in -3..=3 {
                                    let flight = (dist / 18.0).clamp(0.8, 1.8);
                                    let aim = tpos + side * (i as f32 * 2.2) + tvel * flight * 0.5;
                                    let vel = lob(mouth, aim, flight, ACID_GRAVITY);
                                    spawn_enemy_projectile(&mut commands, ProjectileKind::Acid, mouth, vel, ACID_GRAVITY, damage_amount * 0.6, 2.0, tick.0);
                                }
                            } else {
                                for i in 0..16 {
                                    let a = i as f32 / 16.0 * std::f32::consts::TAU;
                                    let vel = Vec3::new(a.cos(), -0.12, a.sin()) * 18.0;
                                    spawn_enemy_projectile(&mut commands, ProjectileKind::Thorn, mouth, vel, 0.0, damage_amount * 0.5, 0.0, tick.0);
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

        // --- Charge: locked direction, very fast, hurts on contact.
        if ai.charge_timer > 0.0 {
            ai.charge_timer -= DT;
            want = ai.charge_dir * 24.0;
            for (pe, ppos, _) in &targets {
                if !ai.charge_hits.contains(pe) && ppos.distance(state.pos) < def.radius + 1.0 {
                    ai.charge_hits.push(*pe);
                    damage.write(DamagePlayer { target: *pe, amount: damage_amount, attacker: Some(entity) });
                }
            }
            // The lava boar leaves burning lava behind it.
            if mob.kind == MobKind::LavaBoar && ai.hop_timer <= 0.0 {
                ai.hop_timer = 0.12;
                commands.spawn((
                    Replicated,
                    Hazard::enemy(HazardKind::LavaPool, state.pos, tick.0, 1.3, 4.5),
                    HazardServer::new(damage_amount * 0.25),
                ));
            }
        }

        // --- Physics.
        if ai.burrowed {
            // Underground: glides along the ground, through obstacles.
            state.pos += Vec3::new(want.x, 0.0, want.z) * DT;
            state.pos.y = terrain.wgen.ground_height(state.pos.x, state.pos.z);
            ai.vel = Vec3::ZERO;
            ai.grounded = true;
        } else if def.flying {
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

        state.anim = if ai.down_timer > 0.0 {
            MobAnim::Toppled
        } else if ai.burrowed {
            MobAnim::Burrowed
        } else if ai.windup > 0.0 {
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
    let size = match kind {
        ProjectileKind::BossOrb => 0.9,
        ProjectileKind::Shard => 0.4,
        ProjectileKind::Thorn => 0.3,
        _ => 0.35,
    };
    commands.spawn((
        Replicated,
        Projectile { kind, origin, vel, gravity, spawn_tick: tick, owner_slot: 255, size, life: 0.0 },
        ProjectileServer::enemy(damage, splash, 5.0),
    ));
}

/// Keeps monsters from stacking on top of each other.
fn separate_mobs(mut mobs: Query<(&Mob, &mut MobState)>) {
    let snapshot: Vec<(Vec3, f32)> = mobs.iter().map(|(m, s)| (s.pos, m.kind.def().radius)).collect();
    for (i, (mob, mut state)) in mobs.iter_mut().enumerate() {
        if state.anim == MobAnim::Burrowed || state.anim == MobAnim::Toppled {
            continue;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn early_monsters_fade_late() {
        assert!(spawn_weight(MobKind::Skitter, 12.0) < spawn_weight(MobKind::Skitter, 3.0));
        assert_eq!(spawn_weight(MobKind::ShieldKnight, 12.0), MobKind::ShieldKnight.def().spawn_weight);
    }

    #[test]
    fn each_run_has_its_own_roster() {
        let a = Director::new(1, MapKind::Meadow);
        let b = Director::new(2, MapKind::Meadow);
        assert!(a.roster != b.roster || a.bosses != b.bosses);
        // Same seed, same run (everyone sees the same thing).
        assert_eq!(Director::new(7, MapKind::Swamp).roster, Director::new(7, MapKind::Swamp).roster);
        // Each map has its own bosses.
        let desert = Director::new(3, MapKind::Embers);
        assert!(desert.bosses.iter().all(|b| MapKind::Embers.bosses().contains(b)));
    }
}
