//! The first-person weapon: model swap on weapon change, recoil, sway,
//! bob, reload dip, switch animation, muzzle flash.

use bevy::camera::visibility::RenderLayers;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::avatars::AnimState;
use super::camera::CameraFx;
use super::model::{spawn_model, PrimCache};
use super::models;
use super::ViewModelCamera;
use crate::net::client::{InputState, PredictedEvents, Prediction};
use crate::net::protocol::player_color;
use crate::net::LocalPlayer;
use crate::shared::weapons::WeaponKind;

/// Root of the weapon model (child of the view model camera).
#[derive(Component)]
pub struct WeaponRoot {
    kind: WeaponKind,
}

/// Muzzle flash entity (shown briefly after a shot).
#[derive(Component)]
struct MuzzleFlash {
    timer: f32,
}

#[derive(Resource, Default)]
struct ViewModelState {
    /// Smoothed look delta for sway.
    last_yaw: f32,
    last_pitch: f32,
    sway: Vec2,
    kick: f32,
    switch_anim: f32,
    bob: f32,
}

/// Where the weapon sits on screen (right, down, forward).
const HIP: Vec3 = Vec3::new(0.2, -0.19, -0.5);
/// The weapon models are built at real scale; shrink them a bit on screen.
const VIEWMODEL_SCALE: f32 = 0.7;

pub struct ViewModelPlugin;

impl Plugin for ViewModelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewModelState>()
            .add_systems(Update, (swap_weapon_model, react_to_shots, animate_weapon).chain());
    }
}

#[allow(clippy::too_many_arguments)]
fn swap_weapon_model(
    mut commands: Commands,
    pred: Res<Prediction>,
    local: Res<LocalPlayer>,
    camera: Query<Entity, With<ViewModelCamera>>,
    current: Query<(Entity, &WeaponRoot)>,
    mut cache: ResMut<PrimCache>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut vm: ResMut<ViewModelState>,
) {
    let wanted = pred.active.then(|| pred.sim.arsenal.current_kind());
    let existing = current.single().ok();
    if existing.map(|(_, w)| w.kind) == wanted {
        return;
    }
    if let Some((e, _)) = existing {
        commands.entity(e).try_despawn();
    }
    let (Some(kind), Ok(cam)) = (wanted, camera.single()) else { return };
    let accent = player_color(local.slot.unwrap_or(0));
    let root = commands
        .spawn((
            WeaponRoot { kind },
            AnimState::default(),
            Transform::from_translation(HIP).with_scale(Vec3::splat(VIEWMODEL_SCALE)),
            Visibility::default(),
            RenderLayers::layer(super::VIEWMODEL_LAYER),
            ChildOf(cam),
        ))
        .id();
    let model = models::weapon(kind, accent);
    spawn_model(
        &mut commands,
        &mut cache,
        &mut meshes,
        &mut materials,
        root,
        root,
        &model,
        (RenderLayers::layer(super::VIEWMODEL_LAYER), NotShadowCaster),
    );
    // Muzzle flash: a glowing star, hidden until we shoot.
    let flash_mat = cache.material(&mut materials, Color::srgb(1.0, 0.8, 0.4), 8.0);
    let flash_mesh = cache.mesh(&mut meshes, super::model::Prim::Ico, Vec3::new(0.12, 0.12, 0.2));
    commands.spawn((
        MuzzleFlash { timer: 0.0 },
        Mesh3d(flash_mesh),
        MeshMaterial3d(flash_mat),
        Transform::from_translation(models::muzzle(kind)),
        Visibility::Hidden,
        RenderLayers::layer(super::VIEWMODEL_LAYER),
        NotShadowCaster,
        ChildOf(root),
    ));
    vm.switch_anim = 1.0;
}

fn react_to_shots(
    mut events: MessageReader<PredictedEvents>,
    mut vm: ResMut<ViewModelState>,
    mut cam_fx: ResMut<CameraFx>,
    mut flashes: Query<(&mut MuzzleFlash, &mut Visibility, &mut Transform)>,
    mut weapon: Query<&mut AnimState, With<WeaponRoot>>,
) {
    for PredictedEvents(ev) in events.read() {
        if let Some(kind) = ev.weapons.fired {
            let recoil = kind.def().recoil;
            vm.kick = (vm.kick + 0.35 * recoil).min(1.5);
            cam_fx.recoil += 0.012 * recoil;
            cam_fx.add_trauma(0.04 * recoil);
            for mut st in &mut weapon {
                st.recoil = 1.0;
            }
            for (mut flash, mut vis, mut t) in &mut flashes {
                flash.timer = 0.05;
                *vis = Visibility::Inherited;
                t.rotation = Quat::from_rotation_z(vm.kick * 7.0);
            }
        }
        if ev.weapons.switched {
            vm.switch_anim = 1.0;
        }
        if ev.mv.landed {
            cam_fx.land_dip = 0.12;
        }
        if ev.mv.dashed {
            cam_fx.fov_kick = 12.0;
        }
        if ev.mv.pad_launched {
            cam_fx.fov_kick = 15.0;
            cam_fx.add_trauma(0.2);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn animate_weapon(
    time: Res<Time>,
    input: Res<InputState>,
    pred: Res<Prediction>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut vm: ResMut<ViewModelState>,
    mut weapon: Query<(&mut Transform, &mut AnimState), With<WeaponRoot>>,
    mut flashes: Query<(&mut MuzzleFlash, &mut Visibility)>,
) {
    let dt = time.delta_secs();
    let Ok((mut transform, mut st)) = weapon.single_mut() else { return };

    // Sway: the weapon lags behind fast mouse movements.
    let dyaw = input.yaw - vm.last_yaw;
    let dpitch = input.pitch - vm.last_pitch;
    vm.last_yaw = input.yaw;
    vm.last_pitch = input.pitch;
    let dyaw = (dyaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    let target_sway = Vec2::new(dyaw, dpitch).clamp_length_max(0.15) * 1.5;
    let k = 1.0 - (-dt * 10.0).exp();
    vm.sway = vm.sway + (target_sway - vm.sway) * k;

    vm.kick *= (-dt * 12.0).exp();
    vm.switch_anim = (vm.switch_anim - dt * 4.0).max(0.0);
    st.recoil *= (-dt * 18.0).exp();

    let mv = &pred.sim.mv;
    let speed = Vec2::new(mv.vel.x, mv.vel.z).length();
    if mv.grounded && speed > 1.0 {
        vm.bob += dt * speed * 1.25;
    }
    let bob_amt = if mv.grounded { (speed / 11.0).min(1.0) } else { 0.0 };
    let bob = Vec3::new((vm.bob).sin() * 0.012, (vm.bob * 2.0).sin().abs() * 0.012, 0.0) * bob_amt;

    let reload = pred.sim.arsenal.reload_left;
    let reload_dip = if reload > 0.0 { 1.0 } else { 0.0 };
    let aiming = mouse.pressed(MouseButton::Right);
    let base = if aiming { Vec3::new(0.0, -0.11, -0.42) } else { HIP };
    let sliding = if mv.is_sliding() { 1.0 } else { 0.0 };

    let target = base
        + bob
        + Vec3::new(-vm.sway.x * 0.1, vm.sway.y * 0.1, vm.kick * 0.07)
        + Vec3::new(0.0, -0.25 * vm.switch_anim - 0.12 * reload_dip, 0.0)
        + Vec3::new(-0.04, -0.03, 0.0) * sliding;
    transform.translation = transform.translation.lerp(target, 1.0 - (-dt * 20.0).exp());
    transform.rotation = Quat::from_euler(
        EulerRot::YXZ,
        vm.sway.x * 0.6,
        vm.kick * 0.25 + vm.sway.y * 0.4 - reload_dip * 0.5 - vm.switch_anim * 0.8,
        vm.sway.x * 0.5 + sliding * 0.25 + reload_dip * 0.3,
    );

    for (mut flash, mut vis) in &mut flashes {
        flash.timer -= dt;
        if flash.timer <= 0.0 {
            *vis = Visibility::Hidden;
        }
    }
}
