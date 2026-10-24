//! Launch configuration + small persistent settings file.
//!
//! Settings are stored as plain `key=value` lines in `settings.txt` next to
//! where the game is launched: easy to read and edit by hand.

use bevy::prelude::*;

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
                _ => {}
            }
        }
        cfg
    }

    pub fn save(&self) {
        let text = format!(
            "name={}\nlast_address={}\nsensitivity={}\nfov={}\nvolume={}\n",
            self.name, self.last_address, self.sensitivity, self.fov, self.volume
        );
        if let Err(e) = std::fs::write(SETTINGS_FILE, text) {
            warn!("could not save settings: {e}");
        }
    }
}
