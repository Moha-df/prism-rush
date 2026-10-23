//! Streams chunk meshes around the camera: the "infinite" map.
//!
//! Each chunk becomes 3 meshes: terrain, props (rocks, trees...) and glowing
//! props (crystals, pads). All props of a chunk are *baked* into one mesh
//! with vertex colors, so a chunk costs 3 draw calls whatever its content.

use std::collections::HashMap;

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::model::{MeshBuffer, PrimCache};
use super::models;
use super::palette::{self, mix};
use super::MainCamera;
use crate::game::world::TerrainRes;
use crate::shared::noise::fbm;
use crate::shared::rng::hash_2d;
use crate::shared::worldgen::{chunk_of, WorldGen, CELL, CELLS_PER_CHUNK};

/// Chunks visible around the camera (radius, in chunks). 4 => 9x9 chunks = 288 m.
pub const VIEW_RADIUS: i32 = 4;
/// New chunks built per frame (spreads the CPU cost).
const BUILDS_PER_FRAME: usize = 2;

#[derive(Resource, Default)]
struct LoadedChunks {
    map: HashMap<IVec2, Entity>,
    seed: Option<u64>,
}

#[derive(Resource)]
struct ChunkMaterials {
    lit: Handle<StandardMaterial>,
    glow: Handle<StandardMaterial>,
}

pub struct TerrainRenderPlugin;

impl Plugin for TerrainRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoadedChunks>()
            .add_systems(Startup, setup_materials)
            .add_systems(Update, stream_chunks);
    }
}

fn setup_materials(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(ChunkMaterials {
        // White base color: the vertex colors give the actual colors.
        lit: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.92,
            reflectance: 0.2,
            ..default()
        }),
        // Unlit + HDR vertex colors (> 1.0) => picked up by bloom.
        glow: materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, ..default() }),
    });
}

/// Color of a terrain triangle from its height, slope and position.
fn terrain_color(wgen: &WorldGen, center: Vec3, normal: Vec3) -> [f32; 4] {
    let s = wgen.seed;
    let biome = fbm(s.wrapping_add(71), center.x / 140.0, center.z / 140.0, 2);
    let patch = fbm(s.wrapping_add(83), center.x / 18.0, center.z / 18.0, 2);
    let mut c = mix(palette::GRASS_TEAL, palette::GRASS_A, (biome * 1.6 - 0.3).clamp(0.0, 1.0));
    c = mix(c, palette::GRASS_B, (patch - 0.45).clamp(0.0, 0.5));

    let h = center.y;
    if h < -9.0 {
        c = mix(c, palette::SAND, ((-9.0 - h) / 3.0).clamp(0.0, 1.0));
    }
    // Steep slopes: layered clay cliffs (strata bands by height).
    let steep = ((0.82 - normal.y) / 0.12).clamp(0.0, 1.0);
    if steep > 0.0 {
        let band = palette::CLAY[((h / 1.3).floor() as i32).rem_euclid(3) as usize];
        c = mix(c, band, steep);
    }
    if h > 20.0 && normal.y > 0.75 {
        c = mix(c, palette::SNOW, ((h - 20.0) / 4.0).clamp(0.0, 1.0));
    }
    // Per-triangle brightness jitter: makes the facets readable.
    let jitter = (hash_2d(s, (center.x * 7.0) as i32, (center.z * 7.0) as i32) >> 40) as f32 / (1u64 << 24) as f32;
    let k = 0.94 + jitter * 0.1;
    // Convert sRGB-ish palette values to linear for vertex colors.
    let lin = Color::srgb(c[0] * k, c[1] * k, c[2] * k).to_linear();
    [lin.red, lin.green, lin.blue, 1.0]
}

fn build_terrain_mesh(wgen: &WorldGen, coord: IVec2) -> MeshBuffer {
    let n = CELLS_PER_CHUNK;
    let ox = coord.x * n;
    let oz = coord.y * n;
    // Heights of the (n+1)^2 grid points.
    let mut h = vec![0.0f32; ((n + 1) * (n + 1)) as usize];
    for z in 0..=n {
        for x in 0..=n {
            h[(z * (n + 1) + x) as usize] = wgen.grid_height(ox + x, oz + z);
        }
    }
    let p = |x: i32, z: i32| {
        Vec3::new((ox + x) as f32 * CELL, h[(z * (n + 1) + x) as usize], (oz + z) as f32 * CELL)
    };
    let mut buf = MeshBuffer::default();
    for z in 0..n {
        for x in 0..n {
            let (a, b, c, d) = (p(x, z), p(x + 1, z), p(x, z + 1), p(x + 1, z + 1));
            // Same split as `WorldGen::ground_height`, counter-clockwise seen from above.
            for tri in [[a, c, b], [d, b, c]] {
                let normal = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize_or_zero();
                let center = (tri[0] + tri[1] + tri[2]) / 3.0;
                buf.add_tri(tri, terrain_color(wgen, center, normal));
            }
        }
    }
    buf
}

#[allow(clippy::too_many_arguments)]
fn stream_chunks(
    mut commands: Commands,
    mut loaded: ResMut<LoadedChunks>,
    mut terrain: ResMut<TerrainRes>,
    mut cache: ResMut<PrimCache>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<ChunkMaterials>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
) {
    let Ok(cam) = camera.single() else { return };
    let seed = terrain.wgen.seed;
    if loaded.seed != Some(seed) {
        for (_, e) in loaded.map.drain() {
            commands.entity(e).try_despawn();
        }
        loaded.seed = Some(seed);
    }

    let pos = cam.translation();
    let center = chunk_of(pos.x, pos.z);

    // Unload far chunks.
    loaded.map.retain(|coord, entity| {
        let d = (*coord - center).abs();
        let keep = d.x <= VIEW_RADIUS + 1 && d.y <= VIEW_RADIUS + 1;
        if !keep {
            commands.entity(*entity).try_despawn();
        }
        keep
    });

    // Load the closest missing chunks first.
    let mut missing: Vec<IVec2> = Vec::new();
    for dz in -VIEW_RADIUS..=VIEW_RADIUS {
        for dx in -VIEW_RADIUS..=VIEW_RADIUS {
            let c = center + IVec2::new(dx, dz);
            if !loaded.map.contains_key(&c) {
                missing.push(c);
            }
        }
    }
    missing.sort_by_key(|c| (*c - center).length_squared());

    let TerrainRes(t) = &mut *terrain;
    for coord in missing.into_iter().take(BUILDS_PER_FRAME) {
        let data = t.cache.get_or_generate(&t.wgen, coord);
        let ground = build_terrain_mesh(&t.wgen, coord);

        let mut props = MeshBuffer::default();
        let mut glow = MeshBuffer::default();
        let identity = Transform::IDENTITY;
        for (i, s) in data.solids.iter().enumerate() {
            let variant = (hash_2d(seed, coord.x * 31 + i as i32, coord.y) >> 40) as f32 / (1u64 << 24) as f32;
            props.add_part(&mut cache, &models::solid(s, variant), &identity, &mut glow);
        }
        for pad in &data.pads {
            props.add_part(&mut cache, &models::jump_pad(pad.pos, pad.radius), &identity, &mut glow);
        }
        for d in &data.decor {
            props.add_part(&mut cache, &models::decor(d), &identity, &mut glow);
        }

        let root = commands.spawn((Transform::IDENTITY, Visibility::default(), Name::new(format!("chunk {coord}")))).id();
        commands.spawn((Mesh3d(meshes.add(ground.build())), MeshMaterial3d(materials.lit.clone()), ChildOf(root)));
        if !props.is_empty() {
            commands.spawn((Mesh3d(meshes.add(props.build())), MeshMaterial3d(materials.lit.clone()), ChildOf(root)));
        }
        if !glow.is_empty() {
            commands.spawn((
                Mesh3d(meshes.add(glow.build())),
                MeshMaterial3d(materials.glow.clone()),
                NotShadowCaster,
                ChildOf(root),
            ));
        }
        loaded.map.insert(coord, root);
    }
}
