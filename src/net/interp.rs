//! Snapshot interpolation for *other* players and monsters.
//!
//! We receive their positions ~60 times per second, but packets arrive with
//! jitter. Instead of showing the latest one (stuttery), we show them slightly
//! in the past (`INTERP_TICKS`) and blend between the two snapshots around
//! that time: perfectly smooth motion at the cost of 100 ms of visual delay.

use std::collections::VecDeque;

use bevy::prelude::*;

use super::client::ServerClock;
use super::protocol::{MobState, Player, PlayerState, Projectile};
use super::{LocalPlayer, NetRole};

#[derive(Component, Debug, Default)]
pub struct InterpBuffer {
    samples: VecDeque<Snapshot>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snapshot {
    pub tick: u32,
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

/// Linear interpolation between angles, taking the shortest way around.
pub fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    let mut d = (b - a) % std::f32::consts::TAU;
    if d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    } else if d < -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    a + d * t
}

impl InterpBuffer {
    pub fn push(&mut self, s: Snapshot) {
        if self.samples.back().is_some_and(|last| last.tick >= s.tick) {
            return; // out of order or duplicate
        }
        self.samples.push_back(s);
        while self.samples.len() > 32 {
            self.samples.pop_front();
        }
    }

    /// State at (fractional) `tick`. Holds the newest snapshot if we run out
    /// of data (better than extrapolating wildly).
    pub fn sample(&self, tick: f64) -> Option<Snapshot> {
        let first = self.samples.front()?;
        if tick <= f64::from(first.tick) {
            return Some(*first);
        }
        for pair in self.samples.iter().collect::<Vec<_>>().windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if tick <= f64::from(b.tick) {
                let t = ((tick - f64::from(a.tick)) / f64::from(b.tick - a.tick)) as f32;
                return Some(Snapshot {
                    tick: a.tick,
                    pos: a.pos.lerp(b.pos, t),
                    yaw: lerp_angle(a.yaw, b.yaw, t),
                    pitch: a.pitch + (b.pitch - a.pitch) * t,
                });
            }
        }
        self.samples.back().copied()
    }

    pub fn latest(&self) -> Option<Snapshot> {
        self.samples.back().copied()
    }
}

pub struct InterpPlugin;

impl Plugin for InterpPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (record_snapshots, apply_interpolation)
                .chain()
                .before(TransformSystems::Propagate)
                .run_if(|role: Res<NetRole>| *role != NetRole::Dedicated),
        );
    }
}

fn record_snapshots(
    mut commands: Commands,
    local: Res<LocalPlayer>,
    mut mobs: Query<(Entity, &MobState, Option<&mut InterpBuffer>), Changed<MobState>>,
    mut players: Query<(Entity, &Player, &PlayerState, Option<&mut InterpBuffer>), (Changed<PlayerState>, Without<MobState>)>,
    projectiles: Query<Entity, (Added<Projectile>, Without<Transform>)>,
) {
    for (entity, state, buffer) in &mut mobs {
        let snap = Snapshot { tick: state.tick, pos: state.pos, yaw: state.yaw, pitch: 0.0 };
        match buffer {
            Some(mut b) => b.push(snap),
            None => {
                let mut b = InterpBuffer::default();
                b.push(snap);
                commands.entity(entity).try_insert((b, Transform::from_translation(state.pos), Visibility::default()));
            }
        }
    }
    for (entity, player, state, buffer) in &mut players {
        let snap = Snapshot { tick: state.tick, pos: state.sim.mv.pos, yaw: state.yaw, pitch: state.pitch };
        let is_local = local.slot == Some(player.slot);
        match buffer {
            Some(mut b) => b.push(snap),
            None => {
                let mut b = InterpBuffer::default();
                b.push(snap);
                let mut e = commands.entity(entity);
                e.insert((b, Transform::from_translation(state.sim.mv.pos), Visibility::default()));
                if is_local {
                    e.insert(LocalAvatar);
                }
            }
        }
    }
    for entity in &projectiles {
        commands.entity(entity).try_insert((Transform::default(), Visibility::default()));
    }
}

/// Marks the entity of our own player (rendered from prediction, not interpolation).
#[derive(Component)]
pub struct LocalAvatar;

/// Remote players and monsters: interpolated. Projectiles: computed from their trajectory.
fn apply_interpolation(
    clock: Res<ServerClock>,
    local: Res<LocalPlayer>,
    mut q: Query<(&InterpBuffer, &mut Transform), Without<LocalAvatar>>,
    mut projectiles: Query<(&Projectile, &mut Transform), Without<InterpBuffer>>,
) {
    let tick = clock.render_tick();
    for (buffer, mut transform) in &mut q {
        if let Some(s) = buffer.sample(tick) {
            transform.translation = s.pos;
            transform.rotation = Quat::from_rotation_y(s.yaw);
        }
    }
    for (proj, mut transform) in &mut projectiles {
        // Our own shots live "in the present" (they leave the weapon right
        // away); the others are shown at the interpolated time, like monsters.
        let t = if Some(proj.owner_slot) == local.slot { clock.estimate } else { tick };
        let t = t.max(f64::from(proj.spawn_tick));
        let p = proj.pos_at(t);
        let next = proj.pos_at(t + 1.0);
        transform.translation = p;
        if let Some(dir) = (next - p).try_normalize() {
            transform.look_to(dir, Vec3::Y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(tick: u32, x: f32) -> Snapshot {
        Snapshot { tick, pos: Vec3::new(x, 0.0, 0.0), yaw: 0.0, pitch: 0.0 }
    }

    #[test]
    fn interpolates_between_snapshots() {
        let mut b = InterpBuffer::default();
        b.push(snap(10, 0.0));
        b.push(snap(20, 10.0));
        assert!((b.sample(15.0).unwrap().pos.x - 5.0).abs() < 1e-4);
        assert_eq!(b.sample(5.0).unwrap().pos.x, 0.0);
        assert_eq!(b.sample(25.0).unwrap().pos.x, 10.0);
    }

    #[test]
    fn ignores_out_of_order() {
        let mut b = InterpBuffer::default();
        b.push(snap(10, 0.0));
        b.push(snap(8, 99.0));
        assert_eq!(b.latest().unwrap().tick, 10);
    }

    #[test]
    fn angle_lerp_takes_short_path() {
        let a = 3.0;
        let b = -3.0; // close to a across the ±π boundary
        let mid = lerp_angle(a, b, 0.5);
        assert!((mid.abs() - std::f32::consts::PI).abs() < 0.2, "{mid}");
    }
}
