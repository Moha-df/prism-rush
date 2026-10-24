//! Gives a visual model to every networked entity (other players, monsters,
//! chests, projectiles) and keeps their animation state up to date.

use bevy::prelude::*;

use super::model::{spawn_model, AnimTag, Part, PrimCache};
use super::palette;
use super::{models, monsters as models_v4};
use crate::net::interp::LocalAvatar;
use crate::net::protocol::*;
use crate::shared::mobs::MobKind;

/// The entity already has its visual model.
#[derive(Component)]
pub struct HasModel {
    /// Child entity holding the model (so we can tilt/remove it as a whole).
    pub model: Entity,
}

/// Short squash when a monster is hit.
#[derive(Component)]
pub struct HitPop(pub f32);

/// Everything animations need to know about an entity.
#[derive(Component, Debug, Default)]
pub struct AnimState {
    pub speed: f32,
    /// Walk cycle phase (radians).
    pub phase: f32,
    /// Stride length: bigger creatures take slower steps.
    pub stride: f32,
    pub action: MobAnim,
    pub action_time: f32,
    pub downed: bool,
    pub downed_amount: f32,
    /// Chest lid opening, 0..1.
    pub open: f32,
    /// Weapon recoil kick, 0..1 (view model).
    pub recoil: f32,
    pub last_pos: Option<Vec3>,
    /// Broken legs (bosses).
    pub broken: u8,
    /// How deep the model sinks when burrowed, and how far it is, 0..1.
    pub sink: f32,
    pub burrow: f32,
}

impl AnimState {
    pub fn with_stride(stride: f32) -> Self {
        Self { stride, ..default() }
    }
}

pub struct AvatarsPlugin;

impl Plugin for AvatarsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                attach_models,
                detach_player_models,
                place_chests,
                update_anim_states,
                grow_shockwaves,
                dim_used_statues,
                dim_used_totems,
                hit_pop,
                fill_shrines,
                show_mounds,
            )
                .chain(),
        );
    }
}

/// Shockwave rings grow with time (in the server's present, so what you see
/// is exactly when you must jump).
fn grow_shockwaves(
    clock: Res<crate::net::client::ServerClock>,
    hazards: Query<(&Hazard, &HasModel)>,
    children: Query<&Children>,
    mut models: Query<&mut Transform, Without<HasModel>>,
) {
    for (h, has) in &hazards {
        if h.kind == HazardKind::Tornado {
            // Tornados wander: move the model along their path.
            if let Ok(mut t) = models.get_mut(has.model) {
                t.translation = h.pos_at(clock.estimate) - h.pos;
            }
            continue;
        }
        if h.kind == HazardKind::Blast {
            let k = (h.age(clock.estimate) / h.life.max(0.1)).min(1.0);
            if let Ok(children) = children.get(has.model)
                && let Some(shell) = children.iter().nth(3)
                && let Ok(mut t) = models.get_mut(shell)
            {
                t.translation.y = (1.0 - k) * 25.0;
            }
            continue;
        }
        if h.kind != HazardKind::Shockwave {
            continue;
        }
        let age = ((clock.estimate - f64::from(h.spawn_tick)) / crate::shared::movement::TICK_HZ).max(0.0) as f32;
        let radius = (age * SHOCKWAVE_SPEED).max(0.1);
        if let Ok(mut t) = models.get_mut(has.model) {
            t.scale = Vec3::new(radius, 1.0, radius);
        }
    }
}

fn hit_pop(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut HitPop, &HasModel)>,
    mut models: Query<&mut Transform, Without<HasModel>>,
) {
    for (e, mut pop, has) in &mut q {
        pop.0 -= time.delta_secs();
        let k = (pop.0 / 0.12).max(0.0);
        if let Ok(mut t) = models.get_mut(has.model) {
            t.scale = Vec3::new(1.0 + 0.18 * k, 1.0 - 0.12 * k, 1.0 + 0.18 * k);
        }
        if pop.0 <= 0.0 {
            commands.entity(e).remove::<HitPop>();
        }
    }
}

/// Dirt mound of a burrowing monster (visible while it is underground).
#[derive(Component)]
struct BurrowMound;

fn show_mounds(states: Query<&AnimState>, mut mounds: Query<(&ChildOf, &mut Transform), With<BurrowMound>>) {
    for (parent, mut t) in &mut mounds {
        if let Ok(st) = states.get(parent.parent()) {
            t.scale = Vec3::splat(st.burrow.max(0.001));
        }
    }
}

/// The column inside a charge shrine (scaled to the charge).
#[derive(Component)]
struct ShrineFill;

fn fill_shrines(shrines: Query<(&ShrineCharge, &Children)>, mut fills: Query<&mut Transform, With<ShrineFill>>) {
    for (charge, children) in &shrines {
        for c in children.iter() {
            if let Ok(mut t) = fills.get_mut(c) {
                // Full height 2.6 m; a used shrine stays full.
                let h = if charge.used { 1.0 } else { charge.charge };
                t.scale.y = (h * 2.6).max(0.01);
            }
        }
    }
}

/// Awakened statues lose their magic glow (hidden model glow ring).
fn dim_used_statues(statues: Query<(&StatueUsed, &HasModel), Changed<StatueUsed>>, mut models: Query<&mut Transform, Without<HasModel>>) {
    for (used, has) in &statues {
        if used.0
            && let Ok(mut t) = models.get_mut(has.model)
        {
            // Sink the statue a little: it "spent" its power.
            t.translation.y = -0.6;
        }
    }
}

/// An activated totem sinks into the ground: its curse is now on the run.
fn dim_used_totems(totems: Query<(&TotemUsed, &HasModel), Changed<TotemUsed>>, mut models: Query<&mut Transform, Without<HasModel>>) {
    for (used, has) in &totems {
        if used.0
            && let Ok(mut t) = models.get_mut(has.model)
        {
            t.translation.y = -1.6;
        }
    }
}

fn spawn_child_model(
    commands: &mut Commands,
    cache: &mut PrimCache,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    root: Entity,
    part: &super::model::Part,
) -> Entity {
    let model = commands.spawn((Transform::IDENTITY, Visibility::default(), ChildOf(root))).id();
    spawn_model(commands, cache, meshes, materials, root, model, part, ());
    model
}

type NeedsModel = (Without<HasModel>, With<Transform>);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn attach_models(
    mut commands: Commands,
    mut cache: ResMut<PrimCache>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    players: Query<(Entity, &Player), (NeedsModel, With<PlayerState>, Without<LocalAvatar>)>,
    mobs: Query<(Entity, &Mob), NeedsModel>,
    chests: Query<(Entity, &Chest), NeedsModel>,
    projectiles: Query<(Entity, &Projectile), NeedsModel>,
    statues: Query<(Entity, &Statue), NeedsModel>,
    hazards: Query<(Entity, &Hazard), NeedsModel>,
    gems: Query<(Entity, &XpGem), NeedsModel>,
    shrines: Query<(Entity, &Shrine), NeedsModel>,
    powerups: Query<(Entity, &Powerup), NeedsModel>,
    totems: Query<(Entity, &Totem), NeedsModel>,
) {
    let mut attach = |commands: &mut Commands, entity: Entity, part: super::model::Part, stride: f32| {
        let model = spawn_child_model(commands, &mut cache, &mut meshes, &mut materials, entity, &part);
        commands.entity(entity).try_insert((HasModel { model }, AnimState::with_stride(stride)));
    };
    for (e, p) in &players {
        attach(&mut commands, e, models::player(player_color(p.slot)), 1.4);
    }
    let mut burrowers = Vec::new();
    for (e, m) in &mobs {
        let def = m.kind.def();
        // Elites: bigger, with a golden aura ring and a little crown.
        let model = models::mob_with_elite(m.kind, m.elite);
        attach(&mut commands, e, model, (def.height * 0.6).max(0.8));
        if matches!(m.kind, MobKind::Burrower | MobKind::Sandworm) {
            burrowers.push((e, def.radius, def.height));
        }
    }
    for (e, s) in &statues {
        attach(&mut commands, e, models::statue(s.boss), 1.0);
    }
    for (e, _) in &totems {
        attach(&mut commands, e, models::totem(), 1.0);
    }
    for (e, g) in &gems {
        attach(&mut commands, e, models::xp_gem(g.value), 1.0);
    }
    for (e, p) in &powerups {
        attach(&mut commands, e, models::powerup(p.kind), 1.0);
    }
    for (e, _) in &shrines {
        attach(&mut commands, e, models::shrine(), 1.0);
    }
    for (e, h) in &hazards {
        let part = match h.kind {
            // Unit ring, scaled every frame to the shockwave radius.
            HazardKind::Shockwave => Part::torus(2.0, 0.6).color(palette::GLOW_CYAN).glow(5.0),
            HazardKind::Lightning => Part::pivot()
                .with(Part::cylinder(LIGHTNING_RADIUS * 2.0, 0.06).color(palette::GLOW_MAGENTA).glow(2.5).tag(AnimTag::Pulse))
                .with(Part::torus(LIGHTNING_RADIUS * 2.0, 0.15).color(palette::GLOW_MAGENTA).glow(5.0)),
            HazardKind::Mine => Part::pivot()
                .with(Part::cylinder(0.7, 0.18).color(palette::METAL_DARK))
                .with(Part::sphere(0.22).at(0.0, 0.12, 0.0).color(palette::BOMB_RED).glow(5.0).tag(AnimTag::Pulse))
                .with(Part::torus(0.8, 0.04).at(0.0, 0.06, 0.0).color(palette::GLOW_ORANGE).glow(2.0)),
            HazardKind::Poison => {
                let d = h.radius * 2.0;
                Part::pivot()
                    .with(Part::cylinder(d, 0.04).color(Color::srgb(0.3, 0.75, 0.15)).glow(1.2))
                    .with(Part::torus(d, 0.1).color(palette::GLOW_TOXIC).glow(3.0).tag(AnimTag::Pulse))
                    .with_all((0..5).map(|i| {
                        let a = i as f32 * 1.3;
                        Part::sphere(0.25).at(a.cos() * h.radius * 0.5, 0.1, a.sin() * h.radius * 0.5).color(palette::GLOW_TOXIC).glow(4.0).tag(AnimTag::Bob)
                    }))
            }
            HazardKind::Fire => Part::pivot().tag(AnimTag::Pulse).with_all((0..4).map(|i| {
                let a = i as f32 * 1.7;
                let r = h.radius * 0.45;
                let s = 0.5 + 0.15 * i as f32;
                Part::cone(s * h.radius * 0.6, s * 1.2)
                    .at(a.cos() * r, s * 0.6, a.sin() * r)
                    .color(if i % 2 == 0 { palette::GLOW_ORANGE } else { Color::srgb(1.0, 0.85, 0.3) })
                    .glow(5.0)
            })),
            // Marked circle (pulsing) + a shell falling on it.
            HazardKind::Blast => {
                let d = h.radius * 2.0;
                Part::pivot()
                    .with(Part::cylinder(d, 0.05).color(palette::GLOW_ORANGE).glow(1.5).tag(AnimTag::Pulse))
                    .with(Part::torus(d, 0.18).color(palette::GLOW_ORANGE).glow(5.0))
                    .with(Part::torus(d * 0.35, 0.12).color(palette::GLOW_ORANGE).glow(4.0))
                    .with(Part::pivot().with(Part::sphere(0.8).color(palette::STONE_DARK)).with(Part::ico(0.4, 0.4, 0.4).at(0.0, 0.4, 0.0).color(palette::GLOW_ORANGE).glow(5.0)))
            }
            HazardKind::AcidPool | HazardKind::LavaPool => {
                let d = h.radius * 2.0;
                let (main, bright) = if h.kind == HazardKind::AcidPool {
                    (Color::srgb(0.35, 0.8, 0.1), palette::GLOW_TOXIC)
                } else {
                    (Color::srgb(0.9, 0.25, 0.05), palette::GLOW_ORANGE)
                };
                Part::pivot()
                    .with(Part::cylinder(d, 0.05).at(0.0, 0.03, 0.0).color(main).glow(2.0))
                    .with(Part::torus(d, 0.08).color(bright).glow(3.5).tag(AnimTag::Pulse))
                    .with_all((0..4).map(|i| {
                        let a = i as f32 * 1.6;
                        Part::sphere(0.22 * h.radius.max(1.0)).at(a.cos() * h.radius * 0.45, 0.1, a.sin() * h.radius * 0.45).color(bright).glow(4.0).tag(AnimTag::Bob)
                    }))
            }
            HazardKind::Tornado => Part::pivot().tag(AnimTag::Spin(9.0)).with_all((0..5).map(|i| {
                let k = i as f32;
                Part::frustum(h.radius * (0.5 + 0.3 * k), 0.7)
                    .at(0.15 * k.sin(), 0.35 + 0.65 * k, 0.15 * k.cos())
                    .color(Color::srgb(0.82, 0.86, 0.9))
                    .glow(if i % 2 == 0 { 0.6 } else { 0.0 })
            })),
        };
        attach(&mut commands, e, part, 1.0);
    }
    for (e, c) in &chests {
        attach(&mut commands, e, models::chest(c.kind), 1.0);
    }
    for (e, p) in &projectiles {
        attach(&mut commands, e, models::projectile(p.kind, p.size), 1.0);
    }
    // A mound of dirt shows where burrowing monsters travel underground.
    for (e, radius, height) in burrowers {
        let mound = spawn_child_model(&mut commands, &mut cache, &mut meshes, &mut materials, e, &models_v4::mound(radius));
        commands.entity(mound).insert(BurrowMound);
        commands.entity(e).try_insert(AnimState { sink: height * 1.15, ..AnimState::with_stride((height * 0.6).max(0.8)) });
    }
    // The glowing column that fills up as a shrine charges.
    for (e, _) in &shrines {
        let fill = Part::cylinder(0.5, 1.0).at(0.0, 0.5, 0.0).color(palette::GLOW_CYAN).glow(6.0);
        let holder = commands
            .spawn((ShrineFill, Transform::from_xyz(0.0, 0.3, 0.0).with_scale(Vec3::new(1.0, 0.01, 1.0)), Visibility::default(), ChildOf(e)))
            .id();
        spawn_model(&mut commands, &mut cache, &mut meshes, &mut materials, e, holder, &fill, ());
    }
}

/// Back in the lobby, players lose their run components: remove their model.
fn detach_player_models(
    mut commands: Commands,
    q: Query<(Entity, &HasModel), (With<Player>, Without<PlayerState>)>,
) {
    for (entity, has) in &q {
        commands.entity(has.model).try_despawn();
        commands.entity(entity).remove::<(HasModel, AnimState)>();
    }
}

/// Chests never move: give them a transform once.
fn place_chests(
    mut commands: Commands,
    chests: Query<(Entity, &Chest), Without<Transform>>,
    statues: Query<(Entity, &Statue), Without<Transform>>,
    hazards: Query<(Entity, &Hazard), Without<Transform>>,
    gems: Query<(Entity, &XpGem), Without<Transform>>,
    shrines: Query<(Entity, &Shrine), Without<Transform>>,
    powerups: Query<(Entity, &Powerup), Without<Transform>>,
    totems: Query<(Entity, &Totem), Without<Transform>>,
) {
    for (e, t) in &totems {
        commands.entity(e).try_insert((Transform::from_translation(t.pos), Visibility::default()));
    }
    for (e, g) in &gems {
        commands.entity(e).try_insert((Transform::from_translation(g.pos), Visibility::default()));
    }
    for (e, s) in &shrines {
        commands.entity(e).try_insert((Transform::from_translation(s.pos), Visibility::default()));
    }
    for (e, p) in &powerups {
        commands.entity(e).try_insert((Transform::from_translation(p.pos), Visibility::default()));
    }
    for (e, s) in &statues {
        commands.entity(e).try_insert((Transform::from_translation(s.pos), Visibility::default()));
    }
    for (e, h) in &hazards {
        commands.entity(e).try_insert((Transform::from_translation(h.pos + Vec3::Y * 0.15), Visibility::default()));
    }
    for (e, c) in &chests {
        commands
            .entity(e)
            .try_insert((Transform::from_translation(c.pos).with_rotation(Quat::from_rotation_y(c.yaw)), Visibility::default()));
    }
}

#[allow(clippy::type_complexity)]
pub fn update_anim_states(
    time: Res<Time>,
    mut q: Query<(
        &Transform,
        &HasModel,
        &mut AnimState,
        Option<&MobState>,
        Option<&Health>,
        Option<&ChestOpen>,
        Option<&MobParts>,
        Has<Player>,
    )>,
    mut models: Query<&mut Transform, Without<HasModel>>,
) {
    let dt = time.delta_secs().max(1e-4);
    for (transform, has, mut st, mob, health, chest, parts, is_player) in &mut q {
        st.broken = parts.map(|p| p.broken).unwrap_or(0);
        if let Some(m) = mob
            && st.sink > 0.0
        {
            // Burrowing monsters dive under the ground (and come back up).
            let target = if m.anim == MobAnim::Burrowed { 1.0 } else { 0.0 };
            st.burrow += (target - st.burrow) * (1.0 - (-dt * 8.0).exp());
            if let Ok(mut model) = models.get_mut(has.model) {
                model.translation.y = -st.sink * st.burrow;
            }
        }
        let pos = transform.translation;
        let flat_speed = st
            .last_pos
            .map(|last| Vec2::new(pos.x - last.x, pos.z - last.z).length() / dt)
            .unwrap_or(0.0);
        st.last_pos = Some(pos);
        // Smooth the measured speed (network positions are a bit noisy).
        st.speed += (flat_speed.min(30.0) - st.speed) * (1.0 - (-dt * 10.0).exp());
        let stride = st.stride.max(0.3);
        st.phase += dt * st.speed / stride * 1.6;

        if let Some(m) = mob
            && m.anim != st.action
        {
            st.action = m.anim;
            st.action_time = 0.0;
        }
        st.action_time += dt;

        if let Some(open) = chest {
            let target = if open.opened { 1.0 } else { 0.0 };
            st.open += (target - st.open) * (1.0 - (-dt * 6.0).exp());
        }

        if is_player {
            st.downed = health.is_some_and(|h| h.downed);
            let target = if st.downed { 1.0 } else { 0.0 };
            st.downed_amount += (target - st.downed_amount) * (1.0 - (-dt * 8.0).exp());
            // Downed players lie on the ground.
            if let Ok(mut model) = models.get_mut(has.model) {
                model.rotation = Quat::from_rotation_x(-1.35 * st.downed_amount);
                model.translation = Vec3::new(0.0, 0.25 * st.downed_amount, 0.0);
            }
        }
    }
}
