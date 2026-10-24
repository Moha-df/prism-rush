//! Gives a visual model to every networked entity (other players, monsters,
//! chests, projectiles) and keeps their animation state up to date.

use bevy::prelude::*;

use super::model::{spawn_model, AnimTag, Part, PrimCache};
use super::palette;
use super::models;
use crate::net::interp::LocalAvatar;
use crate::net::protocol::*;

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
            (attach_models, detach_player_models, place_chests, update_anim_states, grow_shockwaves, dim_used_statues, hit_pop).chain(),
        );
    }
}

/// Shockwave rings grow with time (in the server's present, so what you see
/// is exactly when you must jump).
fn grow_shockwaves(
    clock: Res<crate::net::client::ServerClock>,
    hazards: Query<(&Hazard, &HasModel)>,
    mut models: Query<&mut Transform, Without<HasModel>>,
) {
    for (h, has) in &hazards {
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
    pickups: Query<(Entity, &WeaponPickup), NeedsModel>,
    statues: Query<(Entity, &Statue), NeedsModel>,
    hazards: Query<(Entity, &Hazard), NeedsModel>,
    gems: Query<(Entity, &XpGem), NeedsModel>,
) {
    let mut attach = |commands: &mut Commands, entity: Entity, part: super::model::Part, stride: f32| {
        let model = spawn_child_model(commands, &mut cache, &mut meshes, &mut materials, entity, &part);
        commands.entity(entity).try_insert((HasModel { model }, AnimState::with_stride(stride)));
    };
    for (e, p) in &players {
        attach(&mut commands, e, models::player(player_color(p.slot)), 1.4);
    }
    for (e, m) in &mobs {
        let def = m.kind.def();
        let model = if m.elite {
            // Elites: bigger, with a golden aura ring and a little crown.
            models::scale_part(models::mob(m.kind), 1.3)
                .with(Part::torus(def.radius * 3.0, 0.1).at(0.0, 0.15, 0.0).color(palette::GOLD).glow(4.0).tag(AnimTag::Pulse))
                .with(Part::cone(0.3, 0.35).at(0.0, def.height * 1.3 + 0.3, 0.0).color(palette::GOLD).glow(2.0).tag(AnimTag::Bob))
        } else {
            models::mob(m.kind)
        };
        attach(&mut commands, e, model, (def.height * 0.6).max(0.8));
    }
    for (e, s) in &statues {
        attach(&mut commands, e, models::statue(s.boss), 1.0);
    }
    for (e, g) in &gems {
        attach(&mut commands, e, models::xp_gem(g.value), 1.0);
    }
    for (e, h) in &hazards {
        let part = match h.kind {
            // Unit ring, scaled every frame to the shockwave radius.
            HazardKind::Shockwave => Part::torus(2.0, 0.6).color(palette::GLOW_CYAN).glow(5.0),
            HazardKind::Lightning => Part::pivot()
                .with(Part::cylinder(LIGHTNING_RADIUS * 2.0, 0.06).color(palette::GLOW_MAGENTA).glow(2.5).tag(AnimTag::Pulse))
                .with(Part::torus(LIGHTNING_RADIUS * 2.0, 0.15).color(palette::GLOW_MAGENTA).glow(5.0)),
        };
        attach(&mut commands, e, part, 1.0);
    }
    for (e, c) in &chests {
        attach(&mut commands, e, models::chest(c.kind), 1.0);
    }
    for (e, p) in &projectiles {
        attach(&mut commands, e, models::projectile(p.kind), 1.0);
    }
    for (e, p) in &pickups {
        attach(&mut commands, e, models::weapon_pickup(p.kind), 1.0);
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
    pickups: Query<(Entity, &WeaponPickup), Without<Transform>>,
    statues: Query<(Entity, &Statue), Without<Transform>>,
    hazards: Query<(Entity, &Hazard), Without<Transform>>,
    gems: Query<(Entity, &XpGem), Without<Transform>>,
) {
    for (e, g) in &gems {
        commands.entity(e).try_insert((Transform::from_translation(g.pos), Visibility::default()));
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
    // Weapons on the ground don't move either.
    for (e, p) in &pickups {
        commands.entity(e).try_insert((Transform::from_translation(p.pos), Visibility::default()));
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
        Has<Player>,
    )>,
    mut models: Query<&mut Transform, Without<HasModel>>,
) {
    let dt = time.delta_secs().max(1e-4);
    for (transform, has, mut st, mob, health, chest, is_player) in &mut q {
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
