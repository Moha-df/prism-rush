//! Difficulty, money and prices. Pure functions = easy to test and tune.
//!
//! ## Money design (coop)
//! Each player has their **own wallet**, but **every kill pays every player**
//! the full reward (whoever landed the last hit). Why:
//! - no kill-stealing frustration, and support play (reviving, kiting) is not
//!   punished;
//! - unlike a single shared wallet, nobody can "spend the team's money" on a
//!   chest the others wanted: each player decides for themselves;
//! - it scales naturally with the number of players (each one gets items).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChestKind {
    Item,
    Weapon,
}

pub const BASE_ITEM_CHEST_PRICE: f32 = 25.0;
pub const BASE_WEAPON_CHEST_PRICE: f32 = 70.0;

/// Difficulty coefficient: 1.0 at the start, grows linearly with time,
/// faster with more players. (Inspired by Risk of Rain 2.)
pub fn difficulty(elapsed_secs: f32, players: u32) -> f32 {
    let minutes = elapsed_secs / 60.0;
    let player_factor = 1.0 + 0.15 * (players.max(1) - 1) as f32;
    1.0 + minutes * 0.16 * player_factor
}

/// Integer "level" displayed in the HUD and used for monster stats.
pub fn monster_level(difficulty: f32) -> u32 {
    1 + ((difficulty - 1.0) * 3.0).floor().max(0.0) as u32
}

pub fn monster_hp_mult(level: u32) -> f32 {
    1.0 + 0.32 * (level - 1) as f32
}

pub fn monster_damage_mult(level: u32) -> f32 {
    1.0 + 0.18 * (level - 1) as f32
}

/// Chest prices grow a bit faster than income: spending early is rewarded.
pub fn chest_price(kind: ChestKind, difficulty: f32) -> u32 {
    let base = match kind {
        ChestKind::Item => BASE_ITEM_CHEST_PRICE,
        ChestKind::Weapon => BASE_WEAPON_CHEST_PRICE,
    };
    (base * difficulty.powf(1.25)).round() as u32
}

/// Gold paid to *each* player for a kill.
pub fn kill_reward(base_gold: f32, difficulty: f32, gold_mult: f32) -> u32 {
    (base_gold * difficulty * gold_mult).round().max(1.0) as u32
}

/// Spawn budget gained per second by the monster director.
pub fn director_credit_rate(difficulty: f32, players: u32) -> f32 {
    (0.9 + 0.55 * players.max(1) as f32) * difficulty.powf(0.9)
}

/// Max monsters alive at once (keeps performance and readability in check).
pub fn max_alive_monsters(players: u32, difficulty: f32) -> usize {
    let n = 10.0 + 5.0 * players.max(1) as f32 + 4.0 * (difficulty - 1.0);
    (n as usize).min(70)
}

/// Seconds between boss spawns.
pub const BOSS_INTERVAL: f32 = 300.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn difficulty_starts_at_one_and_grows() {
        assert_eq!(difficulty(0.0, 1), 1.0);
        assert!(difficulty(600.0, 1) > difficulty(300.0, 1));
        // More players => harder.
        assert!(difficulty(300.0, 4) > difficulty(300.0, 1));
    }

    #[test]
    fn prices_increase_over_time() {
        let mut last = 0;
        for minute in 0..30 {
            let p = chest_price(ChestKind::Item, difficulty(minute as f32 * 60.0, 2));
            assert!(p >= last);
            last = p;
        }
        assert_eq!(chest_price(ChestKind::Item, 1.0), 25);
        assert!(chest_price(ChestKind::Weapon, 1.0) > chest_price(ChestKind::Item, 1.0));
    }

    #[test]
    fn prices_outpace_income() {
        // Kills needed for a chest should slowly increase with difficulty.
        let kills = |d: f32| chest_price(ChestKind::Item, d) as f32 / kill_reward(5.0, d, 1.0) as f32;
        assert!(kills(4.0) > kills(1.0));
        // ...but stay reasonable (a few dozen kills max).
        assert!(kills(6.0) < 20.0);
    }

    #[test]
    fn monster_level_progression() {
        assert_eq!(monster_level(1.0), 1);
        assert!(monster_level(2.0) > 1);
        assert!(monster_hp_mult(5) > monster_hp_mult(1));
    }

    #[test]
    fn reward_is_never_zero() {
        assert!(kill_reward(0.1, 1.0, 1.0) >= 1);
    }

    #[test]
    fn monster_cap_is_bounded() {
        assert!(max_alive_monsters(4, 50.0) <= 70);
        assert!(max_alive_monsters(1, 1.0) >= 10);
    }
}
