//! V3 build: the weapons and tomes of a player, and level-up choices.
//!
//! Two **separate** weapon categories:
//! - the **main weapon** (exactly one, chosen in the lobby): held in hand,
//!   *you aim and shoot*. Its upgrades change its projectiles (count, speed,
//!   size, bounces...);
//! - **automatic weapons** (up to `MAX_AUTOS`): aura, lightning from the
//!   sky, orbiting orbs... they act on their own.
//!
//! **Tomes** (up to `MAX_TOMES`) raise one character stat for every weapon
//! (like Megabonk). Tome levels are stored in the `Inventory` next to items,
//! so `PlayerStats::from_inventory` already includes them.
//!
//! Everything here is pure data + functions: deterministic and tested.

use serde::{Deserialize, Serialize};

use super::items::{Inventory, PlayerStats, Rarity};
use super::rng::Rng;

pub const MAX_AUTOS: usize = 3;
pub const MAX_TOMES: usize = 4;

// ---------------------------------------------------------------------------
// Weapon stats that upgrades can raise
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum WStat {
    Damage,
    Count,
    Speed,
    Size,
    Bounces,
    CritChance,
    CritDamage,
    AttackSpeed,
    Duration,
}

pub const WSTAT_COUNT: usize = 9;

impl WStat {
    pub fn name(self) -> &'static str {
        match self {
            WStat::Damage => "Dégâts",
            WStat::Count => "Projectiles",
            WStat::Speed => "Vitesse",
            WStat::Size => "Taille",
            WStat::Bounces => "Rebonds",
            WStat::CritChance => "Chance de critique",
            WStat::CritDamage => "Dégâts critiques",
            WStat::AttackSpeed => "Cadence",
            WStat::Duration => "Durée",
        }
    }

    /// Value of a *common* upgrade (rarity multiplies it).
    pub fn base_increment(self) -> f32 {
        match self {
            WStat::Damage => 0.15,
            WStat::Count => 1.0,
            WStat::Speed => 0.12,
            WStat::Size => 0.15,
            WStat::Bounces => 1.0,
            WStat::CritChance => 0.08,
            WStat::CritDamage => 0.2,
            WStat::AttackSpeed => 0.08,
            WStat::Duration => 0.15,
        }
    }

    /// Integer stats are rounded (1 extra projectile, 2 at legendary).
    pub fn is_integer(self) -> bool {
        matches!(self, WStat::Count | WStat::Bounces)
    }

    pub fn increment(self, rarity: Rarity) -> f32 {
        let v = self.base_increment() * rarity.factor();
        if self.is_integer() { v.round().max(1.0) } else { v }
    }

    /// Human text for an upgrade value, e.g. "+18% Dégâts" or "+1 Projectiles".
    pub fn describe(self, value: f32) -> String {
        if self.is_integer() {
            format!("+{:.0} {}", value, self.name())
        } else {
            format!("+{:.0}% {}", value * 100.0, self.name())
        }
    }
}

/// Accumulated upgrades of one weapon.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WeaponLevel {
    pub level: u8,
    pub bonus: [f32; WSTAT_COUNT],
}

impl WeaponLevel {
    pub fn get(&self, s: WStat) -> f32 {
        self.bonus[s as usize]
    }

    pub fn apply(&mut self, ups: &[(WStat, f32)]) {
        self.level = self.level.saturating_add(1);
        for (s, v) in ups {
            self.bonus[*s as usize] += v;
        }
    }
}

// ---------------------------------------------------------------------------
// Main weapons (aimed)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MainWeapon {
    FireStaff,
    Bow,
    Revolver,
    Shotgun,
    Sniper,
    LightningStaff,
    Bananarang,
    HeroSword,
}

/// How a main weapon delivers its damage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// Flying projectile.
    Projectile,
    /// Out and back (boomerang).
    Boomerang,
    /// Instant line that pierces everything.
    Beam,
    /// Instant hit on the aimed monster, then chains to nearby ones.
    Chain,
}

#[derive(Clone, Copy, Debug)]
pub struct MainDef {
    pub name: &'static str,
    pub description: &'static str,
    pub delivery: Delivery,
    pub damage: f32,
    /// Seconds between shots (hold the trigger to keep firing).
    pub interval: f32,
    pub count: u32,
    /// Angle between projectiles of a volley (degrees), or random cone for pellets.
    pub spread_deg: f32,
    pub random_spread: bool,
    pub speed: f32,
    /// Projectile radius (hit size) in meters.
    pub size: f32,
    pub range: f32,
    pub bounces: u32,
    /// Monsters a projectile goes through (u32::MAX = all).
    pub pierce: u32,
    /// Explosion radius on impact (0 = none).
    pub splash: f32,
    pub gravity: f32,
    pub crit_chance: f32,
    pub recoil: f32,
    /// Stats its upgrades can roll.
    pub upgrades: &'static [WStat],
    /// Unlocked from the start (others cost silver).
    pub starter: bool,
    pub silver_cost: u32,
}

const MAIN_BASE: MainDef = MainDef {
    name: "",
    description: "",
    delivery: Delivery::Projectile,
    damage: 20.0,
    interval: 0.4,
    count: 1,
    spread_deg: 6.0,
    random_spread: false,
    speed: 40.0,
    size: 0.25,
    range: 70.0,
    bounces: 0,
    pierce: 0,
    splash: 0.0,
    gravity: 0.0,
    crit_chance: 0.05,
    recoil: 0.6,
    upgrades: &[WStat::Damage, WStat::Count, WStat::AttackSpeed],
    starter: false,
    silver_cost: 0,
};

impl MainWeapon {
    pub const ALL: [MainWeapon; 8] = [
        MainWeapon::FireStaff,
        MainWeapon::Bow,
        MainWeapon::Revolver,
        MainWeapon::Shotgun,
        MainWeapon::Sniper,
        MainWeapon::LightningStaff,
        MainWeapon::Bananarang,
        MainWeapon::HeroSword,
    ];

    pub fn def(self) -> MainDef {
        use WStat::*;
        match self {
            MainWeapon::FireStaff => MainDef {
                name: "Bâton de feu",
                description: "Boules de feu qui explosent à l'impact",
                damage: 22.0,
                interval: 0.42,
                speed: 32.0,
                size: 0.35,
                splash: 2.2,
                upgrades: &[Damage, Count, Speed, Size, AttackSpeed],
                starter: true,
                ..MAIN_BASE
            },
            MainWeapon::Bow => MainDef {
                name: "Arc",
                description: "Flèches rapides et précises, gros critiques",
                damage: 26.0,
                interval: 0.5,
                speed: 70.0,
                size: 0.18,
                pierce: 1,
                gravity: 4.0,
                crit_chance: 0.15,
                range: 100.0,
                upgrades: &[Damage, Count, Speed, CritChance, CritDamage, AttackSpeed],
                starter: true,
                ..MAIN_BASE
            },
            MainWeapon::Revolver => MainDef {
                name: "Revolver",
                description: "Balles qui rebondissent d'ennemi en ennemi",
                damage: 30.0,
                interval: 0.36,
                speed: 95.0,
                size: 0.14,
                bounces: 1,
                crit_chance: 0.1,
                range: 90.0,
                upgrades: &[Damage, Count, Bounces, CritChance, CritDamage, AttackSpeed],
                silver_cost: 150,
                ..MAIN_BASE
            },
            MainWeapon::Shotgun => MainDef {
                name: "Fusil à pompe",
                description: "Une gerbe de plombs dévastatrice de près",
                damage: 9.0,
                interval: 0.8,
                count: 7,
                spread_deg: 7.0,
                random_spread: true,
                speed: 75.0,
                size: 0.14,
                range: 35.0,
                recoil: 1.6,
                upgrades: &[Damage, Count, Size, CritChance, AttackSpeed],
                silver_cost: 200,
                ..MAIN_BASE
            },
            MainWeapon::Sniper => MainDef {
                name: "Sniper",
                description: "Rayon instantané qui traverse tout",
                delivery: Delivery::Beam,
                damage: 75.0,
                interval: 1.1,
                spread_deg: 1.5,
                size: 0.3,
                range: 300.0,
                pierce: u32::MAX,
                crit_chance: 0.1,
                recoil: 2.2,
                upgrades: &[Damage, Count, Size, CritChance, CritDamage, AttackSpeed],
                silver_cost: 300,
                ..MAIN_BASE
            },
            MainWeapon::LightningStaff => MainDef {
                name: "Bâton de foudre",
                description: "L'éclair frappe la cible visée puis saute aux voisins",
                delivery: Delivery::Chain,
                damage: 20.0,
                interval: 0.55,
                size: 1.0,
                range: 45.0,
                bounces: 3,
                recoil: 0.5,
                upgrades: &[Damage, Count, Bounces, Size, AttackSpeed],
                silver_cost: 250,
                ..MAIN_BASE
            },
            MainWeapon::Bananarang => MainDef {
                name: "Bananarang",
                description: "Revient comme un boomerang en traversant tout",
                delivery: Delivery::Boomerang,
                damage: 18.0,
                interval: 0.75,
                spread_deg: 12.0,
                speed: 1.0,
                size: 0.45,
                range: 20.0,
                pierce: u32::MAX,
                upgrades: &[Damage, Count, Speed, Size, AttackSpeed],
                silver_cost: 200,
                ..MAIN_BASE
            },
            MainWeapon::HeroSword => MainDef {
                name: "Épée du héros",
                description: "Lance des vagues tranchantes larges et perforantes",
                damage: 28.0,
                interval: 0.55,
                spread_deg: 10.0,
                speed: 38.0,
                size: 0.9,
                range: 26.0,
                pierce: u32::MAX,
                recoil: 0.9,
                upgrades: &[Damage, Count, Speed, Size, AttackSpeed],
                starter: true,
                ..MAIN_BASE
            },
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|w| *w == self).unwrap_or(0)
    }
}

/// Final numbers of a main weapon with its upgrades and the player's stats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MainParams {
    pub kind: MainWeapon,
    pub delivery: Delivery,
    pub damage: f32,
    pub interval: f32,
    pub count: u32,
    pub spread_deg: f32,
    pub random_spread: bool,
    pub speed: f32,
    pub size: f32,
    pub range: f32,
    pub bounces: u32,
    pub pierce: u32,
    pub splash: f32,
    pub gravity: f32,
    pub crit_chance: f32,
    pub crit_mult: f32,
    pub recoil: f32,
}

pub fn main_params(kind: MainWeapon, up: &WeaponLevel, stats: &PlayerStats) -> MainParams {
    let d = kind.def();
    let size_mult = (1.0 + up.get(WStat::Size)) * stats.size_mult;
    MainParams {
        kind,
        delivery: d.delivery,
        damage: d.damage * (1.0 + up.get(WStat::Damage)) * stats.damage_mult,
        interval: d.interval / ((1.0 + up.get(WStat::AttackSpeed)) * stats.fire_rate_mult),
        count: d.count + up.get(WStat::Count) as u32 + stats.quantity,
        spread_deg: d.spread_deg,
        random_spread: d.random_spread,
        speed: d.speed * (1.0 + up.get(WStat::Speed)) * stats.proj_speed_mult,
        size: d.size * size_mult,
        range: d.range * if d.delivery == Delivery::Boomerang { size_mult.sqrt() } else { 1.0 },
        bounces: d.bounces + up.get(WStat::Bounces) as u32,
        pierce: d.pierce,
        splash: d.splash * size_mult,
        gravity: d.gravity,
        crit_chance: (d.crit_chance + up.get(WStat::CritChance) + stats.crit_chance).min(1.0),
        crit_mult: stats.crit_mult + up.get(WStat::CritDamage),
        recoil: d.recoil,
    }
}

// ---------------------------------------------------------------------------
// Automatic weapons
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AutoWeapon {
    Aura,
    Lightning,
    Orbs,
    Axes,
    Mines,
    Tornado,
    Poison,
    FireTrail,
}

#[derive(Clone, Copy, Debug)]
pub struct AutoDef {
    pub name: &'static str,
    pub description: &'static str,
    pub damage: f32,
    /// Seconds between activations.
    pub cooldown: f32,
    pub count: u32,
    pub size: f32,
    pub speed: f32,
    /// Lifetime of what it leaves behind (mines, pools, tornados, fire).
    pub duration: f32,
    pub upgrades: &'static [WStat],
    pub starter: bool,
    pub silver_cost: u32,
}

impl AutoWeapon {
    pub const ALL: [AutoWeapon; 8] = [
        AutoWeapon::Aura,
        AutoWeapon::Lightning,
        AutoWeapon::Orbs,
        AutoWeapon::Axes,
        AutoWeapon::Mines,
        AutoWeapon::Tornado,
        AutoWeapon::Poison,
        AutoWeapon::FireTrail,
    ];

    pub fn def(self) -> AutoDef {
        use WStat::*;
        match self {
            AutoWeapon::Aura => AutoDef {
                name: "Aura",
                description: "Un champ de force brûle les ennemis autour de toi",
                damage: 7.0,
                cooldown: 0.4,
                count: 1,
                size: 3.2,
                speed: 1.0,
                duration: 0.0,
                upgrades: &[Damage, Size, AttackSpeed],
                starter: true,
                silver_cost: 0,
            },
            AutoWeapon::Lightning => AutoDef {
                name: "Foudre céleste",
                description: "Des éclairs frappent des ennemis au hasard depuis le ciel",
                damage: 30.0,
                cooldown: 1.8,
                count: 1,
                size: 1.5,
                speed: 1.0,
                duration: 0.0,
                upgrades: &[Damage, Count, Size, AttackSpeed],
                starter: true,
                silver_cost: 0,
            },
            AutoWeapon::Orbs => AutoDef {
                name: "Orbes gardiens",
                description: "Des orbes tournent autour de toi et frappent au contact",
                damage: 12.0,
                cooldown: 0.25,
                count: 2,
                size: 2.6,
                speed: 2.4,
                duration: 0.0,
                upgrades: &[Damage, Count, Speed, Size],
                starter: true,
                silver_cost: 0,
            },
            AutoWeapon::Axes => AutoDef {
                name: "Haches",
                description: "Lance des haches en cloche qui traversent les ennemis",
                damage: 26.0,
                cooldown: 1.5,
                count: 1,
                size: 0.6,
                speed: 1.0,
                duration: 0.0,
                upgrades: &[Damage, Count, Size, Speed, AttackSpeed],
                starter: false,
                silver_cost: 120,
            },
            AutoWeapon::Mines => AutoDef {
                name: "Mines",
                description: "Pose des mines qui explosent au passage des ennemis",
                damage: 45.0,
                cooldown: 2.2,
                count: 1,
                size: 3.2,
                speed: 1.0,
                duration: 20.0,
                upgrades: &[Damage, Count, Size, Duration, AttackSpeed],
                starter: false,
                silver_cost: 150,
            },
            AutoWeapon::Tornado => AutoDef {
                name: "Tornade",
                description: "Des tornades errantes broient tout sur leur passage",
                damage: 9.0,
                cooldown: 3.2,
                count: 1,
                size: 1.8,
                speed: 7.0,
                duration: 3.5,
                upgrades: &[Damage, Count, Size, Speed, Duration],
                starter: false,
                silver_cost: 180,
            },
            AutoWeapon::Poison => AutoDef {
                name: "Fiole de poison",
                description: "Crée des flaques de poison sous les ennemis",
                damage: 6.0,
                cooldown: 2.6,
                count: 1,
                size: 3.0,
                speed: 1.0,
                duration: 3.5,
                upgrades: &[Damage, Count, Size, Duration, AttackSpeed],
                starter: false,
                silver_cost: 150,
            },
            AutoWeapon::FireTrail => AutoDef {
                name: "Traînée de feu",
                description: "Tu laisses des flammes derrière toi en courant",
                damage: 5.0,
                cooldown: 0.22,
                count: 1,
                size: 1.4,
                speed: 1.0,
                duration: 2.0,
                upgrades: &[Damage, Size, Duration],
                starter: false,
                silver_cost: 120,
            },
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|w| *w == self).unwrap_or(0)
    }
}

/// Final numbers of an automatic weapon.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoParams {
    pub damage: f32,
    pub cooldown: f32,
    pub count: u32,
    pub size: f32,
    pub speed: f32,
    pub duration: f32,
}

pub fn auto_params(kind: AutoWeapon, up: &WeaponLevel, stats: &PlayerStats) -> AutoParams {
    let d = kind.def();
    // Continuous weapons (aura, orbs, trail) keep their tick rate; only the
    // others get faster with attack speed.
    let rate = (1.0 + up.get(WStat::AttackSpeed)) * stats.fire_rate_mult;
    let cooldown = match kind {
        AutoWeapon::Aura => d.cooldown / (1.0 + up.get(WStat::AttackSpeed)),
        AutoWeapon::Orbs | AutoWeapon::FireTrail => d.cooldown,
        _ => d.cooldown / rate,
    };
    AutoParams {
        damage: d.damage * (1.0 + up.get(WStat::Damage)) * stats.damage_mult,
        cooldown,
        count: d.count + up.get(WStat::Count) as u32 + stats.quantity,
        size: d.size * (1.0 + up.get(WStat::Size)) * stats.size_mult,
        speed: d.speed * (1.0 + up.get(WStat::Speed)) * stats.proj_speed_mult,
        duration: d.duration * (1.0 + up.get(WStat::Duration)) * stats.duration_mult,
    }
}

// ---------------------------------------------------------------------------
// Tomes
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Tome {
    Damage,
    Cooldown,
    Quantity,
    Size,
    ProjectileSpeed,
    Duration,
    Health,
    Regen,
    Armor,
    Agility,
    Luck,
    Experience,
    Gold,
    Attraction,
    Precision,
}

pub const TOME_COUNT: usize = 15;

impl Tome {
    pub const ALL: [Tome; TOME_COUNT] = [
        Tome::Damage,
        Tome::Cooldown,
        Tome::Quantity,
        Tome::Size,
        Tome::ProjectileSpeed,
        Tome::Duration,
        Tome::Health,
        Tome::Regen,
        Tome::Armor,
        Tome::Agility,
        Tome::Luck,
        Tome::Experience,
        Tome::Gold,
        Tome::Attraction,
        Tome::Precision,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Tome::Damage => "Tome de dégâts",
            Tome::Cooldown => "Tome de cadence",
            Tome::Quantity => "Tome de quantité",
            Tome::Size => "Tome de taille",
            Tome::ProjectileSpeed => "Tome de vélocité",
            Tome::Duration => "Tome de durée",
            Tome::Health => "Tome de vitalité",
            Tome::Regen => "Tome de régénération",
            Tome::Armor => "Tome d'armure",
            Tome::Agility => "Tome d'agilité",
            Tome::Luck => "Tome de chance",
            Tome::Experience => "Tome d'expérience",
            Tome::Gold => "Tome doré",
            Tome::Attraction => "Tome d'attraction",
            Tome::Precision => "Tome de précision",
        }
    }

    /// Effect of one *point* (a common level; rarity gives more points).
    pub fn per_point(self) -> &'static str {
        match self {
            Tome::Damage => "+8% dégâts de toutes les armes",
            Tome::Cooldown => "+7,5% cadence de toutes les armes",
            Tome::Quantity => "+1 projectile pour toutes les armes (par point entier)",
            Tome::Size => "+10% taille des projectiles et zones",
            Tome::ProjectileSpeed => "+15% vitesse des projectiles",
            Tome::Duration => "+15% durée des effets",
            Tome::Health => "+25 PV max",
            Tome::Regen => "+1,5 PV/s",
            Tome::Armor => "+12 armure",
            Tome::Agility => "+8% vitesse de déplacement",
            Tome::Luck => "Meilleures raretés",
            Tome::Experience => "+10% XP gagnée",
            Tome::Gold => "+12% or gagné",
            Tome::Attraction => "+60% portée de ramassage",
            Tome::Precision => "+7% chance de critique",
        }
    }
}

/// Tome points are stored in tenths (common = 10, legendary = 20).
pub fn tome_points(inv: &Inventory, t: Tome) -> f32 {
    f32::from(inv.tomes[t.index()]) / 10.0
}

pub fn owned_tomes(inv: &Inventory) -> Vec<Tome> {
    Tome::ALL.into_iter().filter(|t| inv.tomes[t.index()] > 0).collect()
}

// ---------------------------------------------------------------------------
// The player's weapons (replicated as a component in `net::protocol`)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Loadout {
    pub main: MainWeapon,
    pub main_up: WeaponLevel,
    pub autos: Vec<(AutoWeapon, WeaponLevel)>,
}

impl Loadout {
    pub fn new(main: MainWeapon) -> Self {
        Self { main, main_up: WeaponLevel::default(), autos: Vec::new() }
    }

    /// Start of a run: the main weapon plus the automatic weapon picked in the lobby.
    pub fn starting(main: MainWeapon, auto: Option<AutoWeapon>) -> Self {
        let mut l = Self::new(main);
        if let Some(a) = auto {
            l.autos.push((a, WeaponLevel { level: 1, ..Default::default() }));
        }
        l
    }

    pub fn has_auto(&self, a: AutoWeapon) -> bool {
        self.autos.iter().any(|(k, _)| *k == a)
    }
}

// ---------------------------------------------------------------------------
// Level-up choices
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Offer {
    /// Improve the main weapon: 1 stat (2 from rare).
    UpgradeMain { stats: Vec<(WStat, f32)> },
    NewAuto(AutoWeapon),
    UpgradeAuto { weapon: AutoWeapon, stats: Vec<(WStat, f32)> },
    /// New tome or one more level: adds `points` tenths.
    Tome { tome: Tome, points: u16 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OfferCard {
    pub offer: Offer,
    pub rarity: Rarity,
}

/// Rarity odds of level-up cards, improved by luck.
pub fn roll_rarity(luck: f32, rng: &mut Rng) -> Rarity {
    let boost = 1.0 + 0.35 * luck;
    let w = [60.0, 25.0 * boost, 10.0 * boost, 4.0 * boost * boost, 1.0 * boost * boost];
    Rarity::TIERS[rng.weighted_index(&w)]
}

fn roll_stats(pool: &[WStat], rarity: Rarity, rng: &mut Rng) -> Vec<(WStat, f32)> {
    let n = if rarity >= Rarity::Rare { 2 } else { 1 };
    let mut out: Vec<(WStat, f32)> = Vec::new();
    while out.len() < n.min(pool.len()) {
        let s = *rng.pick(pool);
        if !out.iter().any(|(x, _)| *x == s) {
            out.push((s, s.increment(rarity)));
        }
    }
    out
}

/// Draws `count` different level-up offers.
/// `auto_pool` = automatic weapons this player has unlocked.
pub fn roll_offers(
    loadout: &Loadout,
    inv: &Inventory,
    auto_pool: &[AutoWeapon],
    luck: f32,
    count: usize,
    rng: &mut Rng,
) -> Vec<OfferCard> {
    // Every possible kind of offer, then pick distinct ones.
    #[derive(Clone, Copy, PartialEq)]
    enum Slot {
        Main,
        NewAuto(AutoWeapon),
        UpAuto(AutoWeapon),
        Tome(Tome),
    }
    let mut candidates: Vec<(Slot, f32)> = vec![(Slot::Main, 3.0)];
    for (a, _) in &loadout.autos {
        candidates.push((Slot::UpAuto(*a), 2.0));
    }
    if loadout.autos.len() < MAX_AUTOS {
        for a in auto_pool.iter().filter(|a| !loadout.has_auto(**a)) {
            candidates.push((Slot::NewAuto(*a), 1.2));
        }
    }
    let owned = owned_tomes(inv);
    for t in &owned {
        candidates.push((Slot::Tome(*t), 1.5));
    }
    if owned.len() < MAX_TOMES {
        for t in Tome::ALL.into_iter().filter(|t| !owned.contains(t)) {
            candidates.push((Slot::Tome(t), 0.6));
        }
    }

    let mut cards = Vec::new();
    while cards.len() < count && !candidates.is_empty() {
        let weights: Vec<f32> = candidates.iter().map(|c| c.1).collect();
        let (slot, _) = candidates.remove(rng.weighted_index(&weights));
        let rarity = roll_rarity(luck, rng);
        let offer = match slot {
            Slot::Main => Offer::UpgradeMain { stats: roll_stats(loadout.main.def().upgrades, rarity, rng) },
            Slot::NewAuto(a) => Offer::NewAuto(a),
            Slot::UpAuto(a) => Offer::UpgradeAuto { weapon: a, stats: roll_stats(a.def().upgrades, rarity, rng) },
            Slot::Tome(t) => Offer::Tome { tome: t, points: (10.0 * rarity.factor()).round() as u16 },
        };
        cards.push(OfferCard { offer, rarity });
    }
    cards
}

/// Applies a chosen offer.
pub fn apply_offer(card: &OfferCard, loadout: &mut Loadout, inv: &mut Inventory) {
    match &card.offer {
        Offer::UpgradeMain { stats } => loadout.main_up.apply(stats),
        Offer::NewAuto(a) => {
            if !loadout.has_auto(*a) && loadout.autos.len() < MAX_AUTOS {
                loadout.autos.push((*a, WeaponLevel { level: 1, ..Default::default() }));
            }
        }
        Offer::UpgradeAuto { weapon, stats } => {
            if let Some((_, up)) = loadout.autos.iter_mut().find(|(k, _)| k == weapon) {
                up.apply(stats);
            }
        }
        Offer::Tome { tome, points } => {
            let t = &mut inv.tomes[tome.index()];
            *t = t.saturating_add(*points);
        }
    }
}

/// Short title + description of an offer card (UI).
pub fn describe_offer(card: &OfferCard) -> (String, String) {
    let stats_text = |stats: &[(WStat, f32)]| stats.iter().map(|(s, v)| s.describe(*v)).collect::<Vec<_>>().join("\n");
    match &card.offer {
        Offer::UpgradeMain { stats } => ("Arme principale".to_string(), stats_text(stats)),
        Offer::NewAuto(a) => (format!("Nouvelle arme : {}", a.def().name), a.def().description.to_string()),
        Offer::UpgradeAuto { weapon, stats } => (weapon.def().name.to_string(), stats_text(stats)),
        Offer::Tome { tome, points } => (
            tome.name().to_string(),
            format!("{}\n(×{:.1})", tome.per_point(), f32::from(*points) / 10.0),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats() -> PlayerStats {
        PlayerStats::from_inventory(&Inventory::default())
    }

    #[test]
    fn increments_scale_with_rarity() {
        assert_eq!(WStat::Count.increment(Rarity::Common), 1.0);
        assert_eq!(WStat::Count.increment(Rarity::Legendary), 2.0);
        assert!((WStat::Damage.increment(Rarity::Legendary) - 0.30).abs() < 1e-5);
    }

    #[test]
    fn main_upgrades_change_params() {
        let s = stats();
        let base = main_params(MainWeapon::FireStaff, &WeaponLevel::default(), &s);
        let mut up = WeaponLevel::default();
        up.apply(&[(WStat::Count, 2.0), (WStat::Size, 0.5), (WStat::Speed, 0.5)]);
        let better = main_params(MainWeapon::FireStaff, &up, &s);
        assert_eq!(better.count, base.count + 2);
        assert!(better.size > base.size && better.speed > base.speed && better.splash > base.splash);
    }

    #[test]
    fn tomes_affect_all_weapons() {
        let mut inv = Inventory::default();
        inv.tomes[Tome::Quantity.index()] = 10;
        inv.tomes[Tome::Damage.index()] = 20;
        let s = PlayerStats::from_inventory(&inv);
        let p = main_params(MainWeapon::Bow, &WeaponLevel::default(), &s);
        assert_eq!(p.count, 2);
        let a = auto_params(AutoWeapon::Lightning, &WeaponLevel::default(), &s);
        assert_eq!(a.count, 2);
        assert!(p.damage > MainWeapon::Bow.def().damage * 1.1);
    }

    #[test]
    fn offers_respect_slots_and_are_distinct() {
        let mut rng = Rng::new(5);
        let mut loadout = Loadout::new(MainWeapon::FireStaff);
        let mut inv = Inventory::default();
        // Fill all auto and tome slots by applying random offers.
        for _ in 0..200 {
            let cards = roll_offers(&loadout, &inv, &AutoWeapon::ALL, 0.0, 3, &mut rng);
            assert!(!cards.is_empty());
            for (i, c) in cards.iter().enumerate() {
                for d in &cards[..i] {
                    assert!(std::mem::discriminant(&c.offer) != std::mem::discriminant(&d.offer) || c.offer != d.offer);
                }
            }
            apply_offer(&cards[0], &mut loadout, &mut inv);
            assert!(loadout.autos.len() <= MAX_AUTOS);
            assert!(owned_tomes(&inv).len() <= MAX_TOMES);
        }
    }

    #[test]
    fn luck_improves_offer_rarity() {
        let count = |luck: f32| {
            let mut rng = Rng::new(3);
            (0..3000).filter(|_| roll_rarity(luck, &mut rng) >= Rarity::Epic).count()
        };
        assert!(count(3.0) > count(0.0) * 2);
    }

    #[test]
    fn every_weapon_has_upgrades_and_starters_exist() {
        assert!(MainWeapon::ALL.iter().any(|w| w.def().starter));
        assert!(AutoWeapon::ALL.iter().any(|w| w.def().starter));
        for w in MainWeapon::ALL {
            assert!(!w.def().upgrades.is_empty());
        }
        for w in AutoWeapon::ALL {
            assert!(!w.def().upgrades.is_empty());
        }
    }
}
