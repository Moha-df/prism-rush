//! Weapons: static definitions + a deterministic per-tick simulation
//! (cooldown, magazine, reload, switching, bursts, spin-up). Like movement,
//! this runs on the server *and* on the client (prediction) with the same
//! inputs.
//!
//! V2: a player carries **at most 2 weapons**. Taking a third one drops the
//! weapon in hand on the ground (see `Arsenal::give`).

use bevy::math::{Quat, Vec3};
use serde::{Deserialize, Serialize};

use super::items::{PlayerStats, Rarity};
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
    Revolver,
    BurstRifle,
    Minigun,
    GrenadeLauncher,
    Railgun,
    PlasmaRifle,
}

/// What a projectile weapon fires (`None` = hitscan).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectileType {
    Rocket,
    Grenade,
    Plasma,
}

/// Static description of a weapon.
#[derive(Clone, Copy, Debug)]
pub struct WeaponDef {
    pub name: &'static str,
    pub rarity: Rarity,
    pub damage: f32,
    /// Seconds between two trigger pulls (or between shots for automatics).
    pub interval: f32,
    pub magazine: u32,
    pub reload_time: f32,
    pub pellets: u32,
    /// Spread cone half-angle in radians.
    pub spread: f32,
    /// Hold to fire continuously?
    pub automatic: bool,
    pub range: f32,
    /// Shots per trigger pull, fired `burst_gap` seconds apart.
    pub burst: u32,
    pub burst_gap: f32,
    /// Seconds to reach full fire rate while holding the trigger (0 = none).
    pub spin_up: f32,
    /// Hitscan that goes through every monster on the line.
    pub pierce: bool,
    pub projectile: Option<ProjectileType>,
    pub projectile_speed: f32,
    pub projectile_gravity: f32,
    /// Explosion radius for projectiles.
    pub splash_radius: f32,
    /// Visual recoil strength (camera kick + view model).
    pub recoil: f32,
}

const BASE: WeaponDef = WeaponDef {
    name: "",
    rarity: Rarity::Common,
    damage: 10.0,
    interval: 0.2,
    magazine: 10,
    reload_time: 1.5,
    pellets: 1,
    spread: 0.0,
    automatic: false,
    range: 150.0,
    burst: 1,
    burst_gap: 0.0,
    spin_up: 0.0,
    pierce: false,
    projectile: None,
    projectile_speed: 0.0,
    projectile_gravity: 0.0,
    splash_radius: 0.0,
    recoil: 0.5,
};

impl WeaponKind {
    pub const ALL: [WeaponKind; 12] = [
        WeaponKind::Pistol,
        WeaponKind::Smg,
        WeaponKind::Shotgun,
        WeaponKind::AssaultRifle,
        WeaponKind::Sniper,
        WeaponKind::RocketLauncher,
        WeaponKind::Revolver,
        WeaponKind::BurstRifle,
        WeaponKind::Minigun,
        WeaponKind::GrenadeLauncher,
        WeaponKind::Railgun,
        WeaponKind::PlasmaRifle,
    ];

    pub fn def(self) -> WeaponDef {
        match self {
            WeaponKind::Pistol => WeaponDef {
                name: "Pistolet",
                damage: 22.0,
                interval: 0.16,
                magazine: 12,
                reload_time: 1.0,
                spread: 0.004,
                ..BASE
            },
            WeaponKind::Smg => WeaponDef {
                name: "Mitraillette",
                damage: 11.0,
                interval: 0.06,
                magazine: 36,
                reload_time: 1.3,
                spread: 0.03,
                automatic: true,
                range: 90.0,
                recoil: 0.25,
                ..BASE
            },
            WeaponKind::Revolver => WeaponDef {
                name: "Revolver",
                damage: 58.0,
                interval: 0.42,
                magazine: 6,
                reload_time: 1.6,
                spread: 0.002,
                range: 170.0,
                recoil: 1.3,
                ..BASE
            },
            WeaponKind::Shotgun => WeaponDef {
                name: "Fusil à pompe",
                rarity: Rarity::Rare,
                damage: 12.0,
                interval: 0.75,
                magazine: 6,
                reload_time: 1.6,
                pellets: 9,
                spread: 0.09,
                range: 45.0,
                recoil: 1.6,
                ..BASE
            },
            WeaponKind::AssaultRifle => WeaponDef {
                name: "Fusil d'assaut",
                rarity: Rarity::Rare,
                damage: 17.0,
                interval: 0.1,
                magazine: 30,
                reload_time: 1.5,
                spread: 0.012,
                automatic: true,
                range: 160.0,
                recoil: 0.4,
                ..BASE
            },
            WeaponKind::BurstRifle => WeaponDef {
                name: "Fusil à rafale",
                rarity: Rarity::Rare,
                damage: 21.0,
                interval: 0.38,
                magazine: 24,
                reload_time: 1.5,
                spread: 0.006,
                burst: 3,
                burst_gap: 0.06,
                range: 170.0,
                recoil: 0.45,
                ..BASE
            },
            WeaponKind::Sniper => WeaponDef {
                name: "Sniper",
                rarity: Rarity::Epic,
                damage: 140.0,
                interval: 1.0,
                magazine: 5,
                reload_time: 2.0,
                range: 400.0,
                recoil: 2.2,
                ..BASE
            },
            WeaponKind::GrenadeLauncher => WeaponDef {
                name: "Lance-grenades",
                rarity: Rarity::Epic,
                damage: 75.0,
                interval: 0.65,
                magazine: 6,
                reload_time: 2.0,
                range: 120.0,
                projectile: Some(ProjectileType::Grenade),
                projectile_speed: 26.0,
                projectile_gravity: 14.0,
                splash_radius: 4.5,
                recoil: 1.4,
                ..BASE
            },
            WeaponKind::PlasmaRifle => WeaponDef {
                name: "Fusil plasma",
                rarity: Rarity::Epic,
                damage: 26.0,
                interval: 0.12,
                magazine: 30,
                reload_time: 1.6,
                spread: 0.008,
                automatic: true,
                range: 140.0,
                projectile: Some(ProjectileType::Plasma),
                projectile_speed: 48.0,
                splash_radius: 1.8,
                recoil: 0.35,
                ..BASE
            },
            WeaponKind::RocketLauncher => WeaponDef {
                name: "Lance-roquettes",
                rarity: Rarity::Legendary,
                damage: 95.0,
                interval: 0.85,
                magazine: 4,
                reload_time: 2.2,
                range: 200.0,
                projectile: Some(ProjectileType::Rocket),
                projectile_speed: 38.0,
                splash_radius: 5.0,
                recoil: 1.8,
                ..BASE
            },
            WeaponKind::Minigun => WeaponDef {
                name: "Minigun",
                rarity: Rarity::Legendary,
                damage: 10.0,
                interval: 0.035,
                magazine: 140,
                reload_time: 3.0,
                spread: 0.045,
                automatic: true,
                spin_up: 0.9,
                range: 110.0,
                recoil: 0.2,
                ..BASE
            },
            WeaponKind::Railgun => WeaponDef {
                name: "Railgun",
                rarity: Rarity::Legendary,
                damage: 170.0,
                interval: 1.2,
                magazine: 3,
                reload_time: 2.2,
                pierce: true,
                range: 300.0,
                recoil: 2.4,
                ..BASE
            },
        }
    }

    pub fn rarity(self) -> Rarity {
        self.def().rarity
    }

    /// Picks a weapon according to rarity odds (weapon crates use the
    /// "small chest" odds, but weapons are rarer at the top end).
    pub fn roll(weights: [f32; 4], exclude: &[WeaponKind], rng: &mut Rng) -> WeaponKind {
        let rarity = Rarity::ALL[rng.weighted_index(&weights)];
        let pool: Vec<WeaponKind> =
            Self::ALL.into_iter().filter(|w| w.rarity() == rarity && !exclude.contains(w)).collect();
        if pool.is_empty() {
            let any: Vec<WeaponKind> = Self::ALL.into_iter().filter(|w| !exclude.contains(w)).collect();
            if any.is_empty() { WeaponKind::Pistol } else { *rng.pick(&any) }
        } else {
            *rng.pick(&pool)
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
    /// Shots left in the current burst, and time before the next one.
    pub burst_left: u8,
    pub burst_timer: f32,
    /// Minigun spin, 0..1.
    pub spin: f32,
}

pub const MAX_WEAPONS: usize = 2;
pub const SWITCH_TIME: f32 = 0.25;

impl Default for Arsenal {
    fn default() -> Self {
        Self::starting()
    }
}

/// One fired bullet/pellet/projectile.
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
    /// A shot or volley happened this tick (for sounds/recoil).
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
            burst_left: 0,
            burst_timer: 0.0,
            spin: 0.0,
        }
    }

    pub fn current_kind(&self) -> WeaponKind {
        self.weapons[self.current].kind
    }

    pub fn has(&self, kind: WeaponKind) -> bool {
        self.weapons.iter().any(|w| w.kind == kind)
    }

    /// Takes a weapon and switches to it. If already owned, it is refilled.
    /// With both slots full, the weapon in hand is replaced and **returned**
    /// so the caller can drop it on the ground.
    pub fn give(&mut self, kind: WeaponKind, stats: &PlayerStats) -> Option<WeaponKind> {
        let fresh = WeaponState { kind, ammo: magazine_size(kind, stats) };
        if let Some(i) = self.weapons.iter().position(|w| w.kind == kind) {
            self.weapons[i] = fresh;
            self.select(i);
            return None;
        }
        if self.weapons.len() < MAX_WEAPONS {
            self.weapons.push(fresh);
            self.select(self.weapons.len() - 1);
            return None;
        }
        let dropped = self.weapons[self.current].kind;
        self.weapons[self.current] = fresh;
        self.reset_timers();
        Some(dropped)
    }

    fn reset_timers(&mut self) {
        self.reload_left = 0.0;
        self.burst_left = 0;
        self.spin = 0.0;
        self.switch_delay = SWITCH_TIME;
    }

    fn select(&mut self, index: usize) {
        if index < self.weapons.len() && index != self.current {
            self.current = index;
            self.reset_timers();
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

/// Fires one shot (all its pellets) of the current weapon.
fn fire(a: &mut Arsenal, input: &PlayerInput, stats: &PlayerStats, eye: Vec3, burst_index: u32, ev: &mut WeaponEvents) {
    let kind = a.current_kind();
    let def = kind.def();
    let ammo = &mut a.weapons[a.current].ammo;
    if *ammo == 0 {
        ev.dry_fire = true;
        a.burst_left = 0;
        return;
    }
    *ammo -= 1;
    ev.fired = Some(kind);
    let aim = input.aim_dir();
    for pellet in 0..def.pellets {
        let seed = hash_combine(&[u64::from(input.seq), u64::from(pellet), kind as u64, u64::from(burst_index)]);
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
    } else if (pressed(buttons::NEXT_WEAPON) || pressed(buttons::PREV_WEAPON)) && a.weapons.len() > 1 {
        a.select((a.current + 1) % a.weapons.len());
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

    // Magazine might have shrunk/grown with items: never above max.
    let ammo = &mut a.weapons[a.current].ammo;
    *ammo = (*ammo).min(mag);

    // --- Manual reload.
    if pressed(buttons::RELOAD) && *ammo < mag && a.reload_left <= 0.0 {
        a.reload_left = def.reload_time;
        a.burst_left = 0;
        ev.reload_started = true;
    }

    let ready = can_shoot && a.switch_delay <= 0.0 && a.reload_left <= 0.0;

    // --- Spin-up (minigun): holding the trigger spins the barrels.
    if def.spin_up > 0.0 {
        if ready && input.held(buttons::FIRE) {
            a.spin = (a.spin + dt / def.spin_up).min(1.0);
        } else {
            a.spin = (a.spin - 2.0 * dt / def.spin_up).max(0.0);
        }
    }

    // --- Remaining shots of a burst.
    if a.burst_left > 0 && ready {
        a.burst_timer -= dt;
        if a.burst_timer <= 0.0 {
            let index = u32::from(def.burst as u8 - a.burst_left);
            a.burst_left -= 1;
            a.burst_timer = def.burst_gap;
            fire(a, input, stats, eye, index, &mut ev);
        }
    }

    // --- New trigger pull.
    let trigger = if def.automatic { input.held(buttons::FIRE) } else { pressed(buttons::FIRE) };
    if ready && trigger && a.cooldown <= 0.0 && a.burst_left == 0 {
        let rate = if def.spin_up > 0.0 { 0.25 + 0.75 * a.spin } else { 1.0 };
        a.cooldown = def.interval / (stats.fire_rate_mult * rate);
        fire(a, input, stats, eye, 0, &mut ev);
        if def.burst > 1 && ev.fired.is_some() {
            a.burst_left = (def.burst - 1) as u8;
            a.burst_timer = def.burst_gap;
        }
    }

    // --- Auto reload on empty magazine.
    if a.weapons[a.current].ammo == 0 && a.reload_left <= 0.0 && a.cooldown <= 0.0 {
        a.reload_left = def.reload_time;
        a.burst_left = 0;
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

    fn idle(seq: u32) -> PlayerInput {
        PlayerInput { seq, select_slot: 255, ..Default::default() }
    }

    fn ready_with(kind: WeaponKind) -> Arsenal {
        let mut a = Arsenal::starting();
        a.give(kind, &stats());
        a.switch_delay = 0.0;
        a
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
            let input = if i % 2 == 0 { fire_input(i) } else { idle(i) };
            let prev = if i % 2 == 0 { 0 } else { buttons::FIRE };
            shots += simulate_weapons(&mut a, &input, prev, &s, Vec3::ZERO, true, 0.2).shots.len();
        }
        assert_eq!(shots as u32, mag);
        assert!(a.is_reloading());
        let mut finished = false;
        for i in 0..200 {
            finished |= simulate_weapons(&mut a, &idle(i), 0, &s, Vec3::ZERO, true, 0.016).reload_finished;
        }
        assert!(finished);
        assert_eq!(a.weapons[0].ammo, mag);
    }

    #[test]
    fn shotgun_fires_pellets_deterministically() {
        let s = stats();
        let mut a = ready_with(WeaponKind::Shotgun);
        let mut b = a.clone();
        let ea = simulate_weapons(&mut a, &fire_input(7), 0, &s, Vec3::ZERO, true, 0.016);
        let eb = simulate_weapons(&mut b, &fire_input(7), 0, &s, Vec3::ZERO, true, 0.016);
        assert_eq!(ea.shots.len(), 9);
        assert_eq!(ea, eb);
        assert_ne!(ea.shots[0].dir, ea.shots[1].dir);
    }

    #[test]
    fn two_slots_max_and_swap_returns_dropped_weapon() {
        let s = stats();
        let mut a = Arsenal::starting();
        assert_eq!(a.give(WeaponKind::Shotgun, &s), None);
        assert_eq!(a.weapons.len(), 2);
        assert_eq!(a.current_kind(), WeaponKind::Shotgun);
        // Third weapon replaces the one in hand.
        assert_eq!(a.give(WeaponKind::Railgun, &s), Some(WeaponKind::Shotgun));
        assert_eq!(a.weapons.len(), 2);
        assert!(a.has(WeaponKind::Pistol) && a.has(WeaponKind::Railgun));
        // Owned weapon: refilled, nothing dropped.
        assert_eq!(a.give(WeaponKind::Pistol, &s), None);
        assert_eq!(a.current_kind(), WeaponKind::Pistol);
    }

    #[test]
    fn burst_rifle_fires_three_shots_per_pull() {
        let s = stats();
        let mut a = ready_with(WeaponKind::BurstRifle);
        let mut shots = simulate_weapons(&mut a, &fire_input(1), 0, &s, Vec3::ZERO, true, 0.016).shots.len();
        for i in 2..20 {
            shots += simulate_weapons(&mut a, &idle(i), buttons::FIRE, &s, Vec3::ZERO, true, 0.016).shots.len();
        }
        assert_eq!(shots, 3);
    }

    #[test]
    fn minigun_spins_up() {
        let s = stats();
        let mut a = ready_with(WeaponKind::Minigun);
        let count = |a: &mut Arsenal, from: u32| {
            (from..from + 30)
                .map(|i| simulate_weapons(a, &fire_input(i), buttons::FIRE, &s, Vec3::ZERO, true, 1.0 / 60.0).shots.len())
                .sum::<usize>()
        };
        let first_half_second = count(&mut a, 0);
        let later = count(&mut a, 100);
        assert!(later > first_half_second, "{later} <= {first_half_second}");
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
    fn weapon_roll_respects_exclusions_and_rarity() {
        let mut rng = Rng::new(4);
        for _ in 0..300 {
            let w = WeaponKind::roll([0.0, 0.0, 0.0, 1.0], &[WeaponKind::Railgun], &mut rng);
            assert_eq!(w.rarity(), Rarity::Legendary);
            assert_ne!(w, WeaponKind::Railgun);
        }
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
