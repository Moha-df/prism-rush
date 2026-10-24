//! Waves, difficulty, money and prices. Pure functions = easy to test and tune.
//!
//! ## Money design (coop)
//! Each player has their **own wallet**, but **every kill pays every player**
//! the full reward (whoever landed the last hit). Why:
//! - no kill-stealing frustration, and support play (reviving, kiting) is not
//!   punished;
//! - unlike a single shared wallet, nobody can "spend the team's money" on a
//!   chest the others wanted: each player decides for themselves;
//! - it scales naturally with the number of players (each one gets items).
//!
//! ## Waves (V2)
//! The run is a sequence of waves. A wave has a *budget* of monsters; once
//! everything is spawned, the last monster must die to end the wave. Then a
//! short break lets players shop, and the next, harder wave begins. Every
//! 5th wave is a boss wave. Chest prices go up with each wave.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChestKind {
    /// Wooden chest: mostly common upgrades.
    Small,
    /// Big golden chest: better rarity odds.
    Large,
    /// Rare glowing chest (also dropped by bosses): epic/legendary only.
    Legendary,
    /// Steel crate: offers weapon upgrades (a free level-up).
    Weapon,
}

impl ChestKind {
    pub fn base_price(self) -> f32 {
        match self {
            ChestKind::Small => 25.0,
            ChestKind::Large => 60.0,
            ChestKind::Legendary => 160.0,
            ChestKind::Weapon => 55.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ChestKind::Small => "petit coffre",
            ChestKind::Large => "grand coffre",
            ChestKind::Legendary => "coffre légendaire",
            ChestKind::Weapon => "caisse d'arsenal",
        }
    }
}

/// Break between two waves, in seconds (time to shop and regroup).
pub const WAVE_BREAK: f32 = 12.0;
/// A monster farther than this from every player is brought back near them.
pub const STRAGGLER_DISTANCE: f32 = 70.0;
/// Once a wave is fully spawned, monsters still alive after this many
/// seconds are brought back near the players (no endless hunt for the last one).
pub const STRAGGLER_TIME: f32 = 30.0;

/// Difficulty coefficient of a wave: 1.0 for wave 1, faster with more players.
pub fn wave_difficulty(wave: u32, players: u32) -> f32 {
    let player_factor = 1.0 + 0.15 * (players.max(1) - 1) as f32;
    1.0 + (wave.max(1) - 1) as f32 * 0.2 * player_factor
}

/// Monster level = wave number (displayed in the HUD, drives monster stats).
pub fn monster_level(wave: u32) -> u32 {
    wave.max(1)
}

pub fn monster_hp_mult(level: u32) -> f32 {
    1.0 + 0.28 * (level.max(1) - 1) as f32
}

pub fn monster_damage_mult(level: u32) -> f32 {
    1.0 + 0.15 * (level.max(1) - 1) as f32
}

/// Total spawn budget of a wave (sum of monster costs).
pub fn wave_budget(wave: u32, players: u32) -> f32 {
    let w = wave.max(1) as f32;
    (6.0 + 4.0 * w + 0.35 * w * w) * (1.0 + 0.55 * (players.max(1) - 1) as f32)
}

pub fn is_boss_wave(wave: u32) -> bool {
    wave > 0 && wave.is_multiple_of(5)
}

/// Price multiplier of chests for a wave: +25% per wave, slightly accelerating.
pub fn wave_price_mult(wave: u32) -> f32 {
    let w = (wave.max(1) - 1) as f32;
    1.0 + 0.25 * w + 0.015 * w * w
}

/// Chest prices go up with every wave.
pub fn chest_price(kind: ChestKind, wave: u32) -> u32 {
    (kind.base_price() * wave_price_mult(wave)).round() as u32
}

/// Silver earned at the end of a run (spent in the lobby to unlock
/// weapons): surviving long matters most.
pub fn silver_reward(duration: f32, kills: u32, team_level: u32) -> u32 {
    (duration / 12.0) as u32 + kills / 15 + team_level * 3
}

/// Gold paid to *each* player for a kill.
pub fn kill_reward(base_gold: f32, difficulty: f32, gold_mult: f32) -> u32 {
    (base_gold * difficulty * gold_mult).round().max(1.0) as u32
}

/// Max monsters alive at once (keeps performance and readability in check).
pub fn max_alive_monsters(players: u32, wave: u32) -> usize {
    let n = 7.0 + 4.0 * players.max(1) as f32 + 1.5 * wave as f32;
    (n as usize).min(60)
}

/// Seconds between two spawn packs during a wave.
pub fn spawn_interval(wave: u32) -> f32 {
    (3.0 - 0.12 * wave as f32).max(0.9)
}

// ---------------------------------------------------------------------------
// V3: continuous time (Megabonk-like). No waves, no end: it gets harder until
// it is impossible.
// ---------------------------------------------------------------------------

/// The Final Swarm starts: white ghosts join the fight.
pub const FINAL_SWARM_TIME: f32 = 15.0 * 60.0;
/// Black ghosts (faster, tougher) join too.
pub const BLACK_GHOSTS_TIME: f32 = 20.0 * 60.0;
/// Bosses arrive at these times (seconds).
pub const BOSS_TIMES: [f32; 3] = [4.0 * 60.0, 8.0 * 60.0, 12.0 * 60.0];

/// Difficulty coefficient: grows with time, faster with more players.
pub fn time_difficulty(elapsed: f32, players: u32) -> f32 {
    let minutes = elapsed / 60.0;
    let player_factor = 1.0 + 0.15 * (players.max(1) - 1) as f32;
    1.0 + minutes * 0.2 * player_factor
}

/// Monster level (drives HP and damage): +1 every 45 seconds.
pub fn time_monster_level(elapsed: f32) -> u32 {
    1 + (elapsed / 45.0) as u32
}

/// Spawn budget gained per second.
pub fn spawn_rate(elapsed: f32, players: u32) -> f32 {
    // Megabonk-like hordes: automatic weapons need crowds to shine.
    (0.6 + 0.4 * players.max(1) as f32) * (1.0 + elapsed / 60.0 * 0.6)
}

/// Max monsters alive at once (performance and network cap).
pub fn max_alive_time(elapsed: f32, players: u32) -> usize {
    let n = 14.0 + 6.0 * players.max(1) as f32 + elapsed / 60.0 * 8.0;
    (n as usize).min(160)
}

/// Strength multiplier of the Final Swarm ghosts: +12% per minute, forever.
/// (Exponential: after a while nothing can survive. That is the point.)
pub fn swarm_mult(elapsed: f32) -> f32 {
    let minutes = ((elapsed - FINAL_SWARM_TIME) / 60.0).max(0.0);
    1.12f32.powf(minutes)
}

/// Ghosts spawned per second during the Final Swarm.
pub fn ghost_rate(elapsed: f32, players: u32) -> f32 {
    if elapsed < FINAL_SWARM_TIME {
        return 0.0;
    }
    (1.0 + 0.4 * players.max(1) as f32) * swarm_mult(elapsed).sqrt()
}

/// XP needed to go from `level` to `level + 1` (team level).
pub fn xp_to_next(level: u32) -> f32 {
    let l = level.max(1) as f32;
    8.0 + 6.0 * l + 0.6 * l * l
}

/// Chest price for a player: +14% per chest this player already opened,
/// and a little more as time passes.
pub fn chest_price_v3(kind: ChestKind, opened_by_player: u32, elapsed: f32) -> u32 {
    let base = match kind {
        ChestKind::Small | ChestKind::Weapon => 20.0,
        ChestKind::Large => 45.0,
        ChestKind::Legendary => 120.0,
    };
    (base * 1.14f32.powi(opened_by_player as i32) * (1.0 + elapsed / 60.0 * 0.06)).round() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn difficulty_grows_with_waves_and_players() {
        assert_eq!(wave_difficulty(1, 1), 1.0);
        assert!(wave_difficulty(5, 1) > wave_difficulty(2, 1));
        assert!(wave_difficulty(5, 4) > wave_difficulty(5, 1));
    }

    #[test]
    fn prices_increase_every_wave() {
        let mut last = 0;
        for wave in 1..30 {
            let p = chest_price(ChestKind::Small, wave);
            assert!(p > last, "wave {wave}: {p} <= {last}");
            last = p;
        }
        assert_eq!(chest_price(ChestKind::Small, 1), 25);
        assert!(chest_price(ChestKind::Large, 1) > chest_price(ChestKind::Small, 1));
        assert!(chest_price(ChestKind::Legendary, 1) > chest_price(ChestKind::Large, 1));
    }

    #[test]
    fn a_wave_pays_for_some_chests() {
        // Gold earned in a wave (approx: budget * ~3 gold per cost unit)
        // should buy a couple of small chests, early and late.
        for wave in [1, 5, 10, 20] {
            let gold = wave_budget(wave, 1) * 3.0 * wave_difficulty(wave, 1);
            let chests = gold / chest_price(ChestKind::Small, wave) as f32;
            assert!((1.0..12.0).contains(&chests), "wave {wave}: {chests} chests");
        }
    }

    #[test]
    fn boss_waves() {
        assert!(!is_boss_wave(1));
        assert!(is_boss_wave(5));
        assert!(is_boss_wave(10));
        assert!(!is_boss_wave(0));
    }

    #[test]
    fn budgets_grow() {
        assert!(wave_budget(2, 1) > wave_budget(1, 1));
        assert!(wave_budget(1, 4) > wave_budget(1, 1));
    }

    #[test]
    fn time_progression_v3() {
        assert_eq!(time_difficulty(0.0, 1), 1.0);
        assert!(time_difficulty(600.0, 1) > time_difficulty(60.0, 1));
        assert_eq!(swarm_mult(0.0), 1.0);
        assert_eq!(ghost_rate(FINAL_SWARM_TIME - 1.0, 2), 0.0);
        assert!(ghost_rate(FINAL_SWARM_TIME + 600.0, 2) > ghost_rate(FINAL_SWARM_TIME + 60.0, 2));
        // 30 minutes in, ghosts are several times stronger: near impossible.
        assert!(swarm_mult(30.0 * 60.0) > 5.0);
        assert!(xp_to_next(10) > xp_to_next(2));
        assert!(chest_price_v3(ChestKind::Small, 5, 0.0) > chest_price_v3(ChestKind::Small, 0, 0.0));
        assert!(max_alive_time(1e6, 4) <= 160);
    }

    #[test]
    fn reward_is_never_zero() {
        assert!(kill_reward(0.1, 1.0, 1.0) >= 1);
    }

    #[test]
    fn monster_cap_is_bounded() {
        assert!(max_alive_monsters(4, 50) <= 60);
        assert!(max_alive_monsters(1, 1) >= 8);
        assert!(spawn_interval(100) >= 0.9);
    }
}
