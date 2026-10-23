//! Procedural, infinite, deterministic world generation.
//!
//! The world is a function of a single `seed`: every machine (server and
//! clients) computes the very same terrain and objects locally, so we never
//! have to send terrain over the network — only the seed.
//!
//! Two layers:
//! - a **height field** (`ground_height`) sampled on a global grid of `CELL`
//!   meters and linearly interpolated over triangles — *exactly* how the render
//!   mesh is built, so what you see is what you collide with;
//! - **chunks** of `CHUNK_SIZE` meters containing solid objects (rocks,
//!   pillars, floating platforms, tree trunks), jump pads, chest spots and
//!   purely decorative props.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::math::{IVec2, Vec2, Vec3};

use super::economy::ChestKind;
use super::noise::{fbm, ridged};
use super::rng::{hash_2d, Rng};

/// Side length of a chunk, in meters.
pub const CHUNK_SIZE: f32 = 32.0;
/// Terrain grid resolution, in meters (one quad = CELL x CELL).
pub const CELL: f32 = 2.0;
/// Number of quads per chunk side.
pub const CELLS_PER_CHUNK: i32 = (CHUNK_SIZE / CELL) as i32;
/// No obstacles inside this radius around the origin: players spawn there.
pub const SPAWN_CLEAR_RADIUS: f32 = 14.0;

/// What an object is made of — used by the renderer to pick a model/palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolidKind {
    Rock,
    Pillar,
    Platform,
    TreeTrunk,
    Ruin,
    Crystal,
}

/// Collision shape of a solid object. Boxes can be rotated around the Y axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Box { half: Vec3, yaw: f32 },
    Cylinder { radius: f32, half_height: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Solid {
    pub center: Vec3,
    pub shape: Shape,
    pub kind: SolidKind,
}

impl Solid {
    /// Vertical extent `(bottom, top)`.
    pub fn y_range(&self) -> (f32, f32) {
        let h = match self.shape {
            Shape::Box { half, .. } => half.y,
            Shape::Cylinder { half_height, .. } => half_height,
        };
        (self.center.y - h, self.center.y + h)
    }

    /// Horizontal (XZ) push-out vector for a circle of `radius` at `p`,
    /// or `None` if they don't overlap. The returned vector moves the circle
    /// just outside the shape.
    pub fn push_out_xz(&self, p: Vec2, radius: f32) -> Option<Vec2> {
        let c = Vec2::new(self.center.x, self.center.z);
        match self.shape {
            Shape::Cylinder { radius: r, .. } => {
                let d = p - c;
                let dist = d.length();
                let min = r + radius;
                if dist >= min {
                    return None;
                }
                let n = if dist > 1e-5 { d / dist } else { Vec2::X };
                Some(n * (min - dist))
            }
            Shape::Box { half, yaw } => {
                // Work in the box's local space, where it is axis aligned.
                let local = Vec2::from_angle(-yaw).rotate(p - c);
                let h = Vec2::new(half.x, half.z);
                let closest = local.clamp(-h, h);
                let delta = local - closest;
                let dist = delta.length();
                let push_local = if dist > 1e-5 {
                    if dist >= radius {
                        return None;
                    }
                    delta / dist * (radius - dist)
                } else {
                    // Center is inside the box: push out along the shallowest axis.
                    let dx = h.x - local.x.abs();
                    let dz = h.y - local.y.abs();
                    if dx < dz {
                        Vec2::new((dx + radius) * local.x.signum(), 0.0)
                    } else {
                        Vec2::new(0.0, (dz + radius) * local.y.signum())
                    }
                };
                Some(Vec2::from_angle(yaw).rotate(push_local))
            }
        }
    }

    /// Is the point `p` (XZ) above the top face (with some margin)?
    pub fn covers_xz(&self, p: Vec2, margin: f32) -> bool {
        let c = Vec2::new(self.center.x, self.center.z);
        match self.shape {
            Shape::Cylinder { radius, .. } => p.distance(c) <= radius + margin,
            Shape::Box { half, yaw } => {
                let local = Vec2::from_angle(-yaw).rotate(p - c);
                local.x.abs() <= half.x + margin && local.y.abs() <= half.z + margin
            }
        }
    }
}

/// A launch pad: standing on it throws you in the air.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JumpPad {
    pub pos: Vec3,
    pub radius: f32,
    pub strength: f32,
}

/// A place where the server spawns a chest. `id` is globally unique and
/// deterministic, so the server can remember which chests were opened.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChestSpot {
    pub id: u64,
    pub pos: Vec3,
    pub yaw: f32,
    pub kind: ChestKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecorKind {
    Grass,
    Flower,
    Pebble,
    Mushroom,
    Canopy,
    Shard,
}

/// Non-colliding visual prop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decor {
    pub pos: Vec3,
    pub kind: DecorKind,
    pub scale: f32,
    pub yaw: f32,
    /// Free variation value in [0,1) (color tint, shape variant...).
    pub variant: f32,
}

/// Everything inside one chunk (besides the height field).
#[derive(Clone, Debug, Default)]
pub struct ChunkData {
    pub coord: IVec2,
    pub solids: Vec<Solid>,
    pub pads: Vec<JumpPad>,
    pub chests: Vec<ChestSpot>,
    pub decor: Vec<Decor>,
}

/// The world generator. Cheap to copy: it only holds the seed.
#[derive(Clone, Copy, Debug)]
pub struct WorldGen {
    pub seed: u64,
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Chunk coordinate containing the world position `(x, z)`.
pub fn chunk_of(x: f32, z: f32) -> IVec2 {
    IVec2::new((x / CHUNK_SIZE).floor() as i32, (z / CHUNK_SIZE).floor() as i32)
}

impl WorldGen {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// The "true" continuous terrain function. Only sampled at grid points.
    pub fn raw_height(&self, x: f32, z: f32) -> f32 {
        let s = self.seed;
        // Large scale: lowlands vs highlands.
        let continent = fbm(s, x / 260.0, z / 260.0, 3);
        // Rolling hills everywhere.
        let hills = fbm(s.wrapping_add(11), x / 55.0, z / 55.0, 4);
        // Mountain ridges, only in highlands.
        let ridges = ridged(s.wrapping_add(23), x / 150.0, z / 150.0, 4);
        let highland = smoothstep(0.45, 0.72, continent);

        let mut h = (continent - 0.5) * 26.0 + (hills - 0.5) * 12.0 + ridges * ridges * 34.0 * highland;

        // Mesas: in some areas, quantize the height into 4m steps -> plateaus
        // and cliffs that reward jumping and dashing.
        let mesa = smoothstep(0.55, 0.68, fbm(s.wrapping_add(37), x / 190.0, z / 190.0, 2));
        if mesa > 0.0 {
            let step = 4.0;
            let t = (h / step).fract();
            // Soft terrace: flat for most of the step, steep at the end.
            let terr = (h / step).floor() * step + step * smoothstep(0.75, 1.0, t);
            h = h + (terr - h) * mesa;
        }

        // Flatten a safe spawn area around the origin.
        let d = Vec2::new(x, z).length();
        let flat = 1.0 - smoothstep(SPAWN_CLEAR_RADIUS, SPAWN_CLEAR_RADIUS * 2.5, d);
        let center_h = self.raw_center_height();
        h + (center_h - h) * flat
    }

    fn raw_center_height(&self) -> f32 {
        let s = self.seed;
        let continent = fbm(s, 0.0, 0.0, 3);
        let hills = fbm(s.wrapping_add(11), 0.0, 0.0, 4);
        (continent - 0.5) * 26.0 + (hills - 0.5) * 12.0
    }

    /// Height at an integer grid point (the vertices of the terrain mesh).
    pub fn grid_height(&self, ix: i32, iz: i32) -> f32 {
        self.raw_height(ix as f32 * CELL, iz as f32 * CELL)
    }

    /// Ground height anywhere: piecewise-linear interpolation over the same
    /// triangles as the render mesh. Quad split along the (1,0)-(0,1) diagonal.
    pub fn ground_height(&self, x: f32, z: f32) -> f32 {
        let gx = x / CELL;
        let gz = z / CELL;
        let ix = gx.floor() as i32;
        let iz = gz.floor() as i32;
        let fx = gx - ix as f32;
        let fz = gz - iz as f32;
        let b = self.grid_height(ix + 1, iz);
        let c = self.grid_height(ix, iz + 1);
        if fx + fz <= 1.0 {
            let a = self.grid_height(ix, iz);
            a + (b - a) * fx + (c - a) * fz
        } else {
            let d = self.grid_height(ix + 1, iz + 1);
            d + (c - d) * (1.0 - fx) + (b - d) * (1.0 - fz)
        }
    }

    /// Generates the content of a chunk. Pure function of `(seed, coord)`.
    pub fn generate_chunk(&self, coord: IVec2) -> ChunkData {
        let mut rng = Rng::new(hash_2d(self.seed ^ 0xC4C4_0000_5EED, coord.x, coord.y));
        let mut chunk = ChunkData { coord, ..Default::default() };
        let origin = Vec2::new(coord.x as f32 * CHUNK_SIZE, coord.y as f32 * CHUNK_SIZE);

        // A random point inside the chunk (with a margin), away from spawn.
        let random_point = |rng: &mut Rng, margin: f32| -> Option<Vec2> {
            for _ in 0..6 {
                let p = origin
                    + Vec2::new(
                        rng.range(margin, CHUNK_SIZE - margin),
                        rng.range(margin, CHUNK_SIZE - margin),
                    );
                if p.length() > SPAWN_CLEAR_RADIUS + 2.0 {
                    return Some(p);
                }
            }
            None
        };

        // Each chunk gets a "theme" so the world feels varied.
        let theme = rng.weighted_index(&[3.0, 2.5, 2.5, 1.5, 1.0]);
        let mut chest_index = 0u64;
        let mut add_chest = |chunk: &mut ChunkData, rng: &mut Rng, pos: Vec3, kind: ChestKind| {
            chunk.chests.push(ChestSpot {
                id: hash_2d(self.seed ^ 0xC0FF_EE00, coord.x, coord.y) ^ chest_index,
                pos,
                yaw: rng.range(0.0, std::f32::consts::TAU),
                kind,
            });
            chest_index += 1;
        };

        match theme {
            // 0: Rocky field — boulders of various sizes.
            0 => {
                for _ in 0..rng.range_u32(3, 7) {
                    let Some(p) = random_point(&mut rng, 2.0) else { continue };
                    let half = Vec3::new(rng.range(0.8, 2.6), rng.range(0.6, 2.2), rng.range(0.8, 2.6));
                    let g = self.ground_height(p.x, p.y);
                    chunk.solids.push(Solid {
                        center: Vec3::new(p.x, g + half.y * 0.7, p.y),
                        shape: Shape::Box { half, yaw: rng.range(0.0, std::f32::consts::PI) },
                        kind: SolidKind::Rock,
                    });
                }
            }
            // 1: Ruins — pillars holding floating platforms, stair-like.
            1 => {
                let Some(base) = random_point(&mut rng, 8.0) else { return chunk };
                let g = self.ground_height(base.x, base.y);
                let steps = rng.range_u32(2, 5);
                let dir = Vec2::from_angle(rng.range(0.0, std::f32::consts::TAU));
                let mut top = None;
                for i in 0..steps {
                    let p = base + dir * (i as f32 * 5.5);
                    let height = g + 2.6 + i as f32 * 2.4;
                    let half = Vec3::new(rng.range(1.8, 2.8), 0.35, rng.range(1.8, 2.8));
                    chunk.solids.push(Solid {
                        center: Vec3::new(p.x, height, p.y),
                        shape: Shape::Box { half, yaw: dir.to_angle() },
                        kind: SolidKind::Platform,
                    });
                    // Pillar under every other platform (decorative support).
                    if i % 2 == 0 {
                        let gp = self.ground_height(p.x, p.y);
                        let hh = (height - 0.35 - gp).max(0.5) * 0.5;
                        chunk.solids.push(Solid {
                            center: Vec3::new(p.x, gp + hh, p.y),
                            shape: Shape::Cylinder { radius: 0.6, half_height: hh },
                            kind: SolidKind::Pillar,
                        });
                    }
                    top = Some(Vec3::new(p.x, height + 0.35, p.y));
                }
                // Reward climbing: a chest on the highest platform.
                if let Some(t) = top {
                    let kind = if rng.chance(0.35) { ChestKind::Weapon } else { ChestKind::Small };
                    add_chest(&mut chunk, &mut rng, t, kind);
                }
                // Broken walls around.
                for _ in 0..rng.range_u32(1, 4) {
                    let Some(p) = random_point(&mut rng, 3.0) else { continue };
                    let half = Vec3::new(rng.range(1.5, 3.5), rng.range(1.0, 2.2), 0.4);
                    let g = self.ground_height(p.x, p.y);
                    chunk.solids.push(Solid {
                        center: Vec3::new(p.x, g + half.y - 0.3, p.y),
                        shape: Shape::Box { half, yaw: rng.range(0.0, std::f32::consts::PI) },
                        kind: SolidKind::Ruin,
                    });
                }
            }
            // 2: Forest.
            2 => {
                for _ in 0..rng.range_u32(4, 10) {
                    let Some(p) = random_point(&mut rng, 1.5) else { continue };
                    let g = self.ground_height(p.x, p.y);
                    let h = rng.range(2.5, 4.5);
                    chunk.solids.push(Solid {
                        center: Vec3::new(p.x, g + h * 0.5, p.y),
                        shape: Shape::Cylinder { radius: 0.35, half_height: h * 0.5 },
                        kind: SolidKind::TreeTrunk,
                    });
                    chunk.decor.push(Decor {
                        pos: Vec3::new(p.x, g + h, p.y),
                        kind: DecorKind::Canopy,
                        scale: rng.range(1.6, 2.6),
                        yaw: rng.range(0.0, std::f32::consts::TAU),
                        variant: rng.next_f32(),
                    });
                }
            }
            // 3: Tower with a jump pad — big vertical moment.
            3 => {
                let Some(p) = random_point(&mut rng, 7.0) else { return chunk };
                let g = self.ground_height(p.x, p.y);
                let height = rng.range(9.0, 14.0);
                let half = Vec3::new(3.0, 0.4, 3.0);
                chunk.solids.push(Solid {
                    center: Vec3::new(p.x, g + height, p.y),
                    shape: Shape::Box { half, yaw: 0.0 },
                    kind: SolidKind::Platform,
                });
                let hh = (height - 0.4) * 0.5;
                chunk.solids.push(Solid {
                    center: Vec3::new(p.x, g + hh, p.y),
                    shape: Shape::Cylinder { radius: 1.0, half_height: hh },
                    kind: SolidKind::Pillar,
                });
                // Jump pad next to the tower, strong enough to reach the top.
                let pad_pos = p + Vec2::new(4.5, 0.0);
                let pg = self.ground_height(pad_pos.x, pad_pos.y);
                chunk.pads.push(JumpPad {
                    pos: Vec3::new(pad_pos.x, pg, pad_pos.y),
                    radius: 1.2,
                    // v = sqrt(2 g h) with some margin; gravity is in movement.rs.
                    strength: (2.0 * super::movement::GRAVITY * (g + height + 3.0 - pg)).sqrt(),
                });
                let kind = if rng.chance(0.5) { ChestKind::Weapon } else { ChestKind::Small };
                add_chest(&mut chunk, &mut rng, Vec3::new(p.x, g + height + 0.4, p.y), kind);
            }
            // 4: Crystal field — tall shards, a little alien.
            _ => {
                for _ in 0..rng.range_u32(3, 6) {
                    let Some(p) = random_point(&mut rng, 2.0) else { continue };
                    let g = self.ground_height(p.x, p.y);
                    let hh = rng.range(1.5, 4.0);
                    chunk.solids.push(Solid {
                        center: Vec3::new(p.x, g + hh * 0.8, p.y),
                        shape: Shape::Cylinder { radius: rng.range(0.5, 1.0), half_height: hh },
                        kind: SolidKind::Crystal,
                    });
                }
            }
        }

        // Ground chests (more common than platform ones).
        if rng.chance(0.45)
            && let Some(p) = random_point(&mut rng, 3.0)
        {
            let pos = Vec3::new(p.x, self.ground_height(p.x, p.y), p.y);
            if !chunk.solids.iter().any(|s| s.push_out_xz(p, 1.2).is_some()) {
                let kind = if rng.chance(0.15) { ChestKind::Weapon } else { ChestKind::Small };
                add_chest(&mut chunk, &mut rng, pos, kind);
            }
        }

        // Occasional free-standing jump pad for fun.
        if theme != 3
            && rng.chance(0.12)
            && let Some(p) = random_point(&mut rng, 3.0)
            && !chunk.solids.iter().any(|s| s.push_out_xz(p, 2.0).is_some())
        {
            chunk.pads.push(JumpPad {
                pos: Vec3::new(p.x, self.ground_height(p.x, p.y), p.y),
                radius: 1.2,
                strength: 19.0,
            });
        }

        // Small decorative props everywhere.
        for _ in 0..rng.range_u32(10, 22) {
            let p = origin + Vec2::new(rng.range(0.0, CHUNK_SIZE), rng.range(0.0, CHUNK_SIZE));
            let kind = *rng.pick(&[
                DecorKind::Grass,
                DecorKind::Grass,
                DecorKind::Grass,
                DecorKind::Flower,
                DecorKind::Pebble,
                DecorKind::Mushroom,
                DecorKind::Shard,
            ]);
            chunk.decor.push(Decor {
                pos: Vec3::new(p.x, self.ground_height(p.x, p.y), p.y),
                kind,
                scale: rng.range(0.6, 1.4),
                yaw: rng.range(0.0, std::f32::consts::TAU),
                variant: rng.next_f32(),
            });
        }

        // Multi-part structures (ruin stairs) can extend toward the origin:
        // enforce the clear spawn area on the final result.
        let outside = |p: Vec3, extent: f32| Vec2::new(p.x, p.z).length() - extent > SPAWN_CLEAR_RADIUS;
        chunk.solids.retain(|s| {
            let extent = match s.shape {
                Shape::Box { half, .. } => half.x.max(half.z) * std::f32::consts::SQRT_2,
                Shape::Cylinder { radius, .. } => radius,
            };
            outside(s.center, extent)
        });
        chunk.chests.retain(|c| outside(c.pos, 4.5));
        chunk.pads.retain(|p| outside(p.pos, p.radius + 1.0));

        chunk
    }
}

/// Lazily generated chunks, shared by physics (server + client prediction)
/// and rendering. `Arc` = cheap shared ownership, the data is immutable.
#[derive(Default)]
pub struct ChunkCache {
    chunks: HashMap<IVec2, Arc<ChunkData>>,
}

impl ChunkCache {
    pub fn get(&self, coord: IVec2) -> Option<&Arc<ChunkData>> {
        self.chunks.get(&coord)
    }

    pub fn get_or_generate(&mut self, wgen: &WorldGen, coord: IVec2) -> Arc<ChunkData> {
        self.chunks
            .entry(coord)
            .or_insert_with(|| Arc::new(wgen.generate_chunk(coord)))
            .clone()
    }

    /// Makes sure all chunks within `radius` chunks of `pos` exist.
    pub fn ensure_around(&mut self, wgen: &WorldGen, pos: Vec3, radius: i32) {
        let c = chunk_of(pos.x, pos.z);
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                self.get_or_generate(wgen, c + IVec2::new(dx, dz));
            }
        }
    }

    /// Drops chunks farther than `keep_radius` (in chunks) from every point.
    pub fn retain_near(&mut self, points: &[Vec3], keep_radius: i32) {
        let centers: Vec<IVec2> = points.iter().map(|p| chunk_of(p.x, p.z)).collect();
        self.chunks.retain(|coord, _| {
            centers.iter().any(|c| {
                let d = (*coord - *c).abs();
                d.x <= keep_radius && d.y <= keep_radius
            })
        });
    }

    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Calls `f` for every solid whose chunk is within reach of `pos`.
    /// Objects never extend more than ~half a chunk outside their chunk,
    /// so checking the 3x3 neighborhood is enough.
    pub fn for_each_solid_near(&self, pos: Vec3, mut f: impl FnMut(&Solid)) {
        let c = chunk_of(pos.x, pos.z);
        for dz in -1..=1 {
            for dx in -1..=1 {
                if let Some(chunk) = self.chunks.get(&(c + IVec2::new(dx, dz))) {
                    for s in &chunk.solids {
                        f(s);
                    }
                }
            }
        }
    }

    pub fn for_each_pad_near(&self, pos: Vec3, mut f: impl FnMut(&JumpPad)) {
        let c = chunk_of(pos.x, pos.z);
        for dz in -1..=1 {
            for dx in -1..=1 {
                if let Some(chunk) = self.chunks.get(&(c + IVec2::new(dx, dz))) {
                    for p in &chunk.pads {
                        f(p);
                    }
                }
            }
        }
    }
}

/// Everything physics needs to know about the world. Bundled in one struct so
/// systems can pass `&Terrain` around.
pub struct Terrain {
    pub wgen: WorldGen,
    pub cache: ChunkCache,
}

impl Terrain {
    pub fn new(seed: u64) -> Self {
        Self { wgen: WorldGen::new(seed), cache: ChunkCache::default() }
    }

    /// Highest walkable surface below `pos` (terrain or the top of a solid),
    /// considering a body of horizontal `radius`.
    pub fn floor_below(&self, pos: Vec3, radius: f32, step: f32) -> f32 {
        let mut floor = self.wgen.ground_height(pos.x, pos.z);
        let p = Vec2::new(pos.x, pos.z);
        self.cache.for_each_solid_near(pos, |s| {
            let (_, top) = s.y_range();
            if top <= pos.y + step && top > floor && s.covers_xz(p, radius * 0.6) {
                floor = top;
            }
        });
        floor
    }

    /// Simple ray march against terrain + solids (used for hitscan and AI line of sight).
    /// Returns the distance to the first hit, if any within `max_dist`.
    pub fn raycast(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<f32> {
        let dir = dir.normalize_or_zero();
        if dir == Vec3::ZERO {
            return None;
        }
        let step = 0.5;
        let mut t = 0.0;
        while t < max_dist {
            let p = origin + dir * t;
            if p.y < self.wgen.ground_height(p.x, p.z) {
                return Some(t);
            }
            let mut hit = false;
            self.cache.for_each_solid_near(p, |s| {
                if hit {
                    return;
                }
                let (bottom, top) = s.y_range();
                if p.y >= bottom && p.y <= top && s.push_out_xz(Vec2::new(p.x, p.z), 0.01).is_some() {
                    hit = true;
                }
            });
            if hit {
                return Some(t);
            }
            t += step;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_world() {
        let a = WorldGen::new(1234);
        let b = WorldGen::new(1234);
        for i in -20..20 {
            let x = i as f32 * 13.7;
            let z = i as f32 * -7.3;
            assert_eq!(a.ground_height(x, z), b.ground_height(x, z));
        }
        for cx in -3..3 {
            for cz in -3..3 {
                let ca = a.generate_chunk(IVec2::new(cx, cz));
                let cb = b.generate_chunk(IVec2::new(cx, cz));
                assert_eq!(ca.solids, cb.solids);
                assert_eq!(ca.chests, cb.chests);
                assert_eq!(ca.pads, cb.pads);
                assert_eq!(ca.decor, cb.decor);
            }
        }
    }

    #[test]
    fn different_seed_different_world() {
        let a = WorldGen::new(1);
        let b = WorldGen::new(2);
        let differs = (0..50).any(|i| {
            let x = 100.0 + i as f32 * 9.1;
            a.ground_height(x, x) != b.ground_height(x, x)
        });
        assert!(differs);
    }

    #[test]
    fn ground_height_matches_grid_at_vertices() {
        let g = WorldGen::new(99);
        for ix in -5..5 {
            for iz in -5..5 {
                let h = g.ground_height(ix as f32 * CELL, iz as f32 * CELL);
                assert!((h - g.grid_height(ix, iz)).abs() < 1e-3);
            }
        }
    }

    #[test]
    fn spawn_area_is_clear_and_flat() {
        for seed in 0..20 {
            let g = WorldGen::new(seed);
            for cx in -1..=0 {
                for cz in -1..=0 {
                    let chunk = g.generate_chunk(IVec2::new(cx, cz));
                    for s in &chunk.solids {
                        let d = Vec2::new(s.center.x, s.center.z).length();
                        assert!(d > SPAWN_CLEAR_RADIUS, "seed {seed}: solid at {d}");
                    }
                }
            }
            let h0 = g.ground_height(0.0, 0.0);
            let h1 = g.ground_height(5.0, 5.0);
            assert!((h0 - h1).abs() < 0.5, "spawn not flat for seed {seed}");
        }
    }

    #[test]
    fn chest_ids_are_unique() {
        let g = WorldGen::new(5);
        let mut ids = std::collections::HashSet::new();
        for cx in -8..8 {
            for cz in -8..8 {
                for c in g.generate_chunk(IVec2::new(cx, cz)).chests {
                    assert!(ids.insert(c.id), "duplicate chest id");
                }
            }
        }
        assert!(ids.len() > 20, "too few chests: {}", ids.len());
    }

    #[test]
    fn box_push_out() {
        let s = Solid {
            center: Vec3::ZERO,
            shape: Shape::Box { half: Vec3::ONE, yaw: 0.0 },
            kind: SolidKind::Rock,
        };
        // Touching from the right side: pushed toward +X.
        let push = s.push_out_xz(Vec2::new(1.2, 0.0), 0.5).unwrap();
        assert!(push.x > 0.0 && push.y.abs() < 1e-5);
        // Far away: nothing.
        assert!(s.push_out_xz(Vec2::new(3.0, 0.0), 0.5).is_none());
    }

    #[test]
    fn cache_eviction() {
        let t = WorldGen::new(1);
        let mut cache = ChunkCache::default();
        cache.ensure_around(&t, Vec3::ZERO, 2);
        assert_eq!(cache.len(), 25);
        cache.retain_near(&[Vec3::new(1000.0, 0.0, 1000.0)], 2);
        assert!(cache.is_empty());
    }
}
