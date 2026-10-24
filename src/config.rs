//! Launch configuration + small persistent settings file.
//!
//! Settings are stored as plain `key=value` lines in `settings.txt` next to
//! where the game is launched: easy to read and edit by hand.

use bevy::prelude::*;

use crate::shared::build::{AutoWeapon, MainWeapon};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchMode {
    /// Main menu, the player chooses.
    Menu,
    /// Listen server: plays and hosts.
    Host,
    /// Joins `last_address` right away.
    Join,
    /// Server without a local player.
    Dedicated,
    /// Headless client driven by a bot.
    BotClient,
}

#[derive(Resource, Clone, Debug)]
pub struct AppConfig {
    pub launch: LaunchMode,
    pub name: String,
    pub last_address: String,
    pub port: u16,
    pub sensitivity: f32,
    pub fov: f32,
    pub volume: f32,
    pub bots: u8,
    pub autostart: bool,
    pub headless: bool,
    pub duration: Option<f32>,
    pub seed: Option<u64>,
    /// Debug: save screenshots to this folder at fixed times, then quit.
    pub shots: Option<String>,
    /// Debug: the local player is driven by the bot brain.
    pub autopilot: bool,
    /// Debug: runs start at this time (seconds), to test late game.
    pub start_time: f32,
    // --- Progression kept between runs (Megabonk-like meta) ---
    /// Starting main weapon picked in the lobby.
    pub main_weapon: MainWeapon,
    pub unlocked_mains: Vec<MainWeapon>,
    pub unlocked_autos: Vec<AutoWeapon>,
    /// Silver earned in runs, spent in the menu to unlock weapons.
    pub silver: u32,
}

/// Indices <-> list, for the settings file ("0,3,5").
fn parse_list<T: Copy>(value: &str, all: &[T]) -> Vec<T> {
    value.split(',').filter_map(|v| v.trim().parse::<usize>().ok()).filter_map(|i| all.get(i).copied()).collect()
}

fn format_list(indices: impl Iterator<Item = usize>) -> String {
    indices.map(|i| i.to_string()).collect::<Vec<_>>().join(",")
}

impl AppConfig {
    pub fn is_unlocked_main(&self, w: MainWeapon) -> bool {
        w.def().starter || self.unlocked_mains.contains(&w)
    }

    pub fn is_unlocked_auto(&self, a: AutoWeapon) -> bool {
        a.def().starter || self.unlocked_autos.contains(&a)
    }

    /// Automatic weapons that can be offered in a run.
    pub fn auto_pool(&self) -> Vec<AutoWeapon> {
        AutoWeapon::ALL.into_iter().filter(|a| self.is_unlocked_auto(*a)).collect()
    }
}

const SETTINGS_FILE: &str = "settings.txt";

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            launch: LaunchMode::Menu,
            name: "Joueur".into(),
            last_address: "127.0.0.1".into(),
            port: crate::net::protocol::DEFAULT_PORT,
            sensitivity: 1.0,
            fov: 95.0,
            volume: 0.6,
            bots: 0,
            autostart: false,
            headless: false,
            duration: None,
            seed: None,
            shots: None,
            autopilot: false,
            start_time: 0.0,
            main_weapon: MainWeapon::FireStaff,
            unlocked_mains: Vec::new(),
            unlocked_autos: Vec::new(),
            silver: 0,
        }
    }
}

impl AppConfig {
    /// Loads saved settings; missing file or bad lines fall back to defaults.
    pub fn load() -> Self {
        let mut cfg = Self::default();
        let Ok(text) = std::fs::read_to_string(SETTINGS_FILE) else {
            return cfg;
        };
        for line in text.lines() {
            // `split_once` returns an Option: `let Some(..) else` skips bad lines.
            let Some((key, value)) = line.split_once('=') else { continue };
            let value = value.trim();
            match key.trim() {
                "name" if !value.is_empty() => cfg.name = value.chars().take(16).collect(),
                "last_address" => cfg.last_address = value.to_string(),
                "sensitivity" => cfg.sensitivity = value.parse().unwrap_or(cfg.sensitivity),
                "fov" => cfg.fov = value.parse().unwrap_or(cfg.fov),
                "volume" => cfg.volume = value.parse().unwrap_or(cfg.volume),
                "silver" => cfg.silver = value.parse().unwrap_or(0),
                "unlocked_mains" => cfg.unlocked_mains = parse_list(value, &MainWeapon::ALL),
                "unlocked_autos" => cfg.unlocked_autos = parse_list(value, &AutoWeapon::ALL),
                "main_weapon" => {
                    if let Some(w) = parse_list(value, &MainWeapon::ALL).first() {
                        cfg.main_weapon = *w;
                    }
                }
                _ => {}
            }
        }
        if !cfg.is_unlocked_main(cfg.main_weapon) {
            cfg.main_weapon = MainWeapon::FireStaff;
        }
        cfg
    }

    pub fn save(&self) {
        let text = format!(
            "name={}\nlast_address={}\nsensitivity={}\nfov={}\nvolume={}\nsilver={}\nmain_weapon={}\nunlocked_mains={}\nunlocked_autos={}\n",
            self.name,
            self.last_address,
            self.sensitivity,
            self.fov,
            self.volume,
            self.silver,
            self.main_weapon.index(),
            format_list(self.unlocked_mains.iter().map(|w| w.index())),
            format_list(self.unlocked_autos.iter().map(|a| a.index())),
        );
        if let Err(e) = std::fs::write(SETTINGS_FILE, text) {
            warn!("could not save settings: {e}");
        }
    }
}
