//! Procedural models assembled from primitives.
//!
//! A model is a tree of `Part`s. Each part is a primitive (box, sphere,
//! cylinder, cone, prism...) with a size, position, rotation, color and an
//! optional glow. Parts can have children and an animation tag.
//!
//! The same description can be used in two ways:
//! - `spawn_model`: one entity per part (animated characters, chests, weapons);
//! - `MeshBuffer::add_part`: baked into a single vertex-colored mesh (static
//!   world props: thousands of rocks/trees cost one draw call per chunk).
//!
//! Every mesh is **flat shaded** (one normal per triangle): the faceted
//! low-poly look, with consistent facet density across the whole game.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

/// The primitive shapes, all "unit sized" (fit in a 1x1x1 box, centered).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Prim {
    /// No geometry: a pivot used to group/animate children.
    Empty,
    Box,
    /// Low-poly sphere (icosphere, 1 subdivision).
    Sphere,
    /// Even lower poly sphere (icosahedron): crystals, rocks, eyes.
    Ico,
    Cylinder,
    Cone,
    /// Cone with a flat top (truncated), ratio top/bottom = 0.5.
    Frustum,
    Capsule,
    /// Triangular prism (extruded triangle, pointing +Y).
    Prism,
    Torus,
}

/// Names for parts that animations move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnimTag {
    Body,
    Head,
    ArmL,
    ArmR,
    LegL,
    LegR,
    /// Insect leg with a phase offset.
    Leg(f32),
    Wing(f32),
    Jaw,
    /// Chest lid (opens when the chest is opened).
    Lid,
    /// Constant rotation around Y (radians per second).
    Spin(f32),
    /// Floating up/down.
    Bob,
    /// Swells while the monster prepares its attack.
    Pulse,
    /// Weapon parts moving on fire (slide, pump).
    Recoil,
}

#[derive(Clone, Debug)]
pub struct Part {
    pub prim: Prim,
    pub size: Vec3,
    pub pos: Vec3,
    pub rot: Quat,
    pub color: Color,
    /// 0 = matte; > 0 = emissive strength (glows with bloom).
    pub glow: f32,
    pub tag: Option<AnimTag>,
    pub children: Vec<Part>,
}

// --- A tiny builder API: `Part::cube(1.0, 2.0, 1.0).at(0.0, 1.0, 0.0).color(RED)`.
impl Part {
    pub fn new(prim: Prim, size: Vec3) -> Self {
        Self {
            prim,
            size,
            pos: Vec3::ZERO,
            rot: Quat::IDENTITY,
            color: Color::WHITE,
            glow: 0.0,
            tag: None,
            children: Vec::new(),
        }
    }
    pub fn pivot() -> Self {
        Self::new(Prim::Empty, Vec3::ONE)
    }
    pub fn cube(x: f32, y: f32, z: f32) -> Self {
        Self::new(Prim::Box, Vec3::new(x, y, z))
    }
    pub fn sphere(d: f32) -> Self {
        Self::new(Prim::Sphere, Vec3::splat(d))
    }
    pub fn ellipsoid(x: f32, y: f32, z: f32) -> Self {
        Self::new(Prim::Sphere, Vec3::new(x, y, z))
    }
    pub fn ico(x: f32, y: f32, z: f32) -> Self {
        Self::new(Prim::Ico, Vec3::new(x, y, z))
    }
    /// Cylinder along Y: diameter `d`, height `h`.
    pub fn cylinder(d: f32, h: f32) -> Self {
        Self::new(Prim::Cylinder, Vec3::new(d, h, d))
    }
    /// Cone along +Y: base diameter `d`, height `h`.
    pub fn cone(d: f32, h: f32) -> Self {
        Self::new(Prim::Cone, Vec3::new(d, h, d))
    }
    pub fn frustum(d: f32, h: f32) -> Self {
        Self::new(Prim::Frustum, Vec3::new(d, h, d))
    }
    pub fn capsule(d: f32, h: f32) -> Self {
        Self::new(Prim::Capsule, Vec3::new(d, h, d))
    }
    pub fn prism(x: f32, y: f32, z: f32) -> Self {
        Self::new(Prim::Prism, Vec3::new(x, y, z))
    }
    pub fn torus(d: f32, thickness: f32) -> Self {
        Self::new(Prim::Torus, Vec3::new(d, thickness, d))
    }

    pub fn at(mut self, x: f32, y: f32, z: f32) -> Self {
        self.pos = Vec3::new(x, y, z);
        self
    }
    pub fn at_v(mut self, p: Vec3) -> Self {
        self.pos = p;
        self
    }
    /// Rotations in degrees (readability over radians when designing models).
    pub fn rot_x(mut self, deg: f32) -> Self {
        self.rot *= Quat::from_rotation_x(deg.to_radians());
        self
    }
    pub fn rot_y(mut self, deg: f32) -> Self {
        self.rot *= Quat::from_rotation_y(deg.to_radians());
        self
    }
    pub fn rot_z(mut self, deg: f32) -> Self {
        self.rot *= Quat::from_rotation_z(deg.to_radians());
        self
    }
    pub fn color(mut self, c: Color) -> Self {
        self.color = c;
        self
    }
    pub fn glow(mut self, strength: f32) -> Self {
        self.glow = strength;
        self
    }
    pub fn tag(mut self, tag: AnimTag) -> Self {
        self.tag = Some(tag);
        self
    }
    pub fn with(mut self, child: Part) -> Self {
        self.children.push(child);
        self
    }
    pub fn with_all(mut self, children: impl IntoIterator<Item = Part>) -> Self {
        self.children.extend(children);
        self
    }
    /// Mirrors a part (and its children) across the X axis: design one side, get both.
    pub fn mirrored_x(&self) -> Part {
        let mut p = self.clone();
        p.pos.x = -p.pos.x;
        // Mirror the rotation: negate the Y and Z components of the quaternion.
        p.rot = Quat::from_xyzw(p.rot.x, -p.rot.y, -p.rot.z, p.rot.w);
        p.tag = match p.tag {
            Some(AnimTag::ArmL) => Some(AnimTag::ArmR),
            Some(AnimTag::ArmR) => Some(AnimTag::ArmL),
            Some(AnimTag::LegL) => Some(AnimTag::LegR),
            Some(AnimTag::LegR) => Some(AnimTag::LegL),
            Some(AnimTag::Leg(ph)) => Some(AnimTag::Leg(ph + 0.5)),
            Some(AnimTag::Wing(ph)) => Some(AnimTag::Wing(-ph)),
            other => other,
        };
        p.children = p.children.iter().map(Part::mirrored_x).collect();
        p
    }
    pub fn transform(&self) -> Transform {
        Transform { translation: self.pos, rotation: self.rot, scale: Vec3::ONE }
    }
}

/// A list of triangles, the common currency of this module.
pub type Tris = Vec<[Vec3; 3]>;

/// Extracts triangles from a Bevy mesh (positions + indices).
fn mesh_to_tris(mesh: &Mesh) -> Tris {
    let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
        return Vec::new();
    };
    let p = |i: usize| Vec3::from(pos[i]);
    let indices: Vec<usize> = match mesh.indices() {
        Some(Indices::U16(v)) => v.iter().map(|&i| i as usize).collect(),
        Some(Indices::U32(v)) => v.iter().map(|&i| i as usize).collect(),
        None => (0..pos.len()).collect(),
    };
    indices.chunks_exact(3).map(|t| [p(t[0]), p(t[1]), p(t[2])]).collect()
}

/// Unit-sized triangles for each primitive. Low resolution on purpose.
pub fn unit_tris(prim: Prim) -> Tris {
    let mesh: Mesh = match prim {
        Prim::Empty => return Vec::new(),
        Prim::Box => Cuboid::new(1.0, 1.0, 1.0).into(),
        Prim::Sphere => Sphere::new(0.5).mesh().ico(1).expect("ico"),
        Prim::Ico => Sphere::new(0.5).mesh().ico(0).expect("ico"),
        Prim::Cylinder => Cylinder::new(0.5, 1.0).mesh().resolution(8).build(),
        Prim::Cone => Cone { radius: 0.5, height: 1.0 }.mesh().resolution(8).build(),
        Prim::Frustum => ConicalFrustum { radius_top: 0.25, radius_bottom: 0.5, height: 1.0 }
            .mesh()
            .resolution(8)
            .build(),
        Prim::Capsule => Capsule3d::new(0.5, 1.0).mesh().longitudes(8).latitudes(6).build(),
        Prim::Prism => Extrusion::new(
            Triangle2d::new(Vec2::new(0.0, 0.5), Vec2::new(-0.5, -0.5), Vec2::new(0.5, -0.5)),
            1.0,
        )
        .into(),
        Prim::Torus => Torus::new(0.3, 0.5).mesh().minor_resolution(6).major_resolution(12).build(),
    };
    let mut tris = mesh_to_tris(&mesh);
    // Capsule3d is 2 units tall (half_length 0.5 + radius 0.5 on each side): normalize.
    if prim == Prim::Capsule {
        for t in &mut tris {
            for v in t.iter_mut() {
                v.y *= 0.5;
            }
        }
    }
    // Torus: ring radius 0.5 + tube 0.1 => normalize vertical to 1 unit thickness.
    if prim == Prim::Torus {
        for t in &mut tris {
            for v in t.iter_mut() {
                v.y *= 5.0;
            }
        }
    }
    tris
}

/// Builds a flat-shaded mesh from triangles, with optional per-vertex colors.
pub fn tris_to_mesh(tris: &[[Vec3; 3]], colors: Option<&[[f32; 4]]>) -> Mesh {
    let mut positions = Vec::with_capacity(tris.len() * 3);
    let mut normals = Vec::with_capacity(tris.len() * 3);
    for t in tris {
        let n = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
        for v in t {
            positions.push(v.to_array());
            normals.push(n.to_array());
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    if let Some(c) = colors {
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, c.to_vec());
    }
    mesh
}

/// Accumulates many colored parts into one mesh (static world props).
#[derive(Default)]
pub struct MeshBuffer {
    pub tris: Tris,
    pub colors: Vec<[f32; 4]>,
}

impl MeshBuffer {
    pub fn add_tri(&mut self, tri: [Vec3; 3], color: [f32; 4]) {
        self.tris.push(tri);
        self.colors.extend([color; 3]);
    }

    /// Adds a part tree with a parent transform. Glowing parts are routed to `glow`.
    pub fn add_part(&mut self, cache: &mut PrimCache, part: &Part, parent: &Transform, glow: &mut MeshBuffer) {
        let local = parent.mul_transform(part.transform());
        if part.prim != Prim::Empty {
            let lin = part.color.to_linear();
            let (target, mult) = if part.glow > 0.0 { (&mut *glow, part.glow) } else { (&mut *self, 1.0) };
            let color = [lin.red * mult, lin.green * mult, lin.blue * mult, 1.0];
            let m = local.to_matrix() * Mat4::from_scale(part.size);
            for t in cache.tris(part.prim) {
                target.add_tri(
                    [m.transform_point3(t[0]), m.transform_point3(t[1]), m.transform_point3(t[2])],
                    color,
                );
            }
        }
        for child in &part.children {
            self.add_part(cache, child, &local, glow);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tris.is_empty()
    }

    pub fn build(&self) -> Mesh {
        tris_to_mesh(&self.tris, Some(&self.colors))
    }
}

/// Caches unit triangles, scaled meshes and materials so identical parts
/// share GPU resources (and get batched into fewer draw calls).
#[derive(Resource, Default)]
pub struct PrimCache {
    unit: HashMap<Prim, Tris>,
    meshes: HashMap<(Prim, [i32; 3]), Handle<Mesh>>,
    materials: HashMap<([u8; 4], u16), Handle<StandardMaterial>>,
}

impl PrimCache {
    pub fn tris(&mut self, prim: Prim) -> &Tris {
        self.unit.entry(prim).or_insert_with(|| unit_tris(prim))
    }

    pub fn mesh(&mut self, meshes: &mut Assets<Mesh>, prim: Prim, size: Vec3) -> Handle<Mesh> {
        let key = (prim, (size * 100.0).round().as_ivec3().to_array());
        if let Some(h) = self.meshes.get(&key) {
            return h.clone();
        }
        let tris: Tris = self.tris(prim).iter().map(|t| [t[0] * size, t[1] * size, t[2] * size]).collect();
        let handle = meshes.add(tris_to_mesh(&tris, None));
        self.meshes.insert(key, handle.clone());
        handle
    }

    pub fn material(&mut self, materials: &mut Assets<StandardMaterial>, color: Color, glow: f32) -> Handle<StandardMaterial> {
        let s = color.to_srgba();
        let key = ([s.red, s.green, s.blue, s.alpha].map(|c| (c * 255.0) as u8), (glow * 10.0) as u16);
        self.materials
            .entry(key)
            .or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: color,
                    perceptual_roughness: 0.85,
                    reflectance: 0.3,
                    emissive: if glow > 0.0 { color.to_linear() * glow } else { LinearRgba::BLACK },
                    ..default()
                })
            })
            .clone()
    }
}

/// Animated part: remembers its rest pose and which model it belongs to.
#[derive(Component, Clone, Copy, Debug)]
pub struct AnimPart {
    pub tag: AnimTag,
    pub root: Entity,
    pub rest: Transform,
}

/// Spawns a model as a child hierarchy of `parent`. Returns nothing: the
/// parts are found again through `AnimPart::root`.
#[allow(clippy::too_many_arguments)]
pub fn spawn_model(
    commands: &mut Commands,
    cache: &mut PrimCache,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    root: Entity,
    parent: Entity,
    part: &Part,
    extra: impl Bundle + Clone,
) {
    let transform = part.transform();
    let mut e = commands.spawn((transform, Visibility::default(), ChildOf(parent), extra.clone()));
    if part.prim != Prim::Empty {
        e.insert((
            Mesh3d(cache.mesh(meshes, part.prim, part.size)),
            MeshMaterial3d(cache.material(materials, part.color, part.glow)),
        ));
        if part.glow > 0.0 {
            e.insert(NotShadowCaster);
        }
    }
    if let Some(tag) = part.tag {
        e.insert(AnimPart { tag, root, rest: transform });
    }
    let id = e.id();
    for child in &part.children {
        spawn_model(commands, cache, meshes, materials, root, id, child, extra.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_prims_fit_in_unit_box() {
        for prim in [Prim::Box, Prim::Sphere, Prim::Ico, Prim::Cylinder, Prim::Cone, Prim::Frustum, Prim::Capsule, Prim::Prism, Prim::Torus] {
            let tris = unit_tris(prim);
            assert!(!tris.is_empty(), "{prim:?}");
            for t in &tris {
                for v in t {
                    assert!(v.abs().max_element() <= 0.5 + 1e-3, "{prim:?} vertex {v}");
                }
            }
        }
    }

    #[test]
    fn mirroring_swaps_sides() {
        let p = Part::cube(1.0, 1.0, 1.0).at(1.0, 0.0, 0.0).tag(AnimTag::ArmL);
        let m = p.mirrored_x();
        assert_eq!(m.pos.x, -1.0);
        assert_eq!(m.tag, Some(AnimTag::ArmR));
    }

    #[test]
    fn mesh_buffer_bakes_parts() {
        let mut cache = PrimCache::default();
        let mut buf = MeshBuffer::default();
        let mut glow = MeshBuffer::default();
        let part = Part::cube(2.0, 1.0, 1.0).with(Part::sphere(0.5).glow(2.0));
        buf.add_part(&mut cache, &part, &Transform::default(), &mut glow);
        assert_eq!(buf.tris.len(), 12); // a box = 12 triangles
        assert!(!glow.is_empty());
        assert_eq!(buf.colors.len(), buf.tris.len() * 3);
    }
}
