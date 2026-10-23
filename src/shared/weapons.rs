//! Weapons: static definitions + a deterministic per-tick simulation
//! (cooldown, magazine, reload, switching). Like movement, this runs on the
//! server *and* on the client (prediction) with the same inputs.

use bevy::math::{Quat, Vec3};
use serde::{Deserialize, Serialize};

use super::items::PlayerStats;
use super::movement::{buttons, PlayerInput};
use super::rng::{hash_combine, Rng};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WeaponKind {
    Pistol,
    Smg,
    Shotgun,
    AssaultRifle,
    Sniper,
    RocketLauncher,
}

/// Static description of a weapon.
#[derive(Clone, Copy, Debug)]
pub struct WeaponDef {
    pub name: &'static str,
    pub damage: f32,
    /// Seconds between shots.
    pub interval: f32,
    pub magazine: u32,
    pub reload_time: f32,
    pub pellets: u32,
    /// Spread cone half-angle in radians.
    pub spread: f32,
    /// Hold to fire continuously?
    pub automatic: bool,
    pub range: f32,
    /// `Some(speed)` for projectile weapons (rockets), `None` for hitscan.
    pub projectile_speed: Option<f32>,
    /// Explosion radius for projectiles.
    pub splash_radius: f32,
    /// Visual recoil strength (camera kick + view model).
    pub recoil: f32,
}

impl WeaponKind {
    pub const ALL: [WeaponKind; 6] = [
        WeaponKind::Pistol,
        WeaponKind::Smg,
        WeaponKind::Shotgun,
        WeaponKind::AssaultRifle,
        WeaponKind::Sniper,
        WeaponKind::RocketLauncher,
    ];

    pub fn def(self) -> WeaponDef {
        match self {
            WeaponKind::Pistol => WeaponDef {
                name: "Pistolet",
                damage: 22.0,
                interval: 0.16,
                magazine: 12,
                reload_time: 1.0,
                pellets: 1,
                spread: 0.004,
                automatic: false,
                range: 150.0,
                projectile_speed: None,
                splash_radius: 0.0,
                recoil: 0.5,
            },
            WeaponKind::Smg => WeaponDef {
                name: "Mitraillette",
                damage: 11.0,
                interval: 0.06,
                magazine: 36,
                reload_time: 1.3,
                pellets: 1,
                spread: 0.03,
                automatic: true,
                range: 90.0,
                projectile_speed: None,
                splash_radius: 0.0,
                recoil: 0.25,
            },
            WeaponKind::Shotgun => WeaponDef {
                name: "Fusil à pompe",
                damage: 12.0,
                interval: 0.75,
                magazine: 6,
                reload_time: 1.6,
                pellets: 9,
                spread: 0.09,
                automatic: false,
                range: 45.0,
                projectile_speed: None,
                splash_radius: 0.0,
                recoil: 1.6,
            },
            WeaponKind::AssaultRifle => WeaponDef {
                name: "Fusil d'assaut",
                damage: 17.0,
                interval: 0.1,
                magazine: 30,
                reload_time: 1.5,
                pellets: 1,
                spread: 0.012,
                automatic: true,
                range: 160.0,
                projectile_speed: None,
                splash_radius: 0.0,
                recoil: 0.4,
            },
            WeaponKind::Sniper => WeaponDef {
                name: "Sniper",
                damage: 140.0,
                interval: 1.0,
                magazine: 5,
                reload_time: 2.0,
                pellets: 1,
                spread: 0.0,
                automatic: false,
                range: 400.0,
                projectile_speed: None,
                splash_radius: 0.0,
                recoil: 2.2,
            },
            WeaponKind::RocketLauncher => WeaponDef {
                name: "Lance-roquettes",
                damage: 90.0,
                interval: 0.85,
                magazine: 4,
                reload_time: 2.2,
                pellets: 1,
                spread: 0.0,
                automatic: false,
                range: 200.0,
                projectile_speed: Some(38.0),
                splash_radius: 5.0,
                recoil: 1.8,
            },
        }
    }
}

/// Runtime state of one owned weapon.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WeaponState {
    pub kind: WeaponKind,
    pub ammo: u32,
}

/// The weapons a player owns and the timers of the current one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Arsenal {
    pub weapons: Vec<WeaponState>,
    pub current: usize,
    pub cooldown: f32,
    /// Remaining reload time, 0 when not reloading.
    pub reload_left: f32,
    /// Short delay after switching weapons.
    pub switch_delay: f32,
}

pub const MAX_WEAPONS: usize = 6;
pub const SWITCH_TIME: f32 = 0.25;

impl Default for Arsenal {
    fn default() -> Self {
        Self::starting()
    }
}

/// One fired bullet/pellet/rocket.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub weapon: WeaponKind,
    pub origin: Vec3,
    pub dir: Vec3,
    pub damage: f32,
    /// Deterministic per-shot randomness (crits, explosions) shared with the client.
    pub seed: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeaponEvents {
    pub shots: Vec<Shot>,
    /// A trigger pull happened this tick (one per volley, for sounds/recoil).
    pub fired: Option<WeaponKind>,
    pub reload_started: bool,
    pub reload_finished: bool,
    pub switched: bool,
    pub dry_fire: bool,
}

pub fn magazine_size(kind: WeaponKind, stats: &PlayerStats) -> u32 {
    let base = kind.def().magazine as f32;
    // Always at least +1 per stack so small magazines also benefit.
    let extra = ((stats.mag_mult - 1.0) * base).max((stats.mag_mult - 1.0) * 4.0);
    (base + extra).round() as u32
}

impl Arsenal {
    pub fn starting() -> Self {
        Self {
            weapons: vec![WeaponState { kind: WeaponKind::Pistol, ammo: WeaponKind::Pistol.def().magazine }],
            current: 0,
            cooldown: 0.0,
            reload_left: 0.0,
            switch_delay: 0.0,
        }
    }

    pub fn current_kind(&self) -> WeaponKind {
        self.weapons[self.current].kind
    }

    pub fn has(&self, kind: WeaponKind) -> bool {
        self.weapons.iter().any(|w| w.kind == kind)
    }

    /// Adds a weapon (and switches to it). If already owned, refills it.
    /// Returns `false` if nothing changed (should not happen).
    pub fn give(&mut self, kind: WeaponKind, stats: &PlayerStats) -> bool {
        if let Some(i) = self.weapons.iter().position(|w| w.kind == kind) {
            self.weapons[i].ammo = magazine_size(kind, stats);
            self.select(i);
            return true;
        }
        if self.weapons.len() >= MAX_WEAPONS {
            return false;
        }
        self.weapons.push(WeaponState { kind, ammo: magazine_size(kind, stats) });
        self.select(self.weapons.len() - 1);
        true
    }

    fn select(&mut self, index: usize) {
        if index < self.weapons.len() && index != self.current {
            self.current = index;
            self.reload_left = 0.0;
            self.switch_delay = SWITCH_TIME;
        }
    }

    pub fn is_reloading(&self) -> bool {
        self.reload_left > 0.0
    }
}

/// Spreads a direction randomly inside a cone of half-angle `spread`.
pub fn spread_dir(dir: Vec3, spread: f32, rng: &mut Rng) -> Vec3 {
    if spread <= 0.0 {
        return dir;
    }
    // Uniform-ish disk sampling around the axis.
    let angle = rng.range(0.0, std::f32::consts::TAU);
    let radius = spread * rng.next_f32().sqrt();
    let up = if dir.y.abs() > 0.99 { Vec3::X } else { Vec3::Y };
    let right = dir.cross(up).normalize();
    let up = right.cross(dir).normalize();
    let tilt = right * angle.cos() * radius + up * angle.sin() * radius;
    (dir + tilt).normalize()
}

/// One weapon tick. `prev_buttons` comes from the movement state (edge detection).
pub fn simulate_weapons(
    a: &mut Arsenal,
    input: &PlayerInput,
    prev_buttons: u16,
    stats: &PlayerStats,
    eye: Vec3,
    can_shoot: bool,
    dt: f32,
) -> WeaponEvents {
    let mut ev = WeaponEvents::default();
    let pressed = |b: u16| input.held(b) && prev_buttons & b == 0;

    a.cooldown = (a.cooldown - dt).max(0.0);
    a.switch_delay = (a.switch_delay - dt).max(0.0);

    // --- Switching weapons.
    let before = a.current;
    if (input.select_slot as usize) < a.weapons.len() {
        a.select(input.select_slot as usize);
    } else if pressed(buttons::NEXT_WEAPON) && a.weapons.len() > 1 {
        a.select((a.current + 1) % a.weapons.len());
    } else if pressed(buttons::PREV_WEAPON) && a.weapons.len() > 1 {
        a.select((a.current + a.weapons.len() - 1) % a.weapons.len());
    }
    ev.switched = a.current != before;

    let kind = a.current_kind();
    let def = kind.def();
    let mag = magazine_size(kind, stats);

    // --- Reload progress.
    if a.reload_left > 0.0 {
        a.reload_left -= dt * stats.reload_speed_mult;
        if a.reload_left <= 0.0 {
            a.reload_left = 0.0;
            a.weapons[a.current].ammo = mag;
            ev.reload_finished = true;
        }
    }

    // Magazine might have grown with an item: never above max.
    let ammo = &mut a.weapons[a.current].ammo;
    *ammo = (*ammo).min(mag);

    // --- Manual reload.
    if pressed(buttons::RELOAD) && *ammo < mag && a.reload_left <= 0.0 {
        a.reload_left = def.reload_time;
        ev.reload_started = true;
    }

    // --- Fire.
    let trigger = if def.automatic { input.held(buttons::FIRE) } else { pressed(buttons::FIRE) };
    if can_shoot && trigger && a.cooldown <= 0.0 && a.switch_delay <= 0.0 && a.reload_left <= 0.0 {
        let ammo = &mut a.weapons[a.current].ammo;
        if *ammo == 0 {
            ev.dry_fire = true;
        } else {
            *ammo -= 1;
            a.cooldown = def.interval / stats.fire_rate_mult;
            ev.fired = Some(kind);
            let aim = input.aim_dir();
            for pellet in 0..def.pellets {
                let seed = hash_combine(&[u64::from(input.seq), u64::from(pellet), kind as u64]);
                let mut rng = Rng::new(seed);
                ev.shots.push(Shot {
                    weapon: kind,
                    origin: eye,
                    dir: spread_dir(aim, def.spread, &mut rng),
                    damage: def.damage * stats.damage_mult,
                    seed,
                });
            }
        }
    }

    // --- Auto reload on empty magazine.
    if a.weapons[a.current].ammo == 0 && a.reload_left <= 0.0 && a.cooldown <= 0.0 {
        a.reload_left = def.reload_time;
        ev.reload_started = true;
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
    use crate::shared::items::Inventory;

    fn stats() -> PlayerStats {
        PlayerStats::from_inventory(&Inventory::default())
    }

    fn fire_input(seq: u32) -> PlayerInput {
        PlayerInput { seq, buttons: buttons::FIRE, select_slot: 255, ..Default::default() }
    }

    #[test]
    fn semi_auto_needs_release() {
        let mut a = Arsenal::starting();
        let s = stats();
        let ev = simulate_weapons(&mut a, &fire_input(0), 0, &s, Vec3::ZERO, true, 1.0);
        assert_eq!(ev.shots.len(), 1);
        // Still holding: no new shot even after the cooldown.
        let ev = simulate_weapons(&mut a, &fire_input(1), buttons::FIRE, &s, Vec3::ZERO, true, 1.0);
        assert!(ev.shots.is_empty());
    }

    #[test]
    fn empties_magazine_then_reloads() {
        let mut a = Arsenal::starting();
        let s = stats();
        let mag = magazine_size(WeaponKind::Pistol, &s);
        let mut shots = 0;
        for i in 0..mag * 2 {
            // Alternate press / release.
            let input = if i % 2 == 0 { fire_input(i) } else { PlayerInput { select_slot: 255, ..Default::default() } };
            let prev = if i % 2 == 0 { 0 } else { buttons::FIRE };
            shots += simulate_weapons(&mut a, &input, prev, &s, Vec3::ZERO, true, 0.2).shots.len();
        }
        assert_eq!(shots as u32, mag);
        assert!(a.is_reloading());
        // Wait for the reload.
        let idle = PlayerInput { select_slot: 255, ..Default::default() };
        let mut finished = false;
        for _ in 0..200 {
            finished |= simulate_weapons(&mut a, &idle, 0, &s, Vec3::ZERO, true, 0.016).reload_finished;
        }
        assert!(finished);
        assert_eq!(a.weapons[0].ammo, mag);
    }

    #[test]
    fn shotgun_fires_pellets_deterministically() {
        let s = stats();
        let mut a = Arsenal::starting();
        a.give(WeaponKind::Shotgun, &s);
        a.switch_delay = 0.0;
        let mut b = a.clone();
        let ea = simulate_weapons(&mut a, &fire_input(7), 0, &s, Vec3::ZERO, true, 0.016);
        let eb = simulate_weapons(&mut b, &fire_input(7), 0, &s, Vec3::ZERO, true, 0.016);
        assert_eq!(ea.shots.len(), 9);
        assert_eq!(ea, eb);
        // Pellets actually spread.
        assert_ne!(ea.shots[0].dir, ea.shots[1].dir);
    }

    #[test]
    fn extended_mag_increases_magazine() {
        let mut inv = Inventory::default();
        inv.add(crate::shared::items::ItemKind::ExtendedMag);
        let s = PlayerStats::from_inventory(&inv);
        assert!(magazine_size(WeaponKind::Sniper, &s) > WeaponKind::Sniper.def().magazine);
        assert!(magazine_size(WeaponKind::Smg, &s) > WeaponKind::Smg.def().magazine);
    }

    #[test]
    fn give_switches_and_caps() {
        let s = stats();
        let mut a = Arsenal::starting();
        for k in WeaponKind::ALL {
            a.give(k, &s);
        }
        assert_eq!(a.weapons.len(), 6);
        assert_eq!(a.current_kind(), WeaponKind::RocketLauncher);
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
