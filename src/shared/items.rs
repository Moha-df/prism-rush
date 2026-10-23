//! Stackable items (bonuses) and the stats they produce.
//!
//! An `Inventory` is just a count per item. All gameplay numbers are derived
//! from it by `PlayerStats::from_inventory`, a pure function: no hidden state,
//! and trivial to test.

use serde::{Deserialize, Serialize};

use super::movement::MoveStats;
use super::rng::Rng;

/// All items. `#[repr(u8)]` + explicit order: the discriminant is used as an
/// index into the inventory array.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ItemKind {
    Feather,
    SwiftBoots,
    ExtendedMag,
    QuickHands,
    PowerRounds,
    VitalHeart,
    RegenMoss,
    HairTrigger,
    CritLens,
    ArmorPlate,
    Thruster,
    VampireFang,
    LuckyCoin,
    UnstableRounds,
    SpringCoil,
}

pub const ITEM_COUNT: usize = 15;

impl ItemKind {
    pub const ALL: [ItemKind; ITEM_COUNT] = [
        ItemKind::Feather,
        ItemKind::SwiftBoots,
        ItemKind::ExtendedMag,
        ItemKind::QuickHands,
        ItemKind::PowerRounds,
        ItemKind::VitalHeart,
        ItemKind::RegenMoss,
        ItemKind::HairTrigger,
        ItemKind::CritLens,
        ItemKind::ArmorPlate,
        ItemKind::Thruster,
        ItemKind::VampireFang,
        ItemKind::LuckyCoin,
        ItemKind::UnstableRounds,
        ItemKind::SpringCoil,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            ItemKind::Feather => "Plume",
            ItemKind::SwiftBoots => "Bottes véloces",
            ItemKind::ExtendedMag => "Chargeur étendu",
            ItemKind::QuickHands => "Mains agiles",
            ItemKind::PowerRounds => "Balles lourdes",
            ItemKind::VitalHeart => "Cœur vital",
            ItemKind::RegenMoss => "Mousse régénérante",
            ItemKind::HairTrigger => "Gâchette sensible",
            ItemKind::CritLens => "Lentille critique",
            ItemKind::ArmorPlate => "Plaque d'armure",
            ItemKind::Thruster => "Propulseur",
            ItemKind::VampireFang => "Croc vampire",
            ItemKind::LuckyCoin => "Pièce fétiche",
            ItemKind::UnstableRounds => "Obus instables",
            ItemKind::SpringCoil => "Ressort",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ItemKind::Feather => "+1 saut en l'air",
            ItemKind::SwiftBoots => "+12% vitesse de déplacement",
            ItemKind::ExtendedMag => "+25% munitions par chargeur",
            ItemKind::QuickHands => "+20% vitesse de rechargement",
            ItemKind::PowerRounds => "+15% dégâts",
            ItemKind::VitalHeart => "+25 PV max",
            ItemKind::RegenMoss => "+1.5 PV/s de régénération",
            ItemKind::HairTrigger => "+12% cadence de tir",
            ItemKind::CritLens => "+8% chance de critique (x2)",
            ItemKind::ArmorPlate => "+15 armure (réduit les dégâts subis)",
            ItemKind::Thruster => "+1 charge de dash",
            ItemKind::VampireFang => "Soigne 3% des dégâts infligés",
            ItemKind::LuckyCoin => "+15% or gagné",
            ItemKind::UnstableRounds => "10% de chance d'explosion à l'impact",
            ItemKind::SpringCoil => "+12% hauteur de saut",
        }
    }

    /// RGB color used by the HUD and the item's chest glow.
    pub fn color(self) -> [f32; 3] {
        match self {
            ItemKind::Feather => [0.85, 0.95, 1.0],
            ItemKind::SwiftBoots => [0.3, 0.9, 0.5],
            ItemKind::ExtendedMag => [0.9, 0.75, 0.3],
            ItemKind::QuickHands => [0.95, 0.55, 0.2],
            ItemKind::PowerRounds => [0.9, 0.25, 0.25],
            ItemKind::VitalHeart => [1.0, 0.35, 0.5],
            ItemKind::RegenMoss => [0.45, 0.85, 0.3],
            ItemKind::HairTrigger => [1.0, 0.85, 0.2],
            ItemKind::CritLens => [0.95, 0.3, 0.9],
            ItemKind::ArmorPlate => [0.6, 0.65, 0.75],
            ItemKind::Thruster => [0.3, 0.7, 1.0],
            ItemKind::VampireFang => [0.7, 0.1, 0.2],
            ItemKind::LuckyCoin => [1.0, 0.8, 0.1],
            ItemKind::UnstableRounds => [1.0, 0.5, 0.1],
            ItemKind::SpringCoil => [0.5, 1.0, 0.9],
        }
    }

    /// Relative drop weight from item chests.
    pub fn weight(self) -> f32 {
        match self {
            ItemKind::Feather | ItemKind::Thruster | ItemKind::UnstableRounds => 0.7,
            ItemKind::VampireFang | ItemKind::CritLens => 0.85,
            _ => 1.0,
        }
    }

    /// Picks a random item according to weights.
    pub fn roll(rng: &mut Rng) -> ItemKind {
        let weights: Vec<f32> = Self::ALL.iter().map(|i| i.weight()).collect();
        Self::ALL[rng.weighted_index(&weights)]
    }
}

/// Count of each item owned by a player.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    pub counts: [u8; ITEM_COUNT],
}

impl Inventory {
    pub fn count(&self, item: ItemKind) -> u32 {
        u32::from(self.counts[item.index()])
    }

    pub fn add(&mut self, item: ItemKind) {
        let c = &mut self.counts[item.index()];
        *c = c.saturating_add(1);
    }

    pub fn total(&self) -> u32 {
        self.counts.iter().map(|&c| u32::from(c)).sum()
    }
}

/// Every gameplay number influenced by items. Computed, never stored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerStats {
    pub movement: MoveStats,
    pub max_hp: f32,
    pub regen: f32,
    pub armor: f32,
    pub damage_mult: f32,
    pub fire_rate_mult: f32,
    pub reload_speed_mult: f32,
    pub mag_mult: f32,
    pub crit_chance: f32,
    pub lifesteal: f32,
    pub gold_mult: f32,
    pub explode_chance: f32,
}

pub const BASE_MAX_HP: f32 = 100.0;
pub const BASE_REGEN: f32 = 1.0;

impl PlayerStats {
    pub fn from_inventory(inv: &Inventory) -> Self {
        let n = |i: ItemKind| inv.count(i) as f32;
        Self {
            movement: MoveStats {
                speed_mult: 1.0 + 0.12 * n(ItemKind::SwiftBoots),
                air_jumps: 1 + inv.counts[ItemKind::Feather.index()],
                dash_charges: 1 + inv.counts[ItemKind::Thruster.index()],
                jump_mult: 1.0 + 0.12 * n(ItemKind::SpringCoil),
            },
            max_hp: BASE_MAX_HP + 25.0 * n(ItemKind::VitalHeart),
            regen: BASE_REGEN + 1.5 * n(ItemKind::RegenMoss),
            armor: 15.0 * n(ItemKind::ArmorPlate),
            damage_mult: 1.0 + 0.15 * n(ItemKind::PowerRounds),
            fire_rate_mult: 1.0 + 0.12 * n(ItemKind::HairTrigger),
            reload_speed_mult: 1.0 + 0.2 * n(ItemKind::QuickHands),
            mag_mult: 1.0 + 0.25 * n(ItemKind::ExtendedMag),
            // Diminishing returns so it never reaches 100%.
            crit_chance: 1.0 - 0.92f32.powf(n(ItemKind::CritLens)),
            lifesteal: 0.03 * n(ItemKind::VampireFang),
            gold_mult: 1.0 + 0.15 * n(ItemKind::LuckyCoin),
            explode_chance: 1.0 - 0.9f32.powf(n(ItemKind::UnstableRounds)),
        }
    }

    /// Damage actually taken after armor (classic `armor / (armor + 100)` curve).
    pub fn mitigate(&self, damage: f32) -> f32 {
        damage * 100.0 / (100.0 + self.armor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_items_listed_in_order() {
        for (i, item) in ItemKind::ALL.iter().enumerate() {
            assert_eq!(item.index(), i);
            assert!(!item.name().is_empty());
            assert!(!item.description().is_empty());
        }
    }

    #[test]
    fn empty_inventory_gives_base_stats() {
        let s = PlayerStats::from_inventory(&Inventory::default());
        assert_eq!(s.max_hp, BASE_MAX_HP);
        assert_eq!(s.movement, MoveStats::default());
        assert_eq!(s.damage_mult, 1.0);
        assert_eq!(s.crit_chance, 0.0);
        assert_eq!(s.mitigate(10.0), 10.0);
    }

    #[test]
    fn items_stack() {
        let mut inv = Inventory::default();
        inv.add(ItemKind::Feather);
        inv.add(ItemKind::Feather);
        inv.add(ItemKind::SwiftBoots);
        inv.add(ItemKind::VitalHeart);
        let s = PlayerStats::from_inventory(&inv);
        assert_eq!(s.movement.air_jumps, 3);
        assert!((s.movement.speed_mult - 1.12).abs() < 1e-6);
        assert_eq!(s.max_hp, 125.0);
        assert_eq!(inv.total(), 4);
    }

    #[test]
    fn crit_has_diminishing_returns() {
        let mut inv = Inventory::default();
        for _ in 0..100 {
            inv.add(ItemKind::CritLens);
        }
        let s = PlayerStats::from_inventory(&inv);
        assert!(s.crit_chance < 1.0 && s.crit_chance > 0.9);
    }

    #[test]
    fn armor_reduces_damage() {
        let mut inv = Inventory::default();
        inv.add(ItemKind::ArmorPlate);
        let s = PlayerStats::from_inventory(&inv);
        assert!(s.mitigate(100.0) < 100.0 && s.mitigate(100.0) > 80.0);
    }

    #[test]
    fn roll_covers_all_items() {
        let mut rng = Rng::new(1);
        let mut seen = [false; ITEM_COUNT];
        for _ in 0..2000 {
            seen[ItemKind::roll(&mut rng).index()] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn count_saturates() {
        let mut inv = Inventory::default();
        for _ in 0..300 {
            inv.add(ItemKind::LuckyCoin);
        }
        assert_eq!(inv.count(ItemKind::LuckyCoin), 255);
    }
}
