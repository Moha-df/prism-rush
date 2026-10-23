//! "Juice": particles, tracers, impact flashes, explosions, floating damage
//! numbers, screen shake and sounds, triggered by gameplay events.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::audio::{PlaySfx, Sfx};
use super::camera::CameraFx;
use super::model::{Prim, PrimCache};
use super::palette;
use super::MainCamera;
use crate::game::world::TerrainRes;
use crate::net::client::{InputState, PredictedEvents, Prediction};
use crate::net::protocol::{player_color, Fx, GameEvent, MobState, Mob};
use crate::net::LocalPlayer;
use crate::shared::mobs::MobKind;
use crate::shared::rng::Rng;
use crate::shared::weapons::WeaponKind;

#[derive(Component)]
struct Particle {
    vel: Vec3,
    life: f32,
    max_life: f32,
    gravity: f32,
    drag: f32,
    size: f32,
}

#[derive(Component)]
struct Tracer {
    life: f32,
    max_life: f32,
    width: f32,
}

#[derive(Component)]
struct FlashLight {
    life: f32,
    max_life: f32,
    intensity: f32,
}

/// Floating damage number (UI node placed over a world position).
#[derive(Component)]
pub struct DamageNumber {
    world: Vec3,
    life: f32,
    max_life: f32,
}

/// HUD feedback written here, displayed by `ui::hud`.
#[derive(Resource, Default, Debug)]
pub struct HudFeedback {
    pub hitmarker: f32,
    pub hitmarker_crit: bool,
    pub hitmarker_kill: bool,
    pub hurt_flash: f32,
}

#[derive(Resource)]
struct FxRng(Rng);

pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HudFeedback>()
            .insert_resource(FxRng(Rng::new(1234)))
            .add_systems(
                Update,
                (
                    on_server_fx,
                    on_game_events,
                    on_predicted_events,
                    update_particles,
                    update_tracers,
                    update_flash_lights,
                    update_damage_numbers,
                    decay_feedback,
                ),
            );
    }
}

/// Spawns a burst of particles.
#[allow(clippy::too_many_arguments)]
fn burst(
    commands: &mut Commands,
    cache: &mut PrimCache,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    rng: &mut Rng,
    pos: Vec3,
    count: usize,
    color: Color,
    glow: f32,
    speed: f32,
    size: f32,
    life: f32,
    gravity: f32,
) {
    let mesh = cache.mesh(meshes, Prim::Ico, Vec3::ONE);
    let mat = cache.material(materials, color, glow);
    for _ in 0..count {
        let dir = Vec3::new(rng.range(-1.0, 1.0), rng.range(-0.3, 1.0), rng.range(-1.0, 1.0)).normalize_or_zero();
        let s = size * rng.range(0.6, 1.3);
        commands.spawn((
            Particle {
                vel: dir * speed * rng.range(0.4, 1.0),
                life: life * rng.range(0.7, 1.2),
                max_life: life,
                gravity,
                drag: 2.0,
                size: s,
            },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(pos).with_scale(Vec3::splat(s)),
            NotShadowCaster,
        ));
    }
}

fn tracer(
    commands: &mut Commands,
    cache: &mut PrimCache,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    from: Vec3,
    to: Vec3,
    color: Color,
    width: f32,
) {
    let len = from.distance(to);
    if len < 0.1 {
        return;
    }
    let mesh = cache.mesh(meshes, Prim::Box, Vec3::ONE);
    let mat = cache.material(materials, color, 6.0);
    commands.spawn((
        Tracer { life: 0.07, max_life: 0.07, width },
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        Transform::from_translation((from + to) * 0.5)
            .looking_at(to, Vec3::Y)
            .with_scale(Vec3::new(width, width, len)),
        NotShadowCaster,
    ));
}

fn flash_light(commands: &mut Commands, pos: Vec3, color: Color, intensity: f32, range: f32, life: f32) {
    commands.spawn((
        FlashLight { life, max_life: life, intensity },
        PointLight { color, intensity, range, shadow_maps_enabled: false, ..default() },
        Transform::from_translation(pos),
    ));
}

fn damage_number(commands: &mut Commands, pos: Vec3, amount: u16, crit: bool, rng: &mut Rng) {
    let (size, color) = if crit { (30.0, Color::srgb(1.0, 0.85, 0.2)) } else { (20.0, Color::WHITE) };
    commands.spawn((
        DamageNumber {
            world: pos + Vec3::new(rng.range(-0.3, 0.3), 0.3, rng.range(-0.3, 0.3)),
            life: 0.8,
            max_life: 0.8,
        },
        Text::new(if crit { format!("{amount}!") } else { amount.to_string() }),
        crate::ui::widgets::bold(size),
        TextColor(color),
        TextShadow::default(),
        Node { position_type: PositionType::Absolute, ..default() },
        Visibility::Hidden,
    ));
}

fn weapon_tracer_color(kind: WeaponKind) -> Color {
    match kind {
        WeaponKind::Sniper => palette::GLOW_CYAN,
        WeaponKind::RocketLauncher => palette::GLOW_ORANGE,
        _ => Color::srgb(1.0, 0.85, 0.5),
    }
}

#[allow(clippy::too_many_arguments)]
fn on_server_fx(
    mut commands: Commands,
    mut fx_in: MessageReader<Fx>,
    mut cache: ResMut<PrimCache>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<FxRng>,
    mut sfx: MessageWriter<PlaySfx>,
    mut cam_fx: ResMut<CameraFx>,
    mut feedback: ResMut<HudFeedback>,
    local: Res<LocalPlayer>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
) {
    let cam = camera.single().map(|c| c.translation()).unwrap_or_default();
    let rng = &mut rng.0;
    for fx in fx_in.read() {
        match fx {
            Fx::Hit { pos, amount, crit, killed, by_slot } => {
                let color = if *crit { palette::GOLD } else { Color::srgb(1.0, 0.9, 0.7) };
                burst(&mut commands, &mut cache, &mut meshes, &mut materials, rng, *pos, 5, color, 5.0, 6.0, 0.07, 0.25, 9.0);
                damage_number(&mut commands, *pos, *amount, *crit, rng);
                if *killed {
                    burst(&mut commands, &mut cache, &mut meshes, &mut materials, rng, *pos, 12, palette::PLUM, 0.0, 7.0, 0.22, 0.8, 18.0);
                    burst(&mut commands, &mut cache, &mut meshes, &mut materials, rng, *pos, 8, palette::GLOW_MAGENTA, 4.0, 5.0, 0.1, 0.5, 4.0);
                    burst(&mut commands, &mut cache, &mut meshes, &mut materials, rng, *pos + Vec3::Y * 0.5, 5, palette::GOLD, 3.0, 4.0, 0.09, 0.9, 12.0);
                    sfx.write(PlaySfx::at(Sfx::Kill, *pos));
                    sfx.write(PlaySfx { volume: 0.6, ..PlaySfx::ui(Sfx::Coin) });
                }
                if Some(*by_slot) == local.slot {
                    feedback.hitmarker = 0.18;
                    feedback.hitmarker_crit = *crit;
                    feedback.hitmarker_kill = *killed;
                    sfx.write(PlaySfx::ui(if *crit { Sfx::Crit } else { Sfx::Hit }));
                }
            }
            Fx::Tracer { slot, weapon, from, to } => {
                // Other players' shots (ours are predicted locally).
                let _ = player_color(*slot);
                tracer(&mut commands, &mut cache, &mut meshes, &mut materials, *from, *to, weapon_tracer_color(*weapon), 0.04);
                sfx.write(PlaySfx { volume: 0.7, ..PlaySfx::at(Sfx::Shot(*weapon), *from) });
            }
            Fx::Explosion { pos, radius } => {
                let r = *radius;
                burst(&mut commands, &mut cache, &mut meshes, &mut materials, rng, *pos, 22, palette::GLOW_ORANGE, 6.0, r * 3.5, 0.35, 0.45, 2.0);
                burst(&mut commands, &mut cache, &mut meshes, &mut materials, rng, *pos, 10, Color::srgb(1.0, 0.9, 0.5), 8.0, r * 2.0, 0.25, 0.25, 0.0);
                burst(&mut commands, &mut cache, &mut meshes, &mut materials, rng, *pos, 12, Color::srgb(0.35, 0.32, 0.36), 0.0, r * 1.5, 0.5, 1.2, -1.5);
                flash_light(&mut commands, *pos + Vec3::Y, palette::GLOW_ORANGE, 2_000_000.0 * r, r * 6.0, 0.3);
                sfx.write(PlaySfx::at(Sfx::Explosion, *pos));
                let d = pos.distance(cam);
                cam_fx.add_trauma((0.7 - d / 40.0).max(0.0));
            }
            Fx::MobAttack { pos, kind } => {
                let s = if *kind == MobKind::Colossus { Sfx::BossRoar } else { Sfx::Telegraph };
                sfx.write(PlaySfx { volume: 0.7, ..PlaySfx::at(s, *pos) });
            }
            Fx::PlayerHurt { slot, amount } => {
                if Some(*slot) == local.slot {
                    cam_fx.add_trauma(0.15 + (*amount as f32 / 60.0).min(0.4));
                    feedback.hurt_flash = 0.5;
                    sfx.write(PlaySfx::ui(Sfx::Hurt));
                }
            }
            Fx::Gold { .. } => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn on_game_events(
    mut commands: Commands,
    mut events: MessageReader<GameEvent>,
    mut cache: ResMut<PrimCache>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<FxRng>,
    mut sfx: MessageWriter<PlaySfx>,
    local: Res<LocalPlayer>,
) {
    let rng = &mut rng.0;
    for ev in events.read() {
        match ev {
            GameEvent::ChestOpened { pos, slot, .. } => {
                let p = *pos + Vec3::Y * 0.6;
                burst(&mut commands, &mut cache, &mut meshes, &mut materials, rng, p, 25, palette::GOLD, 4.0, 7.0, 0.1, 1.2, 9.0);
                burst(&mut commands, &mut cache, &mut meshes, &mut materials, rng, p, 15, player_color(*slot), 4.0, 5.0, 0.1, 1.0, 6.0);
                flash_light(&mut commands, p + Vec3::Y, palette::GOLD, 400_000.0, 12.0, 0.8);
                // A beam of light rising from the chest.
                tracer(&mut commands, &mut cache, &mut meshes, &mut materials, p, p + Vec3::Y * 12.0, palette::GOLD, 0.25);
                sfx.write(PlaySfx::at(Sfx::ChestOpen, *pos));
            }
            GameEvent::Downed { slot } => {
                sfx.write(PlaySfx { volume: if Some(*slot) == local.slot { 1.0 } else { 0.6 }, ..PlaySfx::ui(Sfx::Downed) });
            }
            GameEvent::Revived { .. } => {
                sfx.write(PlaySfx::ui(Sfx::Revived));
            }
            GameEvent::BossSpawned => {
                sfx.write(PlaySfx::ui(Sfx::BossRoar));
            }
            GameEvent::BossKilled => {
                sfx.write(PlaySfx::ui(Sfx::ChestOpen));
            }
            GameEvent::GameOver(_) => {}
        }
    }
}

/// Our own predicted actions: immediate feedback, no network delay.
#[allow(clippy::too_many_arguments)]
fn on_predicted_events(
    mut commands: Commands,
    mut events: MessageReader<PredictedEvents>,
    mut cache: ResMut<PrimCache>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<FxRng>,
    mut sfx: MessageWriter<PlaySfx>,
    pred: Res<Prediction>,
    input: Res<InputState>,
    terrain: Res<TerrainRes>,
    mobs: Query<(&Mob, &Transform), With<MobState>>,
) {
    let rng = &mut rng.0;
    for PredictedEvents(ev) in events.read() {
        let w = &ev.weapons;
        if let Some(kind) = w.fired {
            sfx.write(PlaySfx { volume: 0.8, ..PlaySfx::ui(Sfx::Shot(kind)) });
        }
        // Tracers start at the (approximate) muzzle on screen.
        let fwd = Quat::from_euler(EulerRot::YXZ, input.yaw, input.pitch, 0.0);
        let muzzle = pred.sim.eye() + fwd * Vec3::new(0.2, -0.15, -0.7);
        for shot in w.shots.iter().take(9) {
            if shot.weapon == WeaponKind::RocketLauncher {
                continue; // the rocket itself is the visual
            }
            let range = shot.weapon.def().range;
            let mut dist = terrain.raycast(shot.origin, shot.dir, range).unwrap_or(range);
            // Stop the tracer on monsters too (approximate, visual only).
            for (mob, t) in &mobs {
                let def = mob.kind.def();
                let center = t.translation + Vec3::Y * def.height * 0.5;
                let to = center - shot.origin;
                let along = to.dot(shot.dir);
                if along > 0.0 && along < dist && (to - shot.dir * along).length() < def.radius.max(def.height * 0.4) {
                    dist = along;
                }
            }
            let end = shot.origin + shot.dir * dist;
            tracer(&mut commands, &mut cache, &mut meshes, &mut materials, muzzle, end, weapon_tracer_color(shot.weapon), 0.03);
            if dist < range {
                burst(&mut commands, &mut cache, &mut meshes, &mut materials, rng, end, 3, Color::srgb(1.0, 0.85, 0.5), 4.0, 4.0, 0.05, 0.2, 8.0);
            }
        }
        if w.reload_started {
            sfx.write(PlaySfx::ui(Sfx::Reload));
        }
        if w.reload_finished {
            sfx.write(PlaySfx { volume: 0.7, ..PlaySfx::ui(Sfx::Reload) });
        }
        if w.dry_fire {
            sfx.write(PlaySfx::ui(Sfx::Empty));
        }
        if w.switched {
            sfx.write(PlaySfx::ui(Sfx::Switch));
        }
        let m = &ev.mv;
        let feet = pred.sim.mv.pos;
        if m.jumped || m.air_jumped {
            sfx.write(PlaySfx::ui(Sfx::Jump));
            if m.air_jumped {
                burst(&mut commands, &mut cache, &mut meshes, &mut materials, rng, feet, 8, Color::srgb(0.9, 0.95, 1.0), 2.0, 4.0, 0.08, 0.35, 0.0);
            }
        }
        if m.landed {
            sfx.write(PlaySfx::ui(Sfx::Land));
            burst(&mut commands, &mut cache, &mut meshes, &mut materials, rng, feet, 6, Color::srgb(0.75, 0.7, 0.6), 0.0, 3.0, 0.12, 0.4, 2.0);
        }
        if m.dashed || m.slide_started {
            sfx.write(PlaySfx { volume: if m.dashed { 1.0 } else { 0.5 }, ..PlaySfx::ui(Sfx::Dash) });
        }
        if m.pad_launched {
            sfx.write(PlaySfx::ui(Sfx::Pad));
            burst(&mut commands, &mut cache, &mut meshes, &mut materials, rng, feet, 16, palette::TEAL_GLOW, 4.0, 6.0, 0.1, 0.6, -2.0);
        }
    }
}

fn update_particles(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Particle, &mut Transform)>) {
    let dt = time.delta_secs();
    for (e, mut p, mut t) in &mut q {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(e).try_despawn();
            continue;
        }
        p.vel.y -= p.gravity * dt;
        let drag = (-p.drag * dt).exp();
        p.vel *= drag;
        t.translation += p.vel * dt;
        t.rotate_y(dt * 5.0);
        let k = (p.life / p.max_life).clamp(0.0, 1.0);
        t.scale = Vec3::splat(p.size * k);
    }
}

fn update_tracers(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Tracer, &mut Transform)>) {
    for (e, mut tr, mut t) in &mut q {
        tr.life -= time.delta_secs();
        if tr.life <= 0.0 {
            commands.entity(e).try_despawn();
            continue;
        }
        let k = tr.life / tr.max_life;
        t.scale.x = tr.width * k;
        t.scale.y = tr.width * k;
    }
}

fn update_flash_lights(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut FlashLight, &mut PointLight)>) {
    for (e, mut f, mut light) in &mut q {
        f.life -= time.delta_secs();
        if f.life <= 0.0 {
            commands.entity(e).try_despawn();
            continue;
        }
        light.intensity = f.intensity * (f.life / f.max_life).powi(2);
    }
}

fn update_damage_numbers(
    mut commands: Commands,
    time: Res<Time>,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut q: Query<(Entity, &mut DamageNumber, &mut Node, &mut Visibility, &mut TextColor)>,
) {
    let Ok((camera, cam_t)) = camera.single() else { return };
    let dt = time.delta_secs();
    for (e, mut d, mut node, mut vis, mut color) in &mut q {
        d.life -= dt;
        if d.life <= 0.0 {
            commands.entity(e).try_despawn();
            continue;
        }
        d.world.y += dt * 1.5;
        match camera.world_to_viewport(cam_t, d.world) {
            Ok(screen) => {
                node.left = Val::Px(screen.x - 12.0);
                node.top = Val::Px(screen.y - 12.0);
                *vis = Visibility::Inherited;
            }
            Err(_) => *vis = Visibility::Hidden,
        }
        color.0.set_alpha((d.life / d.max_life * 2.0).min(1.0));
    }
}

fn decay_feedback(time: Res<Time>, mut f: ResMut<HudFeedback>) {
    let dt = time.delta_secs();
    f.hitmarker = (f.hitmarker - dt).max(0.0);
    f.hurt_flash = (f.hurt_flash - dt).max(0.0);
}
