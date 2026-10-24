//! Stackable upgrades ("items"): categories, rarities, the stats they give,
//! and the card draw of chests.
//!
//! An `Inventory` is just a count per item. All gameplay numbers are derived
//! from it by `PlayerStats::from_inventory`, a pure function: no hidden state,
//! trivial to test.

use serde::{Deserialize, Serialize};

use super::build::{tome_points, Tome, TOME_COUNT};
use super::economy::ChestKind;
use super::movement::MoveStats;
use super::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Category {
    Mobility,
    Offense,
    Defense,
    Utility,
}

impl Category {
    pub fn name(self) -> &'static str {
        match self {
            Category::Mobility => "Mobilité",
            Category::Offense => "Offensif",
            Category::Defense => "Défensif",
            Category::Utility => "Utilitaire",
        }
    }

    pub fn color(self) -> [f32; 3] {
        match self {
            Category::Mobility => [0.35, 0.85, 1.0],
            Category::Offense => [1.0, 0.4, 0.3],
            Category::Defense => [0.45, 0.95, 0.5],
            Category::Utility => [1.0, 0.82, 0.3],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Rarity {
    Common,
    /// Level-up upgrades only (no item has this rarity).
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    /// Rarities items can have (chest draws).
    pub const ALL: [Rarity; 4] = [Rarity::Common, Rarity::Rare, Rarity::Epic, Rarity::Legendary];
    /// All 5 tiers (level-up upgrades).
    pub const TIERS: [Rarity; 5] = [Rarity::Common, Rarity::Uncommon, Rarity::Rare, Rarity::Epic, Rarity::Legendary];

    /// Megabonk-style multiplier applied to an upgrade's value.
    pub fn factor(self) -> f32 {
        match self {
            Rarity::Common => 1.0,
            Rarity::Uncommon => 1.2,
            Rarity::Rare => 1.4,
            Rarity::Epic => 1.6,
            Rarity::Legendary => 2.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Rarity::Common => "Commun",
            Rarity::Uncommon => "Peu commun",
            Rarity::Rare => "Rare",
            Rarity::Epic => "Épique",
            Rarity::Legendary => "Légendaire",
        }
    }

    pub fn color(self) -> [f32; 3] {
        match self {
            Rarity::Common => [0.8, 0.82, 0.86],
            Rarity::Uncommon => [0.35, 0.9, 0.4],
            Rarity::Rare => [0.3, 0.65, 1.0],
            Rarity::Epic => [0.75, 0.35, 1.0],
            Rarity::Legendary => [1.0, 0.65, 0.1],
        }
    }

    /// Draw weights of each rarity for a chest kind.
    pub fn weights(chest: ChestKind) -> [f32; 4] {
        match chest {
            ChestKind::Small | ChestKind::Weapon => [70.0, 25.0, 5.0, 0.4],
            ChestKind::Large => [25.0, 45.0, 24.0, 6.0],
            ChestKind::Legendary => [0.0, 0.0, 60.0, 40.0],
        }
    }
}

/// All items. `#[repr(u8)]` + explicit order: the discriminant is used as an
/// index into the inventory array.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ItemKind {
    // Mobility
    FrogLegs,
    Jetpack,
    SwiftBoots,
    Thruster,
    DashCoil,
    SlideWax,
    FeatherCape,
    PhaseStep,
    // Offense
    PowerRounds,
    HairTrigger,
    CritLens,
    Executioner,
    UnstableRounds,
    TeslaCoil,
    HollowPoint,
    Berserker,
    Bloodlust,
    // Defense
    VitalHeart,
    RegenMoss,
    ArmorPlate,
    VampireFangs,
    EnergyShield,
    Thorns,
    Ankh,
    // Utility
    LuckyCoin,
    ExtendedMag,
    QuickHands,
    TreasureMap,
    Clover,
    Medkit,
    // Legendary specials
    Crown,
    DragonHeart,
    GatlingSoul,
}

pub const ITEM_COUNT: usize = 33;

impl ItemKind {
    pub const ALL: [ItemKind; ITEM_COUNT] = [
        ItemKind::FrogLegs,
        ItemKind::Jetpack,
        ItemKind::SwiftBoots,
        ItemKind::Thruster,
        ItemKind::DashCoil,
        ItemKind::SlideWax,
        ItemKind::FeatherCape,
        ItemKind::PhaseStep,
        ItemKind::PowerRounds,
        ItemKind::HairTrigger,
        ItemKind::CritLens,
        ItemKind::Executioner,
        ItemKind::UnstableRounds,
        ItemKind::TeslaCoil,
        ItemKind::HollowPoint,
        ItemKind::Berserker,
        ItemKind::Bloodlust,
        ItemKind::VitalHeart,
        ItemKind::RegenMoss,
        ItemKind::ArmorPlate,
        ItemKind::VampireFangs,
        ItemKind::EnergyShield,
        ItemKind::Thorns,
        ItemKind::Ankh,
        ItemKind::LuckyCoin,
        ItemKind::ExtendedMag,
        ItemKind::QuickHands,
        ItemKind::TreasureMap,
        ItemKind::Clover,
        ItemKind::Medkit,
        ItemKind::Crown,
        ItemKind::DragonHeart,
        ItemKind::GatlingSoul,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        use ItemKind::*;
        match self {
            FrogLegs => "Pattes de grenouille",
            Jetpack => "Jetpack",
            SwiftBoots => "Bottes de course",
            Thruster => "Propulseur",
            DashCoil => "Bobine de dash",
            SlideWax => "Cire de glisse",
            FeatherCape => "Cape plume",
            PhaseStep => "Pas fantôme",
            PowerRounds => "Balles lourdes",
            HairTrigger => "Gâchette sensible",
            CritLens => "Lunette critique",
            Executioner => "Hache du bourreau",
            UnstableRounds => "Obus instables",
            TeslaCoil => "Bobine Tesla",
            HollowPoint => "Pointe creuse",
            Berserker => "Masque du berserker",
            Bloodlust => "Soif de sang",
            VitalHeart => "Cœur vital",
            RegenMoss => "Mousse régénérante",
            ArmorPlate => "Plaque d'armure",
            VampireFangs => "Canines de vampire",
            EnergyShield => "Bouclier d'énergie",
            Thorns => "Ronces",
            Ankh => "Ankh",
            LuckyCoin => "Pièce fétiche",
            ExtendedMag => "Gros calibre",
            QuickHands => "Plumes du vent",
            TreasureMap => "Carte au trésor",
            Clover => "Trèfle à 4 feuilles",
            Medkit => "Trousse de soin",
            Crown => "Couronne du roi",
            DragonHeart => "Cœur de dragon",
            GatlingSoul => "Âme de gatling",
        }
    }

    pub fn description(self) -> &'static str {
        use ItemKind::*;
        match self {
            FrogLegs => "+1 saut en l'air",
            Jetpack => "+15% hauteur de saut",
            SwiftBoots => "+10% vitesse de déplacement",
            Thruster => "+1 charge de dash",
            DashCoil => "Le dash se recharge 30% plus vite",
            SlideWax => "+25% vitesse de glissade",
            FeatherCape => "+40% contrôle dans les airs",
            PhaseStep => "Invincible pendant les dashs",
            PowerRounds => "+12% dégâts",
            HairTrigger => "+10% cadence de tir",
            CritLens => "+7% chance de coup critique",
            Executioner => "+60% dégâts des critiques",
            UnstableRounds => "10% de chance d'explosion à l'impact",
            TeslaCoil => "12% de chance d'un arc électrique sur 2 ennemis proches",
            HollowPoint => "+30% dégâts aux ennemis en bonne santé (>80% PV)",
            Berserker => "+4% dégâts par tranche de 10% PV manquants",
            Bloodlust => "Chaque kill : +25% vitesse pendant 2 s",
            VitalHeart => "+25 PV max",
            RegenMoss => "+1,5 PV/s",
            ArmorPlate => "+12 armure (moins de dégâts subis)",
            VampireFangs => "Soigne 3% des dégâts infligés",
            EnergyShield => "+25 bouclier qui se recharge hors combat",
            Thorns => "Renvoie 40% des dégâts de mêlée subis",
            Ankh => "Te relève seul une fois quand tu tombes (consommé)",
            LuckyCoin => "+15% or gagné",
            ExtendedMag => "+12% taille des projectiles et zones",
            QuickHands => "+15% vitesse des projectiles",
            TreasureMap => "Coffres -10% moins chers (pour toi)",
            Clover => "Meilleures raretés dans les coffres",
            Medkit => "Soigne 35% de tes PV à chaque niveau d'équipe",
            Crown => "+1 carte au choix dans les coffres",
            DragonHeart => "+60 PV max et +4 PV/s",
            GatlingSoul => "+35% cadence et +1 projectile",
        }
    }

    pub fn category(self) -> Category {
        use ItemKind::*;
        match self {
            FrogLegs | Jetpack | SwiftBoots | Thruster | DashCoil | SlideWax | FeatherCape | PhaseStep => Category::Mobility,
            PowerRounds | HairTrigger | CritLens | Executioner | UnstableRounds | TeslaCoil | HollowPoint | Berserker
            | Bloodlust | GatlingSoul | ExtendedMag | QuickHands => Category::Offense,
            VitalHeart | RegenMoss | ArmorPlate | VampireFangs | EnergyShield | Thorns | Ankh | DragonHeart => {
                Category::Defense
            }
            LuckyCoin | TreasureMap | Clover | Medkit | Crown => Category::Utility,
        }
    }

    pub fn rarity(self) -> Rarity {
        use ItemKind::*;
        match self {
            Jetpack | SwiftBoots | DashCoil | SlideWax | FeatherCape | PowerRounds | HairTrigger | CritLens | VitalHeart
            | RegenMoss | ArmorPlate | Thorns | LuckyCoin | ExtendedMag | QuickHands | Medkit => Rarity::Common,
            FrogLegs | Thruster | Executioner | UnstableRounds | HollowPoint | Bloodlust | VampireFangs | EnergyShield
            | TreasureMap => Rarity::Rare,
            PhaseStep | TeslaCoil | Berserker | Clover => Rarity::Epic,
            Ankh | Crown | DragonHeart | GatlingSoul => Rarity::Legendary,
        }
    }

    /// RGB color used by the HUD and the item's icon background.
    pub fn color(self) -> [f32; 3] {
        self.category().color()
    }

    /// Items of a rarity.
    pub fn of_rarity(rarity: Rarity) -> impl Iterator<Item = ItemKind> {
        Self::ALL.into_iter().filter(move |i| i.rarity() == rarity)
    }
}

/// Draws the cards offered by a chest: `count` distinct items. Each card
/// first rolls a rarity (chest odds, improved by luck), then an item of that
/// rarity not already offered.
pub fn roll_cards(chest: ChestKind, luck: f32, count: usize, rng: &mut Rng) -> Vec<ItemKind> {
    let mut weights = Rarity::weights(chest);
    // Each Clover makes rarer cards more likely.
    let boost = 1.0 + 0.4 * luck;
    weights[1] *= boost;
    weights[2] *= boost * boost;
    weights[3] *= boost * boost;
    let mut cards: Vec<ItemKind> = Vec::with_capacity(count);
    while cards.len() < count.min(ITEM_COUNT) {
        let rarity = Rarity::ALL[rng.weighted_index(&weights)];
        let pool: Vec<ItemKind> = ItemKind::of_rarity(rarity).filter(|i| !cards.contains(i)).collect();
        // Rarity exhausted: fall back to any item not yet drawn.
        let pool = if pool.is_empty() { ItemKind::ALL.into_iter().filter(|i| !cards.contains(i)).collect() } else { pool };
        cards.push(*rng.pick(&pool));
    }
    cards
}

/// Count of each item owned by a player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    #[serde(with = "counts_serde")]
    pub counts: [u8; ITEM_COUNT],
    /// Tome points in tenths (see `build::Tome`).
    pub tomes: [u16; TOME_COUNT],
}

impl Default for Inventory {
    fn default() -> Self {
        Self { counts: [0; ITEM_COUNT], tomes: [0; TOME_COUNT] }
    }
}

/// serde only supports arrays up to 32 elements: (de)serialize as a list.
/// Bonus: a list of another length (other game version) is padded/truncated
/// instead of failing.
mod counts_serde {
    use super::ITEM_COUNT;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(counts: &[u8; ITEM_COUNT], s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(counts.iter())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; ITEM_COUNT], D::Error> {
        let v = Vec::<u8>::deserialize(d)?;
        let mut counts = [0; ITEM_COUNT];
        for (dst, src) in counts.iter_mut().zip(v) {
            *dst = src;
        }
        Ok(counts)
    }
}

impl Inventory {
    pub fn count(&self, item: ItemKind) -> u32 {
        u32::from(self.counts[item.index()])
    }

    pub fn add(&mut self, item: ItemKind) {
        let c = &mut self.counts[item.index()];
        *c = c.saturating_add(1);
    }

    /// Consumes one item (e.g. an Ankh). Returns `false` if none owned.
    pub fn consume(&mut self, item: ItemKind) -> bool {
        let c = &mut self.counts[item.index()];
        if *c == 0 {
            return false;
        }
        *c -= 1;
        true
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
    pub shield_max: f32,
    pub damage_mult: f32,
    pub fire_rate_mult: f32,
    pub crit_chance: f32,
    /// Damage multiplier of a critical hit.
    pub crit_mult: f32,
    pub lifesteal: f32,
    pub gold_mult: f32,
    pub explode_chance: f32,
    pub tesla_chance: f32,
    /// Bonus damage against monsters above 80% health.
    pub hollow_point: f32,
    /// Bonus damage per 10% of missing health.
    pub berserk: f32,
    /// Haste duration (s) granted by each kill.
    pub bloodlust: f32,
    pub thorns: f32,
    pub dash_invulnerable: bool,
    /// Personal chest price multiplier.
    pub price_mult: f32,
    pub luck: f32,
    /// Fraction of max HP healed at the end of each wave.
    pub wave_heal: f32,
    pub cards: usize,
    /// XP gem pickup radius multiplier.
    pub attraction: f32,
    /// XP gain multiplier.
    pub xp_mult: f32,
    /// V3 tome stats.
    pub size_mult: f32,
    pub proj_speed_mult: f32,
    pub duration_mult: f32,
    /// Extra projectiles for every weapon.
    pub quantity: u32,
}

pub const BASE_MAX_HP: f32 = 100.0;
pub const BASE_REGEN: f32 = 1.0;

impl PlayerStats {
    pub fn from_inventory(inv: &Inventory) -> Self {
        use ItemKind::*;
        let n = |i: ItemKind| inv.count(i) as f32;
        let t = |tome: Tome| tome_points(inv, tome);
        Self {
            movement: MoveStats {
                speed_mult: (1.0 + 0.10 * n(SwiftBoots)) * (1.0 + 0.08 * t(Tome::Agility)),
                air_jumps: 1 + inv.counts[FrogLegs.index()],
                dash_charges: 1 + inv.counts[Thruster.index()],
                jump_mult: 1.0 + 0.15 * n(Jetpack),
                dash_recharge_mult: 1.0 + 0.3 * n(DashCoil),
                slide_mult: 1.0 + 0.25 * n(SlideWax),
                air_control_mult: 1.0 + 0.4 * n(FeatherCape),
            },
            max_hp: BASE_MAX_HP + 25.0 * n(VitalHeart) + 60.0 * n(DragonHeart) + 25.0 * t(Tome::Health),
            regen: BASE_REGEN + 1.5 * n(RegenMoss) + 4.0 * n(DragonHeart) + 1.5 * t(Tome::Regen),
            armor: 12.0 * n(ArmorPlate) + 12.0 * t(Tome::Armor),
            shield_max: 25.0 * n(EnergyShield),
            damage_mult: (1.0 + 0.12 * n(PowerRounds)) * (1.0 + 0.08 * t(Tome::Damage)),
            fire_rate_mult: (1.0 + 0.10 * n(HairTrigger) + 0.35 * n(GatlingSoul)) * (1.0 + 0.075 * t(Tome::Cooldown)),
            // Diminishing returns so it never reaches 100%.
            crit_chance: 1.0 - 0.93f32.powf(n(CritLens)) * (1.0 - 0.07 * t(Tome::Precision)).max(0.0),
            crit_mult: 2.0 + 0.6 * n(Executioner),
            lifesteal: 0.03 * n(VampireFangs),
            gold_mult: 1.0 + 0.15 * n(LuckyCoin) + 0.12 * t(Tome::Gold),
            explode_chance: 1.0 - 0.9f32.powf(n(UnstableRounds)),
            tesla_chance: 1.0 - 0.88f32.powf(n(TeslaCoil)),
            hollow_point: 0.3 * n(HollowPoint),
            berserk: 0.04 * n(Berserker),
            bloodlust: 2.0 * n(Bloodlust),
            thorns: 0.4 * n(Thorns),
            dash_invulnerable: inv.count(PhaseStep) > 0,
            price_mult: 0.9f32.powf(n(TreasureMap)).max(0.5),
            luck: n(Clover) + t(Tome::Luck),
            wave_heal: (0.35 * n(Medkit)).min(1.0),
            cards: 3 + inv.count(Crown) as usize,
            attraction: 1.0 + 0.6 * t(Tome::Attraction),
            xp_mult: 1.0 + 0.1 * t(Tome::Experience),
            size_mult: (1.0 + 0.1 * t(Tome::Size)) * (1.0 + 0.12 * n(ExtendedMag)),
            proj_speed_mult: (1.0 + 0.15 * t(Tome::ProjectileSpeed)) * (1.0 + 0.15 * n(QuickHands)),
            duration_mult: 1.0 + 0.15 * t(Tome::Duration),
            quantity: t(Tome::Quantity).floor() as u32 + inv.count(GatlingSoul),
        }
    }

    /// Damage actually taken after armor (classic `armor / (armor + 100)` curve).
    pub fn mitigate(&self, damage: f32) -> f32 {
        damage * 100.0 / (100.0 + self.armor)
    }

    /// Damage multiplier from conditional items, for a hit on a monster.
    pub fn conditional_damage(&self, own_health_frac: f32, target_health_frac: f32) -> f32 {
        let missing_tenths = ((1.0 - own_health_frac.clamp(0.0, 1.0)) * 10.0).floor();
        let mut m = 1.0 + self.berserk * missing_tenths;
        if target_health_frac > 0.8 {
            m += self.hollow_point;
        }
        m
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
    fn every_category_and_rarity_has_items() {
        for cat in [Category::Mobility, Category::Offense, Category::Defense, Category::Utility] {
            assert!(ItemKind::ALL.iter().filter(|i| i.category() == cat).count() >= 5, "{cat:?}");
        }
        for r in Rarity::ALL {
            assert!(ItemKind::of_rarity(r).count() >= 4, "{r:?}");
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
        assert_eq!(s.cards, 3);
        assert_eq!(s.price_mult, 1.0);
        assert_eq!(s.conditional_damage(1.0, 0.5), 1.0);
    }

    #[test]
    fn items_stack() {
        let mut inv = Inventory::default();
        inv.add(ItemKind::FrogLegs);
        inv.add(ItemKind::FrogLegs);
        inv.add(ItemKind::SwiftBoots);
        inv.add(ItemKind::VitalHeart);
        inv.add(ItemKind::Crown);
        let s = PlayerStats::from_inventory(&inv);
        assert_eq!(s.movement.air_jumps, 3);
        assert!((s.movement.speed_mult - 1.10).abs() < 1e-6);
        assert_eq!(s.max_hp, 125.0);
        assert_eq!(s.cards, 4);
        assert_eq!(inv.total(), 5);
    }

    #[test]
    fn inventory_roundtrip() {
        let mut inv = Inventory::default();
        inv.add(ItemKind::GatlingSoul);
        inv.add(ItemKind::FrogLegs);
        let bytes = postcard::to_allocvec(&inv).unwrap();
        let back: Inventory = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(back, inv);
    }

    #[test]
    fn ankh_is_consumed() {
        let mut inv = Inventory::default();
        assert!(!inv.consume(ItemKind::Ankh));
        inv.add(ItemKind::Ankh);
        assert!(inv.consume(ItemKind::Ankh));
        assert_eq!(inv.count(ItemKind::Ankh), 0);
    }

    #[test]
    fn conditional_damage_items() {
        let mut inv = Inventory::default();
        inv.add(ItemKind::Berserker);
        inv.add(ItemKind::HollowPoint);
        let s = PlayerStats::from_inventory(&inv);
        // At 50% HP: 5 tenths missing => +20%; target healthy => +30%.
        assert!((s.conditional_damage(0.5, 1.0) - 1.5).abs() < 1e-5);
        assert!((s.conditional_damage(1.0, 0.5) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cards_are_distinct_and_counted() {
        let mut rng = Rng::new(9);
        for _ in 0..500 {
            let cards = roll_cards(ChestKind::Large, 0.0, 4, &mut rng);
            assert_eq!(cards.len(), 4);
            for (i, c) in cards.iter().enumerate() {
                assert!(!cards[..i].contains(c));
            }
        }
    }

    #[test]
    fn legendary_chests_only_give_rare_stuff() {
        let mut rng = Rng::new(10);
        for _ in 0..300 {
            for c in roll_cards(ChestKind::Legendary, 0.0, 3, &mut rng) {
                assert!(c.rarity() >= Rarity::Epic, "{c:?}");
            }
        }
    }

    #[test]
    fn luck_improves_rarity() {
        let score = |luck: f32| {
            let mut rng = Rng::new(11);
            (0..2000)
                .flat_map(|_| roll_cards(ChestKind::Small, luck, 3, &mut rng))
                .filter(|c| c.rarity() >= Rarity::Epic)
                .count()
        };
        assert!(score(3.0) > score(0.0) * 2);
    }

    #[test]
    fn diminishing_returns() {
        let mut inv = Inventory::default();
        for _ in 0..100 {
            inv.add(ItemKind::CritLens);
            inv.add(ItemKind::TreasureMap);
        }
        let s = PlayerStats::from_inventory(&inv);
        assert!(s.crit_chance < 1.0 && s.crit_chance > 0.9);
        assert_eq!(s.price_mult, 0.5);
    }
}
