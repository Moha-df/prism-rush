//! The procedural world as an ECS resource, kept in sync with the match seed.
//! Runs on server *and* clients.

use bevy::prelude::*;

use crate::net::protocol::{MatchInfo, MobState, PlayerState};
use crate::shared::worldgen::Terrain;

/// Terrain + chunk cache. `Deref` lets us write `terrain.gen` instead of `terrain.0.gen`.
#[derive(Resource, Deref, DerefMut)]
pub struct TerrainRes(pub Terrain);

impl Default for TerrainRes {
    fn default() -> Self {
        Self(Terrain::new(0))
    }
}

/// How many chunks around each player must exist for physics.
pub const PHYSICS_CHUNK_RADIUS: i32 = 3;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainRes>()
            .add_systems(PreUpdate, sync_seed)
            .add_systems(FixedPreUpdate, ensure_chunks.run_if(super::match_playing))
            .add_systems(Update, evict_chunks.run_if(bevy::time::common_conditions::on_timer(
                std::time::Duration::from_secs(5),
            )));
    }
}

/// When a new run starts (new seed), rebuild the terrain.
fn sync_seed(info: Query<&MatchInfo, Changed<MatchInfo>>, mut terrain: ResMut<TerrainRes>) {
    for m in &info {
        if m.seed != terrain.wgen.seed {
            info!("new world seed {}", m.seed);
            terrain.0 = Terrain::new(m.seed);
        }
    }
}

/// Generates chunks around every player and monster before the simulation.
fn ensure_chunks(
    mut terrain: ResMut<TerrainRes>,
    players: Query<&PlayerState>,
    mobs: Query<&MobState>,
) {
    let TerrainRes(t) = &mut *terrain;
    for p in &players {
        t.cache.ensure_around(&t.wgen, p.sim.mv.pos, PHYSICS_CHUNK_RADIUS);
    }
    for m in &mobs {
        t.cache.ensure_around(&t.wgen, m.pos, 1);
    }
}

/// Forgets chunks far from every player (memory stays bounded on long runs).
fn evict_chunks(mut terrain: ResMut<TerrainRes>, players: Query<&PlayerState>) {
    let points: Vec<Vec3> = players.iter().map(|p| p.sim.mv.pos).collect();
    if points.is_empty() {
        return;
    }
    let before = terrain.cache.len();
    terrain.cache.retain_near(&points, PHYSICS_CHUNK_RADIUS + 4);
    let after = terrain.cache.len();
    if before != after {
        debug!("evicted {} chunks ({} kept)", before - after, after);
    }
}
