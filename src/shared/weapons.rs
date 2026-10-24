//! The aimed main weapon: a deterministic per-tick simulation shared by the
//! server and the client (prediction). No ammo, no reload (Megabonk-like):
//! hold the trigger to fire at the weapon's rate. Each shot is a *volley*
//! of `count` projectiles (fan or random cone).
//!
//! What the projectiles then do (fly, bounce, chain, explode) is resolved by
//! the server (`game::combat`) using `build::MainParams`.

use bevy::math::{Quat, Vec3};
use serde::{Deserialize, Serialize};

use super::build::{MainParams, MainWeapon};
use super::movement::{buttons, PlayerInput};
use super::rng::{hash_combine, Rng};

/// Timers of the main weapon.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Gun {
    pub cooldown: f32,
}

/// One projectile / beam / chain of a volley.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub weapon: MainWeapon,
    pub origin: Vec3,
    pub dir: Vec3,
    /// Deterministic per-shot randomness (crits...) shared with the client.
    pub seed: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeaponEvents {
    pub shots: Vec<Shot>,
    /// A volley was fired this tick (sounds, recoil).
    pub fired: Option<MainWeapon>,
}

/// Spreads a direction randomly inside a cone of half-angle `spread`.
pub fn spread_dir(dir: Vec3, spread: f32, rng: &mut Rng) -> Vec3 {
    if spread <= 0.0 {
        return dir;
    }
    let angle = rng.range(0.0, std::f32::consts::TAU);
    let radius = spread * rng.next_f32().sqrt();
    let up = if dir.y.abs() > 0.99 { Vec3::X } else { Vec3::Y };
    let right = dir.cross(up).normalize();
    let up = right.cross(dir).normalize();
    let tilt = right * angle.cos() * radius + up * angle.sin() * radius;
    (dir + tilt).normalize()
}

/// Directions of a volley of `count` projectiles around `aim`.
pub fn volley_dirs(aim: Vec3, p: &MainParams, seed: u64) -> Vec<Vec3> {
    let mut rng = Rng::new(seed);
    let spread = p.spread_deg.to_radians();
    if p.random_spread {
        return (0..p.count).map(|_| spread_dir(aim, spread, &mut rng)).collect();
    }
    // Even fan, horizontally around the aim direction.
    let n = p.count.max(1) as f32;
    (0..p.count.max(1))
        .map(|i| {
            let offset = (i as f32 - (n - 1.0) / 2.0) * spread;
            Quat::from_rotation_y(offset) * aim
        })
        .collect()
}

/// One weapon tick.
pub fn simulate_gun(
    gun: &mut Gun,
    input: &PlayerInput,
    params: &MainParams,
    eye: Vec3,
    can_shoot: bool,
    dt: f32,
) -> WeaponEvents {
    let mut ev = WeaponEvents::default();
    gun.cooldown = (gun.cooldown - dt).max(0.0);
    if can_shoot && input.held(buttons::FIRE) && gun.cooldown <= 0.0 {
        gun.cooldown = params.interval;
        ev.fired = Some(params.kind);
        let seed = hash_combine(&[u64::from(input.seq), params.kind as u64]);
        for (i, dir) in volley_dirs(input.aim_dir(), params, seed).into_iter().enumerate() {
            ev.shots.push(Shot { weapon: params.kind, origin: eye, dir, seed: hash_combine(&[seed, i as u64]) });
        }
    }
    ev
}

/// Rotation helper for view models: yaw/pitch to a quaternion.
pub fn look_rotation(yaw: f32, pitch: f32) -> Quat {
    Quat::from_euler(bevy::math::EulerRot::YXZ, yaw, pitch, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::build::{main_params, WStat, WeaponLevel};
    use crate::shared::items::{Inventory, PlayerStats};

    fn params(kind: MainWeapon, extra_count: f32) -> MainParams {
        let mut up = WeaponLevel::default();
        up.apply(&[(WStat::Count, extra_count)]);
        main_params(kind, &up, &PlayerStats::from_inventory(&Inventory::default()))
    }

    fn fire(seq: u32) -> PlayerInput {
        PlayerInput { seq, buttons: buttons::FIRE, select_slot: 255, ..Default::default() }
    }

    #[test]
    fn holding_fire_shoots_at_the_weapon_rate() {
        let p = params(MainWeapon::FireStaff, 0.0);
        let mut gun = Gun::default();
        let mut volleys = 0;
        for i in 0..120 {
            if simulate_gun(&mut gun, &fire(i), &p, Vec3::ZERO, true, 1.0 / 60.0).fired.is_some() {
                volleys += 1;
            }
        }
        // 2 seconds at ~0.42 s per shot => 4 or 5 volleys.
        assert!((4..=5).contains(&volleys), "{volleys}");
    }

    #[test]
    fn count_upgrades_add_projectiles_in_a_fan() {
        let p = params(MainWeapon::FireStaff, 2.0);
        let mut gun = Gun::default();
        let ev = simulate_gun(&mut gun, &fire(1), &p, Vec3::ZERO, true, 0.016);
        assert_eq!(ev.shots.len(), 3);
        // Symmetric fan: middle shot along the aim.
        let aim = fire(1).aim_dir();
        assert!(ev.shots[1].dir.angle_between(aim) < 1e-4);
        assert!(ev.shots[0].dir.angle_between(aim) > 0.05);
    }

    #[test]
    fn volleys_are_deterministic() {
        let p = params(MainWeapon::Shotgun, 0.0);
        let (mut a, mut b) = (Gun::default(), Gun::default());
        let ea = simulate_gun(&mut a, &fire(7), &p, Vec3::ZERO, true, 0.016);
        let eb = simulate_gun(&mut b, &fire(7), &p, Vec3::ZERO, true, 0.016);
        assert_eq!(ea, eb);
        assert_eq!(ea.shots.len(), 7);
    }

    #[test]
    fn spread_stays_in_cone() {
        let mut rng = Rng::new(3);
        for _ in 0..500 {
            let d = spread_dir(Vec3::NEG_Z, 0.1, &mut rng);
            assert!(d.angle_between(Vec3::NEG_Z) <= 0.1 + 1e-3);
        }
    }
}
