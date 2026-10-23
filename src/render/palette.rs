//! The game's color palette, in one place.
//!
//! Rules (see DECISIONS.md, "Direction artistique"):
//! - the world is warm and natural (sage greens, sandstone, lavender rocks);
//! - monsters are dark plum with *glowing* accents (magenta, orange, toxic
//!   green, cyan): they pop out of the background and read instantly;
//! - players are saturated colors with white armor and a glowing visor.

use bevy::prelude::*;

pub const fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color::srgb(r, g, b)
}

// --- Environment.
pub const GRASS_A: [f32; 3] = [0.42, 0.62, 0.36];
pub const GRASS_B: [f32; 3] = [0.58, 0.68, 0.36];
pub const GRASS_TEAL: [f32; 3] = [0.30, 0.56, 0.46];
pub const SAND: [f32; 3] = [0.86, 0.77, 0.58];
pub const SNOW: [f32; 3] = [0.90, 0.91, 0.96];
pub const CLAY: [[f32; 3]; 3] = [[0.76, 0.50, 0.38], [0.66, 0.42, 0.35], [0.82, 0.62, 0.46]];
pub const ROCK: Color = rgb(0.56, 0.53, 0.60);
pub const ROCK_DARK: Color = rgb(0.42, 0.40, 0.48);
pub const SANDSTONE: Color = rgb(0.86, 0.74, 0.58);
pub const SANDSTONE_DARK: Color = rgb(0.70, 0.58, 0.46);
pub const BARK: Color = rgb(0.45, 0.31, 0.24);
pub const LEAF_A: Color = rgb(0.30, 0.58, 0.42);
pub const LEAF_B: Color = rgb(0.45, 0.66, 0.33);
pub const LEAF_AUTUMN: Color = rgb(0.92, 0.55, 0.28);
pub const CRYSTAL_CYAN: Color = rgb(0.45, 0.92, 1.0);
pub const CRYSTAL_PINK: Color = rgb(1.0, 0.45, 0.85);

// --- Sky / fog (dusk mood: lavender zenith, peach horizon).
pub const SKY_ZENITH: [f32; 3] = [0.36, 0.45, 0.76];
pub const SKY_HORIZON: [f32; 3] = [0.98, 0.76, 0.63];
pub const FOG: Color = rgb(0.86, 0.72, 0.70);
pub const SUN: Color = rgb(1.0, 0.86, 0.68);
pub const AMBIENT: Color = rgb(0.55, 0.60, 0.90);

// --- Monsters.
pub const PLUM: Color = rgb(0.29, 0.18, 0.36);
pub const PLUM_DARK: Color = rgb(0.17, 0.11, 0.22);
pub const PLUM_LIGHT: Color = rgb(0.45, 0.30, 0.52);
pub const BONE: Color = rgb(0.88, 0.82, 0.72);
pub const STONE: Color = rgb(0.34, 0.31, 0.38);
pub const STONE_DARK: Color = rgb(0.22, 0.20, 0.26);
pub const GLOW_MAGENTA: Color = rgb(1.0, 0.22, 0.62);
pub const GLOW_ORANGE: Color = rgb(1.0, 0.48, 0.12);
pub const GLOW_TOXIC: Color = rgb(0.55, 1.0, 0.22);
pub const GLOW_CYAN: Color = rgb(0.30, 0.95, 1.0);
pub const BOMB_RED: Color = rgb(0.58, 0.16, 0.13);

// --- Players / weapons / chests.
pub const ARMOR_WHITE: Color = rgb(0.92, 0.92, 0.95);
pub const SUIT_DARK: Color = rgb(0.16, 0.17, 0.22);
pub const METAL_DARK: Color = rgb(0.18, 0.19, 0.24);
pub const METAL_LIGHT: Color = rgb(0.56, 0.59, 0.66);
pub const WOOD: Color = rgb(0.62, 0.38, 0.20);
pub const WOOD_DARK: Color = rgb(0.46, 0.27, 0.15);
pub const GOLD: Color = rgb(0.98, 0.76, 0.28);
pub const STEEL: Color = rgb(0.30, 0.33, 0.40);
pub const TEAL_GLOW: Color = rgb(0.25, 0.95, 0.85);

/// Mixes two RGB triplets.
pub fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}
