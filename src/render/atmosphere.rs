//! Mood lighting: the sky, sun, fog and ambient light follow the run.
//!
//! Every 4 waves the time of day moves on: golden afternoon -> sunset ->
//! violet dusk -> night (glowing monsters look great in the dark) -> back
//! to afternoon. Values are blended smoothly so a change is never abrupt.
//! Also: a layer of low-poly clouds drifting around the camera.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::model::{MeshBuffer, Part, PrimCache};
use super::palette;
use super::{MainCamera, SkyDome};
use crate::net::protocol::MatchInfo;
use crate::shared::rng::Rng;

/// One lighting mood.
#[derive(Clone, Copy, Debug)]
struct Mood {
    sun: [f32; 3],
    sun_lux: f32,
    /// Direction the light travels (pointing down).
    sun_dir: Vec3,
    fog: [f32; 3],
    ambient: [f32; 3],
    ambient_brightness: f32,
    /// Multiplier on the sky dome colors.
    sky: [f32; 3],
    clouds: [f32; 3],
}

const MOODS: [Mood; 4] = [
    // Golden afternoon.
    Mood {
        sun: [1.0, 0.86, 0.68],
        sun_lux: 9_000.0,
        sun_dir: Vec3::new(-0.55, -0.45, -0.7),
        fog: [0.86, 0.72, 0.70],
        ambient: [0.55, 0.60, 0.90],
        ambient_brightness: 1_000.0,
        sky: [1.0, 1.0, 1.0],
        clouds: [1.0, 0.93, 0.9],
    },
    // Sunset.
    Mood {
        sun: [1.0, 0.55, 0.3],
        sun_lux: 6_500.0,
        sun_dir: Vec3::new(-0.75, -0.2, -0.65),
        fog: [0.95, 0.58, 0.48],
        ambient: [0.62, 0.48, 0.80],
        ambient_brightness: 900.0,
        sky: [1.15, 0.78, 0.7],
        clouds: [1.0, 0.65, 0.55],
    },
    // Violet dusk.
    Mood {
        sun: [0.85, 0.5, 0.95],
        sun_lux: 3_500.0,
        sun_dir: Vec3::new(-0.8, -0.12, -0.6),
        fog: [0.42, 0.32, 0.58],
        ambient: [0.50, 0.42, 0.92],
        ambient_brightness: 850.0,
        sky: [0.6, 0.48, 0.85],
        clouds: [0.6, 0.45, 0.75],
    },
    // Night (moonlight).
    Mood {
        sun: [0.6, 0.75, 1.0],
        sun_lux: 2_400.0,
        sun_dir: Vec3::new(-0.3, -0.8, -0.5),
        fog: [0.11, 0.13, 0.27],
        ambient: [0.42, 0.52, 0.98],
        ambient_brightness: 750.0,
        sky: [0.16, 0.19, 0.4],
        clouds: [0.25, 0.28, 0.45],
    },
];

const WAVES_PER_MOOD: f32 = 4.0;

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    palette::mix(a, b, t)
}

fn blend(a: &Mood, b: &Mood, t: f32) -> Mood {
    Mood {
        sun: mix3(a.sun, b.sun, t),
        sun_lux: a.sun_lux + (b.sun_lux - a.sun_lux) * t,
        sun_dir: a.sun_dir.lerp(b.sun_dir, t).normalize(),
        fog: mix3(a.fog, b.fog, t),
        ambient: mix3(a.ambient, b.ambient, t),
        ambient_brightness: a.ambient_brightness + (b.ambient_brightness - a.ambient_brightness) * t,
        sky: mix3(a.sky, b.sky, t),
        clouds: mix3(a.clouds, b.clouds, t),
    }
}

/// Mood for a (fractional) wave number.
fn mood_at(wave: f32) -> Mood {
    let x = ((wave - 1.0).max(0.0) / WAVES_PER_MOOD) % MOODS.len() as f32;
    let i = x.floor() as usize;
    let t = x.fract();
    // Hold each mood for half of its waves, then blend slowly into the next.
    let t = ((t - 0.5) / 0.5).clamp(0.0, 1.0);
    let t = t * t * (3.0 - 2.0 * t);
    blend(&MOODS[i], &MOODS[(i + 1) % MOODS.len()], t)
}

#[derive(Resource)]
struct AtmosphereState {
    /// Smoothed wave value driving the mood.
    wave: f32,
    sky_material: Option<Handle<StandardMaterial>>,
    cloud_material: Handle<StandardMaterial>,
}

/// The sun disc in the sky (moves with the light direction).
#[derive(Component)]
pub struct SunDisc;

#[derive(Component)]
struct CloudLayer;

pub struct AtmospherePlugin;

impl Plugin for AtmospherePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_clouds)
            .add_systems(Update, (update_atmosphere, follow_clouds));
    }
}

fn spawn_clouds(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cache: ResMut<PrimCache>,
) {
    let cloud_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        fog_enabled: false,
        ..default()
    });
    // All clouds baked into one mesh: a ring of puffy clusters.
    let mut rng = Rng::new(77);
    let mut buf = MeshBuffer::default();
    let mut unused_glow = MeshBuffer::default();
    for _ in 0..28 {
        let angle = rng.range(0.0, std::f32::consts::TAU);
        let dist = rng.range(260.0, 480.0);
        let center = Vec3::new(angle.cos() * dist, rng.range(85.0, 150.0), angle.sin() * dist);
        let size = rng.range(14.0, 30.0);
        let mut cloud = Part::pivot().at_v(center).rot_y(rng.range(0.0, 360.0));
        for _ in 0..rng.range_u32(3, 6) {
            let off = Vec3::new(rng.range(-1.0, 1.0) * size, rng.range(-0.15, 0.25) * size, rng.range(-0.5, 0.5) * size);
            let s = size * rng.range(0.6, 1.1);
            let shade = rng.range(0.9, 1.0);
            cloud = cloud.with(Part::ico(s * 1.4, s * 0.55, s).at_v(off).color(Color::srgb(shade, shade, shade)));
        }
        buf.add_part(&mut cache, &cloud, &Transform::IDENTITY, &mut unused_glow);
    }
    commands.spawn((
        CloudLayer,
        Mesh3d(meshes.add(buf.build())),
        MeshMaterial3d(cloud_material.clone()),
        Transform::default(),
        NotShadowCaster,
    ));
    commands.insert_resource(AtmosphereState { wave: 1.0, sky_material: None, cloud_material });
}

/// Clouds stay around the camera and slowly turn (wind).
fn follow_clouds(
    time: Res<Time>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut clouds: Query<&mut Transform, With<CloudLayer>>,
) {
    let (Ok(cam), Ok(mut t)) = (camera.single(), clouds.single_mut()) else { return };
    t.translation = Vec3::new(cam.translation().x, 0.0, cam.translation().z);
    t.rotation = Quat::from_rotation_y(time.elapsed_secs() * 0.004);
}

#[allow(clippy::too_many_arguments)]
fn update_atmosphere(
    time: Res<Time>,
    info: Query<&MatchInfo>,
    mut state: ResMut<AtmosphereState>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform), Without<SunDisc>>,
    mut sun_disc: Query<&mut Transform, (With<SunDisc>, Without<DirectionalLight>)>,
    mut fog: Query<&mut DistanceFog>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    sky: Query<&MeshMaterial3d<StandardMaterial>, (With<SkyDome>, Without<SunDisc>)>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
) {
    let target = info.single().map(|m| m.wave.max(1) as f32).unwrap_or(1.0);
    // Glide toward the target wave: the mood changes over a few seconds.
    let k = 1.0 - (-time.delta_secs() * 0.4).exp();
    state.wave += (target - state.wave) * k;
    let mood = mood_at(state.wave);

    let c3 = |c: [f32; 3]| Color::srgb(c[0], c[1], c[2]);
    if let Ok((mut light, mut t)) = sun.single_mut() {
        light.color = c3(mood.sun);
        light.illuminance = mood.sun_lux;
        *t = Transform::default().looking_to(mood.sun_dir, Vec3::Y);
    }
    if let (Ok(mut disc), Ok(cam)) = (sun_disc.single_mut(), camera.single()) {
        disc.translation = cam.translation() - mood.sun_dir * 800.0;
    }
    for mut f in &mut fog {
        f.color = c3(mood.fog);
    }
    ambient.color = c3(mood.ambient);
    ambient.brightness = mood.ambient_brightness;
    clear.0 = c3(mood.fog);

    if state.sky_material.is_none() {
        state.sky_material = sky.iter().next().map(|m| m.0.clone());
    }
    if let Some(h) = &state.sky_material
        && let Some(mut m) = materials.get_mut(h)
    {
        m.base_color = Color::linear_rgb(mood.sky[0], mood.sky[1], mood.sky[2]);
    }
    if let Some(mut m) = materials.get_mut(&state.cloud_material) {
        m.base_color = c3(mood.clouds);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moods_cycle_and_blend_smoothly() {
        let a = mood_at(1.0);
        assert_eq!(a.sun_lux, MOODS[0].sun_lux);
        // Night around wave 13-16.
        assert_eq!(mood_at(13.0).sun_lux, MOODS[3].sun_lux);
        // Back to afternoon after a full cycle.
        assert!((mood_at(17.0).sun_lux - MOODS[0].sun_lux).abs() < 1.0);
        // No jump between two close waves.
        for i in 0..200 {
            let w = 1.0 + i as f32 * 0.1;
            let d = (mood_at(w).sun_lux - mood_at(w + 0.1).sun_lux).abs();
            assert!(d < 900.0, "jump at wave {w}: {d}");
        }
    }
}
