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

/// Ground colors of a map (terrain mesh vertex colors).
pub struct Ground {
    /// Lowlands / large biome patches.
    pub low: [f32; 3],
    pub mid: [f32; 3],
    /// Small patches.
    pub patch: [f32; 3],
    /// Deep valleys.
    pub shore: [f32; 3],
    /// Strata of steep cliffs.
    pub cliffs: [[f32; 3]; 3],
    /// Mountain tops.
    pub peak: [f32; 3],
}

pub const GROUND_MEADOW: Ground = Ground { low: GRASS_TEAL, mid: GRASS_A, patch: GRASS_B, shore: SAND, cliffs: CLAY, peak: SNOW };

/// Desert of embers: orange dunes, red rock, dark basalt valleys, ash peaks.
pub const GROUND_EMBERS: Ground = Ground {
    low: [0.80, 0.52, 0.30],
    mid: [0.90, 0.68, 0.42],
    patch: [0.74, 0.40, 0.26],
    shore: [0.26, 0.18, 0.18],
    cliffs: [[0.62, 0.26, 0.18], [0.50, 0.22, 0.17], [0.74, 0.38, 0.24]],
    peak: [0.32, 0.24, 0.24],
};

/// Cursed swamp: dark moss, purple mud, murky valleys, pale lichen.
pub const GROUND_SWAMP: Ground = Ground {
    low: [0.19, 0.25, 0.21],
    mid: [0.27, 0.30, 0.20],
    patch: [0.30, 0.24, 0.33],
    shore: [0.11, 0.19, 0.20],
    cliffs: [[0.30, 0.27, 0.34], [0.24, 0.22, 0.28], [0.37, 0.33, 0.39]],
    peak: [0.56, 0.66, 0.56],
};

pub fn ground(map: crate::shared::rules::MapKind) -> &'static Ground {
    use crate::shared::rules::MapKind;
    match map {
        MapKind::Meadow => &GROUND_MEADOW,
        MapKind::Embers => &GROUND_EMBERS,
        MapKind::Swamp => &GROUND_SWAMP,
    }
}

/// Recolors a prop vertex color (linear RGBA) for a map: the decor models
/// are shared, each map dyes them (dry leaves in the desert, dark teal in
/// the swamp, crystals glowing orange or toxic green...).
pub fn grade(map: crate::shared::rules::MapKind, c: [f32; 4], glow: bool) -> [f32; 4] {
    use crate::shared::rules::MapKind;
    let [r, g, b, a] = c;
    let lum = 0.3 * r + 0.55 * g + 0.15 * b;
    let greenish = g > r * 1.05 && g > b;
    match map {
        MapKind::Meadow => c,
        MapKind::Embers if glow => {
            // Cyan / pink crystals become embers.
            let m = r.max(g).max(b);
            [m, m * 0.4, m * 0.08, a]
        }
        MapKind::Embers if greenish => [lum * 1.35, lum * 0.8, lum * 0.35, a],
        MapKind::Embers => [r * 1.05, g * 0.9, b * 0.8, a],
        MapKind::Swamp if glow => {
            let m = r.max(g).max(b);
            [m * 0.55, m, m * 0.25, a]
        }
        MapKind::Swamp if greenish => [lum * 0.45, lum * 0.85, lum * 0.7, a],
        MapKind::Swamp => [r * 0.55, g * 0.62, b * 0.66, a],
    }
}

/// Mixes two RGB triplets.
pub fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}
