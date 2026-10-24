//! First-person camera and player input (keyboard + mouse).
//!
//! Mouse look is applied *every frame* (not every tick) for perfectly smooth
//! aiming; the position comes from the predicted simulation, interpolated
//! between the last two ticks.

use std::f32::consts::FRAC_PI_2;

use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::anti_alias::smaa::Smaa;
use bevy::pbr::{ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::camera::Hdr;
use bevy::window::{CursorGrabMode, CursorOptions};

use super::{palette, MainCamera, ViewModelCamera, VIEWMODEL_LAYER};
use crate::config::AppConfig;
use crate::game::world::TerrainRes;
use crate::net::client::{InputState, Prediction};
use crate::net::protocol::{Health, Player};
use crate::net::LocalPlayer;
use crate::shared::movement::buttons;
use crate::ui::{AppState, Paused};

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CameraSystems;

/// Camera "juice" state: shake, recoil, FOV kicks. Written by the FX code.
#[derive(Resource, Default, Debug)]
pub struct CameraFx {
    /// 0..1, shake amplitude grows with trauma².
    pub trauma: f32,
    /// Extra pitch from weapon recoil (radians), springs back to 0.
    pub recoil: f32,
    pub fov_kick: f32,
    /// Downward offset after landing.
    pub land_dip: f32,
    pub time: f32,
    pub bob_phase: f32,
    pub eye_height: f32,
}

impl CameraFx {
    pub fn add_trauma(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraFx>()
            .add_systems(Startup, spawn_cameras)
            .add_systems(Update, (grab_cursor, read_input).chain())
            .add_systems(
                PostUpdate,
                update_camera.in_set(CameraSystems).before(TransformSystems::Propagate),
            );
    }
}

fn spawn_cameras(mut commands: Commands, config: Res<AppConfig>) {
    commands
        .spawn((
            MainCamera,
            Camera3d::default(),
            Camera { order: 0, ..default() },
            Hdr,
            Tonemapping::TonyMcMapface,
            Bloom { intensity: 0.12, ..Bloom::NATURAL },
            // Ambient occlusion darkens creases and contact points: adds a lot
            // of depth to flat-shaded low poly. It requires MSAA off, so SMAA
            // (a post-process) smooths the edges instead.
            Msaa::Off,
            ScreenSpaceAmbientOcclusion { quality_level: ScreenSpaceAmbientOcclusionQualityLevel::Medium, ..default() },
            Smaa::default(),
            Projection::from(PerspectiveProjection { fov: config.fov.to_radians(), near: 0.05, ..default() }),
            DistanceFog {
                color: palette::FOG,
                directional_light_color: Color::srgba(1.0, 0.85, 0.65, 0.35),
                directional_light_exponent: 20.0,
                falloff: FogFalloff::Linear { start: 60.0, end: 135.0 },
            },
            Transform::from_xyz(0.0, 12.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
        ))
        .with_child((
            ViewModelCamera,
            Camera3d::default(),
            Camera { order: 1, clear_color: ClearColorConfig::None, ..default() },
            Msaa::Off,
            Hdr,
            Tonemapping::TonyMcMapface,
            Projection::from(PerspectiveProjection { fov: 62f32.to_radians(), near: 0.02, ..default() }),
            RenderLayers::layer(VIEWMODEL_LAYER),
            IsDefaultUiCamera,
        ));
}

fn grab_cursor(
    state: Res<State<AppState>>,
    paused: Res<Paused>,
    choosing: Res<crate::ui::cards::ChoosingCard>,
    mut cursor: Query<&mut CursorOptions>,
) {
    let Ok(mut cursor) = cursor.single_mut() else { return };
    let want_grab = *state.get() == AppState::InGame && !paused.0 && !choosing.0;
    let grabbed = cursor.grab_mode != CursorGrabMode::None;
    if want_grab != grabbed {
        cursor.grab_mode = if want_grab { CursorGrabMode::Locked } else { CursorGrabMode::None };
        cursor.visible = !want_grab;
    }
}

#[allow(clippy::too_many_arguments)]
fn read_input(
    state: Res<State<AppState>>,
    paused: Res<Paused>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    config: Res<AppConfig>,
    choosing: Res<crate::ui::cards::ChoosingCard>,
    mut input: ResMut<InputState>,
) {
    if config.autopilot {
        return; // the bot writes the input
    }
    if *state.get() != AppState::InGame || paused.0 {
        input.held = 0;
        input.move_axis = Vec2::ZERO;
        return;
    }
    // Mouse look (not while picking a card: the mouse is used to click).
    let sens = if choosing.0 { 0.0 } else { 0.0022 * config.sensitivity };
    input.yaw -= motion.delta.x * sens;
    input.pitch = (input.pitch - motion.delta.y * sens).clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
    input.yaw = input.yaw.rem_euclid(std::f32::consts::TAU);

    // Movement: physical keys, so WASD on QWERTY = ZQSD on AZERTY automatically.
    let mut axis = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        axis.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        axis.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        axis.x += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        axis.x -= 1.0;
    }
    input.move_axis = axis.normalize_or_zero();

    let bindings: [(bool, bool, u16); 7] = [
        (keys.pressed(KeyCode::Space), keys.just_pressed(KeyCode::Space), buttons::JUMP),
        (keys.pressed(KeyCode::ShiftLeft), keys.just_pressed(KeyCode::ShiftLeft), buttons::SPRINT),
        (
            keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::KeyC),
            keys.just_pressed(KeyCode::ControlLeft) || keys.just_pressed(KeyCode::KeyC),
            buttons::SLIDE,
        ),
        (
            keys.pressed(KeyCode::KeyQ) || mouse.pressed(MouseButton::Back),
            keys.just_pressed(KeyCode::KeyQ) || mouse.just_pressed(MouseButton::Back),
            buttons::DASH,
        ),
        (
            mouse.pressed(MouseButton::Left) && !choosing.0,
            mouse.just_pressed(MouseButton::Left) && !choosing.0,
            buttons::FIRE,
        ),
        (keys.pressed(KeyCode::KeyR), keys.just_pressed(KeyCode::KeyR), buttons::RELOAD),
        (keys.pressed(KeyCode::KeyE), keys.just_pressed(KeyCode::KeyE), buttons::INTERACT),
    ];
    input.held = 0;
    for (held, just, bit) in bindings {
        if held {
            input.held |= bit;
        }
        if just {
            input.press(bit);
        }
    }
    if scroll.delta.y > 0.0 {
        input.press(buttons::PREV_WEAPON);
    } else if scroll.delta.y < 0.0 {
        input.press(buttons::NEXT_WEAPON);
    }
    let digits = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6];
    for (i, key) in digits.iter().enumerate() {
        // While choosing a card, digits pick the card instead.
        if keys.just_pressed(*key) && !choosing.0 {
            input.select_slot = i as u8;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn update_camera(
    mut camera: Query<(&mut Transform, &mut Projection), With<MainCamera>>,
    state: Res<State<AppState>>,
    pred: Res<Prediction>,
    input: Res<InputState>,
    local: Res<LocalPlayer>,
    players: Query<(&Player, &Health)>,
    fixed: Res<Time<Fixed>>,
    time: Res<Time>,
    config: Res<AppConfig>,
    mouse: Res<ButtonInput<MouseButton>>,
    terrain: Res<TerrainRes>,
    mut fx: ResMut<CameraFx>,
) {
    let Ok((mut transform, mut projection)) = camera.single_mut() else { return };
    let dt = time.delta_secs();
    fx.time += dt;

    let in_game = matches!(state.get(), AppState::InGame | AppState::GameOver) && pred.active;
    if !in_game {
        // Menu / lobby: slow cinematic orbit over the world.
        let t = fx.time * 0.05;
        let pos = Vec3::new(t.cos() * 60.0, 0.0, t.sin() * 60.0);
        let ground = terrain.wgen.ground_height(pos.x, pos.z);
        transform.translation = Vec3::new(pos.x, ground.max(0.0) + 18.0, pos.z);
        transform.look_at(Vec3::new(0.0, terrain.wgen.ground_height(0.0, 0.0) + 2.0, 0.0), Vec3::Y);
        return;
    }

    let downed = players.iter().any(|(p, h)| Some(p.slot) == local.slot && h.downed);
    let mv = &pred.sim.mv;
    let alpha = fixed.overstep_fraction();
    let feet = pred.prev_pos.lerp(mv.pos, alpha) + pred.smoothing;

    // Smooth eye height (sliding / downed crouch the camera).
    let target_eye = if downed { 0.55 } else { mv.eye_height() };
    if fx.eye_height == 0.0 {
        fx.eye_height = target_eye;
    }
    fx.eye_height += (target_eye - fx.eye_height) * (1.0 - (-dt * 14.0).exp());

    // Head bob while walking on the ground.
    let speed = Vec2::new(mv.vel.x, mv.vel.z).length();
    let bobbing = mv.grounded && !mv.is_sliding() && speed > 1.0;
    if bobbing {
        fx.bob_phase += dt * speed * 1.25;
    }
    let bob_amount = if bobbing { (speed / 11.0).min(1.0) * 0.045 } else { 0.0 };
    let bob = Vec3::new(0.0, (fx.bob_phase * 2.0).sin().abs() * bob_amount, 0.0);

    // Shake: random-ish offsets from sines at unrelated frequencies.
    fx.trauma = (fx.trauma - dt * 1.6).max(0.0);
    let shake = fx.trauma * fx.trauma;
    let t = fx.time * 30.0;
    let shake_rot = Vec3::new((t * 1.3).sin(), (t * 1.7 + 1.0).sin(), (t * 2.1 + 2.0).sin()) * shake * 0.05;

    fx.recoil *= (-dt * 10.0).exp();
    fx.land_dip *= (-dt * 9.0).exp();
    fx.fov_kick *= (-dt * 6.0).exp();

    transform.translation = feet + Vec3::Y * (fx.eye_height - fx.land_dip) + bob;
    let roll = if downed { 0.25 } else { 0.0 };
    transform.rotation = Quat::from_euler(
        EulerRot::YXZ,
        input.yaw + shake_rot.y,
        (input.pitch + fx.recoil + shake_rot.x).clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01),
        roll + shake_rot.z,
    );

    // FOV: wider when sprinting/dashing (sense of speed), narrower when zooming.
    let sprint = if speed > 9.0 { 6.0 } else { 0.0 };
    let zoom = mouse.pressed(MouseButton::Right) && !downed;
    let target_fov = if zoom {
        let sniper = pred.sim.arsenal.current_kind() == crate::shared::weapons::WeaponKind::Sniper;
        if sniper { 25.0 } else { config.fov * 0.65 }
    } else {
        config.fov + sprint + fx.fov_kick
    };
    if let Projection::Perspective(p) = projection.as_mut() {
        let current = p.fov.to_degrees();
        let new = current + (target_fov - current) * (1.0 - (-dt * 12.0).exp());
        p.fov = new.to_radians();
    }
}
