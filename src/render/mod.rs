//! Everything visual and audible. Only added when the game has a window.

pub mod anim;
pub mod atmosphere;
pub mod audio;
pub mod autos;
pub mod avatars;
pub mod camera;
pub mod debug;
pub mod fx;
pub mod icons;
pub mod model;
pub mod models;
pub mod palette;
pub mod terrain;
pub mod viewmodel;

use bevy::camera::visibility::RenderLayers;
use bevy::light::{CascadeShadowConfigBuilder, NotShadowCaster};
use bevy::prelude::*;

use model::{MeshBuffer, PrimCache};

/// The world camera.
#[derive(Component)]
pub struct MainCamera;

/// Second camera drawing only the first-person weapon (layer 1), on top of
/// the world, so the weapon never clips into walls.
#[derive(Component)]
pub struct ViewModelCamera;

pub const VIEWMODEL_LAYER: usize = 1;

/// The sky dome follows the camera.
#[derive(Component)]
pub struct SkyDome;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PrimCache>()
            .insert_resource(ClearColor(palette::FOG))
            .insert_resource(GlobalAmbientLight { color: palette::AMBIENT, brightness: 1100.0, ..default() })
            .add_systems(Startup, (setup_lights, setup_sky))
            .add_systems(PostUpdate, follow_sky.after(camera::CameraSystems))
            .add_plugins((
                camera::CameraPlugin,
                terrain::TerrainRenderPlugin,
                avatars::AvatarsPlugin,
                anim::AnimPlugin,
                viewmodel::ViewModelPlugin,
                fx::FxPlugin,
                audio::SfxPlugin,
                debug::DebugPlugin,
                icons::IconsPlugin,
                atmosphere::AtmospherePlugin,
                autos::AutoVisualsPlugin,
            ));
    }
}

fn setup_lights(mut commands: Commands) {
    // Warm low sun: long shadows and a golden-hour mood.
    commands.spawn((
        DirectionalLight {
            color: palette::SUN,
            illuminance: 9_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0).looking_to(Vec3::new(-0.55, -0.45, -0.7), Vec3::Y),
        CascadeShadowConfigBuilder { first_cascade_far_bound: 15.0, maximum_distance: 120.0, ..default() }.build(),
        // Light both the world and the first-person weapon.
        RenderLayers::from_layers(&[0, VIEWMODEL_LAYER]),
    ));
}

/// A big inverted sphere with a vertical color gradient.
fn setup_sky(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut cache: ResMut<PrimCache>) {
    let tris = cache.tris(model::Prim::Sphere).clone();
    let mut buf = MeshBuffer::default();
    let radius = 900.0;
    for t in tris {
        // Reverse winding: we look at the sphere from the inside.
        let tri = [t[0] * radius * 2.0, t[2] * radius * 2.0, t[1] * radius * 2.0];
        for v in [tri[0], tri[1], tri[2]] {
            let h = (v.y / radius).clamp(-1.0, 1.0);
            let c = if h > 0.0 {
                palette::mix(palette::SKY_HORIZON, palette::SKY_ZENITH, h.powf(0.6))
            } else {
                let f = palette::FOG.to_srgba();
                palette::mix(palette::SKY_HORIZON, [f.red, f.green, f.blue], (-h * 3.0).min(1.0))
            };
            let lin = Color::srgb(c[0], c[1], c[2]).to_linear();
            buf.colors.push([lin.red, lin.green, lin.blue, 1.0]);
        }
        buf.tris.push(tri);
    }
    commands.spawn((
        Mesh3d(meshes.add(buf.build())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            fog_enabled: false,
            ..default()
        })),
        Transform::default(),
        NotShadowCaster,
        SkyDome,
    ));
    // The sun disc, far away in the light direction.
    let sun_dir = -Vec3::new(-0.55, -0.45, -0.7).normalize();
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(40.0).mesh().ico(2).expect("ico"))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: LinearRgba::rgb(12.0, 9.0, 6.0),
            unlit: true,
            fog_enabled: false,
            ..default()
        })),
        Transform::from_translation(sun_dir * 800.0),
        NotShadowCaster,
        atmosphere::SunDisc,
    ));
}

fn follow_sky(camera: Query<&GlobalTransform, With<MainCamera>>, mut sky: Query<&mut Transform, With<SkyDome>>, mut offsets: Local<Vec<Vec3>>) {
    let Ok(cam) = camera.single() else { return };
    if offsets.is_empty() {
        offsets.extend(sky.iter().map(|t| t.translation));
    }
    for (i, mut t) in sky.iter_mut().enumerate() {
        t.translation = cam.translation() + offsets.get(i).copied().unwrap_or_default();
    }
}
