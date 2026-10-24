//! Monster definitions (stats only — models live in `render::models`).
//!
//! Every monster has a **role** (swarmer, blob, ranged, bomber, tank,
//! support, charger, boss) and a minute from which it can appear (early,
//! mid or late game). Each run draws a random *roster*: for every role only
//! some of its monsters are in the run, so two runs never look the same.
//!
//! Monsters can also have **hit zones**: spheres attached to the body that
//! change the damage of a direct hit (weak points ×2/×3, shields and shells
//! that block most of the damage, boss legs that can be broken).

use bevy::math::{Quat, Vec3};
use serde::{Deserialize, Serialize};

use super::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MobKind {
    /// Small, fast melee swarmer.
    Skitter,
    /// Ground ranged attacker that lobs acid.
    Spitter,
    /// Flying orb that strafes and shoots bolts.
    Wisp,
    /// Kamikaze that explodes next to you.
    Bomber,
    /// Big slow tank with a ground slam.
    Brute,
    /// Huge boss: slam shockwave + projectile volleys.
    Colossus,
    /// Crystal rhino: winds up, then charges in a straight line.
    Charger,
    /// Slime that splits into two small ones when it dies.
    Jelly,
    /// Small slime born from a Jelly.
    Jellet,
    /// Floating mask that heals nearby monsters and throws slow orbs.
    Shaman,
    /// Flying boss: summons swarms and rains bolts.
    HiveQueen,
    /// Ground boss: shockwaves to jump over + telegraphed lightning strikes.
    StormTitan,
    /// Final Swarm (15 min+): flies through everything, gets stronger every minute.
    WhiteGhost,
    /// Final Swarm (20 min+): faster and deadlier.
    BlackGhost,
    // --- V4 ---
    /// Swarmer: jumps at you in big hops.
    Flea,
    /// Swarmer: travels underground (invulnerable), pops out to bite.
    Burrower,
    /// Swarmer: armored shell in front, soft back (×2).
    Scarab,
    /// Blob: leaves a pool of acid when it dies.
    ToxicJelly,
    /// Blob: explodes a moment after dying (get away!).
    MagmaJelly,
    /// Blob: huge, splits into two jellies.
    MegaJelly,
    /// Ranged: shells that land on a marked circle.
    Mortar,
    /// Ranged (flying): charges a laser beam, weak pupil (×2).
    Eye,
    /// Ranged: shoots a ring of thorns all around.
    Thornback,
    /// Ranged (flying): floats above you and drops acid.
    SkyJelly,
    /// Bomber: walking mushroom that bursts into a poison cloud.
    Sporeling,
    /// Bomber (flying): fast diving bat that explodes.
    FireBat,
    /// Tank: stone golem with a glowing core in its back (×3).
    StoneGolem,
    /// Tank: crystal golem shooting shards, crystal head (×2.5).
    CrystalGolem,
    /// Tank: big shield in front (blocks 90%), weak head (×2).
    ShieldKnight,
    /// Support: summons fleas from afar.
    Summoner,
    /// Charger: lava boar, leaves burning lava when it charges.
    LavaBoar,
    /// Boss: giant spider. Break its 4 legs, it collapses and takes ×2.
    Broodmother,
    /// Boss: armored golem, core in its back (×3); kneels after slamming.
    AncientGolem,
    /// Boss: worm that swims underground and erupts under you; mouth ×2.5.
    Sandworm,
}

/// What a monster is for: each run keeps only a few monsters per role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Swarmer,
    Blob,
    Ranged,
    Bomber,
    Tank,
    Support,
    Charger,
    Boss,
    /// Never spawned by the director (split jellies, ghosts).
    Special,
}

impl Role {
    /// How many monsters of this role a run keeps.
    pub fn roster_size(self) -> usize {
        match self {
            Role::Ranged => 3,
            Role::Swarmer | Role::Blob | Role::Bomber | Role::Tank => 2,
            _ => 1,
        }
    }
}

/// Effect of a hit zone on a direct hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ZoneKind {
    /// Weak point: damage × the factor.
    Weak(f32),
    /// Shell or shield: damage × the factor (< 1).
    Armor(f32),
    /// Breakable leg (index of its bit in the broken mask).
    Leg(u8),
}

/// A sphere attached to the monster (local frame: x right, y up, -z forward).
#[derive(Clone, Copy, Debug)]
pub struct HitZone {
    pub offset: Vec3,
    pub radius: f32,
    pub kind: ZoneKind,
}

/// A static list of zones (built at compile time).
macro_rules! zones {
    ($($z:expr),* $(,)?) => {{
        const Z: &[HitZone] = &[$($z),*];
        Z
    }};
}

const fn zone(x: f32, y: f32, z: f32, radius: f32, kind: ZoneKind) -> HitZone {
    HitZone { offset: Vec3::new(x, y, z), radius, kind }
}

/// Legs of the Broodmother (also used by its model, so they line up).
/// (the knees: shoot the glowing joints).
pub const BROOD_LEGS: [Vec3; 4] = [Vec3::new(3.2, 3.2, -1.8), Vec3::new(-3.2, 3.2, -1.8), Vec3::new(3.2, 3.2, 1.8), Vec3::new(-3.2, 3.2, 1.8)];
/// Fraction of the boss max HP each leg has.
pub const LEG_HP: f32 = 0.06;
/// Time a toppled boss stays on the ground.
pub const TOPPLE_TIME: f32 = 7.0;

#[derive(Clone, Copy, Debug)]
pub struct MobDef {
    pub name: &'static str,
    pub hp: f32,
    pub speed: f32,
    /// Collision radius / height (for hits and terrain).
    pub radius: f32,
    pub height: f32,
    pub damage: f32,
    /// Distance at which the mob starts its attack.
    pub attack_range: f32,
    pub attack_cooldown: f32,
    /// Wind-up before the hit lands (telegraph, readable for the player).
    pub windup: f32,
    /// Spawn budget cost for the director.
    pub cost: f32,
    /// Gold paid to every player on kill (before difficulty scaling).
    pub gold: f32,
    /// Minute of the run from which it can spawn.
    pub unlock_minute: f32,
    /// Relative spawn probability among unlocked monsters.
    pub spawn_weight: f32,
    pub flying: bool,
    pub ranged: bool,
}

/// Defaults for the fields a new monster does not care about.
const fn base(name: &'static str) -> MobDef {
    MobDef {
        name,
        hp: 50.0,
        speed: 5.0,
        radius: 0.6,
        height: 1.0,
        damage: 8.0,
        attack_range: 1.8,
        attack_cooldown: 1.4,
        windup: 0.4,
        cost: 1.0,
        gold: 3.0,
        unlock_minute: 0.0,
        spawn_weight: 1.0,
        flying: false,
        ranged: false,
    }
}

impl MobKind {
    pub const SPAWNABLE: [MobKind; 25] = [
        MobKind::Skitter,
        MobKind::Spitter,
        MobKind::Wisp,
        MobKind::Bomber,
        MobKind::Brute,
        MobKind::Charger,
        MobKind::Jelly,
        MobKind::Shaman,
        MobKind::Flea,
        MobKind::Burrower,
        MobKind::Scarab,
        MobKind::ToxicJelly,
        MobKind::MagmaJelly,
        MobKind::MegaJelly,
        MobKind::Mortar,
        MobKind::Eye,
        MobKind::Thornback,
        MobKind::SkyJelly,
        MobKind::Sporeling,
        MobKind::FireBat,
        MobKind::StoneGolem,
        MobKind::CrystalGolem,
        MobKind::ShieldKnight,
        MobKind::Summoner,
        MobKind::LavaBoar,
    ];
    pub const BOSSES: [MobKind; 6] =
        [MobKind::Colossus, MobKind::HiveQueen, MobKind::StormTitan, MobKind::Broodmother, MobKind::AncientGolem, MobKind::Sandworm];

    pub fn def(self) -> MobDef {
        match self {
            MobKind::Skitter => MobDef {
                name: "Trotteur",
                hp: 45.0,
                speed: 8.5,
                radius: 0.5,
                height: 0.8,
                damage: 5.0,
                attack_range: 1.6,
                attack_cooldown: 1.3,
                windup: 0.35,
                cost: 1.0,
                gold: 3.0,
                unlock_minute: 0.0,
                spawn_weight: 3.0,
                flying: false,
                ranged: false,
            },
            MobKind::Spitter => MobDef {
                name: "Cracheur",
                hp: 70.0,
                speed: 4.5,
                radius: 0.6,
                height: 1.9,
                damage: 10.0,
                attack_range: 28.0,
                attack_cooldown: 2.6,
                windup: 0.6,
                cost: 2.0,
                gold: 5.0,
                unlock_minute: 1.0,
                spawn_weight: 2.0,
                flying: false,
                ranged: true,
            },
            MobKind::Wisp => MobDef {
                name: "Feu follet",
                hp: 55.0,
                speed: 6.5,
                radius: 0.6,
                height: 1.0,
                damage: 8.0,
                attack_range: 24.0,
                attack_cooldown: 1.8,
                windup: 0.5,
                cost: 2.0,
                gold: 5.0,
                unlock_minute: 2.0,
                spawn_weight: 2.0,
                flying: true,
                ranged: true,
            },
            MobKind::Bomber => MobDef {
                name: "Bombeur",
                hp: 35.0,
                speed: 9.5,
                radius: 0.55,
                height: 1.1,
                damage: 28.0,
                attack_range: 2.2,
                attack_cooldown: 99.0,
                windup: 0.55,
                cost: 2.0,
                gold: 4.0,
                unlock_minute: 3.5,
                spawn_weight: 1.4,
                flying: false,
                ranged: false,
            },
            MobKind::Brute => MobDef {
                name: "Brute",
                hp: 420.0,
                speed: 4.2,
                radius: 1.3,
                height: 3.2,
                damage: 24.0,
                attack_range: 3.6,
                attack_cooldown: 2.2,
                windup: 0.7,
                cost: 7.0,
                gold: 18.0,
                unlock_minute: 6.0,
                spawn_weight: 1.0,
                flying: false,
                ranged: false,
            },
            MobKind::Colossus => MobDef {
                name: "Colosse",
                hp: 4500.0,
                speed: 3.6,
                radius: 3.2,
                height: 8.0,
                damage: 35.0,
                attack_range: 8.0,
                attack_cooldown: 2.4,
                windup: 1.0,
                cost: 0.0,
                gold: 150.0,
                unlock_minute: 999.0,
                spawn_weight: 0.0,
                flying: false,
                ranged: true,
            },
            MobKind::Charger => MobDef {
                name: "Chargeur",
                hp: 170.0,
                speed: 5.0,
                radius: 0.9,
                height: 1.5,
                damage: 22.0,
                attack_range: 16.0,
                attack_cooldown: 3.5,
                windup: 0.9,
                cost: 3.0,
                gold: 8.0,
                unlock_minute: 3.0,
                spawn_weight: 1.3,
                flying: false,
                ranged: true,
            },
            MobKind::Jelly => MobDef {
                name: "Gelée",
                hp: 90.0,
                speed: 3.8,
                radius: 0.8,
                height: 1.2,
                damage: 9.0,
                attack_range: 1.9,
                attack_cooldown: 1.4,
                windup: 0.4,
                cost: 2.0,
                gold: 4.0,
                unlock_minute: 0.5,
                spawn_weight: 1.6,
                flying: false,
                ranged: false,
            },
            MobKind::Jellet => MobDef {
                name: "Gelette",
                hp: 28.0,
                speed: 7.0,
                radius: 0.4,
                height: 0.6,
                damage: 4.0,
                attack_range: 1.3,
                attack_cooldown: 1.0,
                windup: 0.25,
                cost: 0.0,
                gold: 1.0,
                unlock_minute: 999.0,
                spawn_weight: 0.0,
                flying: false,
                ranged: false,
            },
            MobKind::Shaman => MobDef {
                name: "Chaman",
                hp: 85.0,
                speed: 4.5,
                radius: 0.6,
                height: 1.4,
                damage: 12.0,
                attack_range: 30.0,
                attack_cooldown: 3.5,
                windup: 0.8,
                cost: 4.0,
                gold: 10.0,
                unlock_minute: 5.0,
                spawn_weight: 0.8,
                flying: true,
                ranged: true,
            },
            MobKind::HiveQueen => MobDef {
                name: "Reine de la ruche",
                hp: 3400.0,
                speed: 5.5,
                radius: 2.2,
                height: 3.5,
                damage: 14.0,
                attack_range: 40.0,
                attack_cooldown: 2.0,
                windup: 0.8,
                cost: 0.0,
                gold: 160.0,
                unlock_minute: 999.0,
                spawn_weight: 0.0,
                flying: true,
                ranged: true,
            },
            MobKind::StormTitan => MobDef {
                name: "Titan de foudre",
                hp: 5200.0,
                speed: 3.0,
                radius: 2.6,
                height: 7.5,
                damage: 30.0,
                attack_range: 45.0,
                attack_cooldown: 3.2,
                windup: 1.1,
                cost: 0.0,
                gold: 180.0,
                unlock_minute: 999.0,
                spawn_weight: 0.0,
                flying: false,
                ranged: true,
            },
            MobKind::WhiteGhost => MobDef {
                name: "Fantôme blanc",
                hp: 60.0,
                speed: 9.0,
                radius: 0.6,
                height: 1.4,
                damage: 12.0,
                attack_range: 1.6,
                attack_cooldown: 0.8,
                windup: 0.15,
                cost: 0.0,
                gold: 2.0,
                unlock_minute: 999.0,
                spawn_weight: 0.0,
                flying: true,
                ranged: false,
            },
            MobKind::BlackGhost => MobDef {
                name: "Fantôme noir",
                hp: 160.0,
                speed: 12.0,
                radius: 0.7,
                height: 1.6,
                damage: 25.0,
                attack_range: 1.8,
                attack_cooldown: 0.7,
                windup: 0.12,
                cost: 0.0,
                gold: 4.0,
                unlock_minute: 999.0,
                spawn_weight: 0.0,
                flying: true,
                ranged: false,
            },
            // --- Swarmers ---
            MobKind::Flea => MobDef {
                hp: 30.0,
                speed: 6.5,
                radius: 0.45,
                height: 0.7,
                damage: 5.0,
                attack_range: 1.7,
                attack_cooldown: 1.2,
                windup: 0.25,
                spawn_weight: 3.0,
                ..base("Puce sauteuse")
            },
            MobKind::Burrower => MobDef {
                hp: 60.0,
                speed: 8.0,
                radius: 0.55,
                height: 0.8,
                damage: 9.0,
                attack_range: 2.2,
                attack_cooldown: 0.6,
                windup: 0.6,
                cost: 1.5,
                gold: 4.0,
                unlock_minute: 3.0,
                spawn_weight: 2.5,
                ..base("Fouisseur")
            },
            MobKind::Scarab => MobDef {
                hp: 80.0,
                speed: 6.5,
                radius: 0.7,
                height: 0.9,
                damage: 8.0,
                attack_range: 1.9,
                attack_cooldown: 1.3,
                windup: 0.35,
                cost: 1.5,
                gold: 4.0,
                unlock_minute: 2.0,
                spawn_weight: 2.5,
                ..base("Scarabée blindé")
            },
            // --- Blobs ---
            MobKind::ToxicJelly => MobDef {
                hp: 80.0,
                speed: 4.0,
                radius: 0.8,
                height: 1.2,
                damage: 8.0,
                attack_range: 1.9,
                cost: 2.0,
                gold: 4.0,
                unlock_minute: 0.5,
                spawn_weight: 1.6,
                ..base("Gelée toxique")
            },
            MobKind::MagmaJelly => MobDef {
                hp: 100.0,
                speed: 4.2,
                radius: 0.8,
                height: 1.2,
                damage: 10.0,
                attack_range: 1.9,
                cost: 2.5,
                gold: 5.0,
                unlock_minute: 3.0,
                spawn_weight: 1.4,
                ..base("Gelée de magma")
            },
            MobKind::MegaJelly => MobDef {
                hp: 380.0,
                speed: 3.2,
                radius: 1.6,
                height: 2.4,
                damage: 18.0,
                attack_range: 3.0,
                attack_cooldown: 1.8,
                windup: 0.6,
                cost: 6.0,
                gold: 12.0,
                unlock_minute: 8.0,
                spawn_weight: 0.9,
                ..base("Gelée géante")
            },
            // --- Ranged ---
            MobKind::Mortar => MobDef {
                hp: 110.0,
                speed: 3.5,
                radius: 0.9,
                height: 1.3,
                damage: 18.0,
                attack_range: 42.0,
                attack_cooldown: 3.6,
                windup: 0.8,
                cost: 3.0,
                gold: 7.0,
                unlock_minute: 4.0,
                spawn_weight: 1.4,
                ranged: true,
                ..base("Crabe mortier")
            },
            MobKind::Eye => MobDef {
                hp: 70.0,
                speed: 5.0,
                radius: 0.7,
                height: 1.4,
                damage: 20.0,
                attack_range: 34.0,
                attack_cooldown: 3.2,
                windup: 1.3,
                cost: 3.0,
                gold: 7.0,
                unlock_minute: 4.5,
                spawn_weight: 1.4,
                flying: true,
                ranged: true,
                ..base("Œil sentinelle")
            },
            MobKind::Thornback => MobDef {
                hp: 90.0,
                speed: 4.5,
                radius: 0.8,
                height: 1.0,
                damage: 7.0,
                attack_range: 16.0,
                attack_cooldown: 2.8,
                windup: 0.7,
                cost: 2.0,
                gold: 5.0,
                unlock_minute: 1.0,
                spawn_weight: 1.8,
                ranged: true,
                ..base("Hérisson épineux")
            },
            MobKind::SkyJelly => MobDef {
                hp: 120.0,
                speed: 5.5,
                radius: 1.0,
                height: 1.6,
                damage: 14.0,
                attack_range: 7.0,
                attack_cooldown: 2.2,
                windup: 0.6,
                cost: 3.5,
                gold: 8.0,
                unlock_minute: 9.0,
                spawn_weight: 1.2,
                flying: true,
                ranged: true,
                ..base("Méduse céleste")
            },
            // --- Bombers ---
            MobKind::Sporeling => MobDef {
                hp: 40.0,
                speed: 5.0,
                radius: 0.55,
                height: 1.0,
                damage: 10.0,
                attack_range: 2.2,
                attack_cooldown: 99.0,
                windup: 0.6,
                cost: 1.5,
                gold: 3.0,
                unlock_minute: 1.5,
                spawn_weight: 1.4,
                ..base("Champignon explosif")
            },
            MobKind::FireBat => MobDef {
                hp: 30.0,
                speed: 13.0,
                radius: 0.5,
                height: 0.7,
                damage: 22.0,
                attack_range: 2.0,
                attack_cooldown: 99.0,
                windup: 0.2,
                cost: 2.0,
                gold: 4.0,
                unlock_minute: 7.0,
                spawn_weight: 1.4,
                flying: true,
                ..base("Chauve-souris de feu")
            },
            // --- Tanks ---
            MobKind::StoneGolem => MobDef {
                hp: 520.0,
                speed: 3.6,
                radius: 1.4,
                height: 3.4,
                damage: 26.0,
                attack_range: 3.8,
                attack_cooldown: 2.4,
                windup: 0.8,
                cost: 7.0,
                gold: 20.0,
                unlock_minute: 5.0,
                spawn_weight: 0.9,
                ..base("Golem de pierre")
            },
            MobKind::CrystalGolem => MobDef {
                hp: 600.0,
                speed: 3.4,
                radius: 1.3,
                height: 3.4,
                damage: 14.0,
                attack_range: 30.0,
                attack_cooldown: 2.6,
                windup: 0.9,
                cost: 8.0,
                gold: 22.0,
                unlock_minute: 10.0,
                spawn_weight: 0.8,
                ranged: true,
                ..base("Golem de cristal")
            },
            MobKind::ShieldKnight => MobDef {
                hp: 300.0,
                speed: 4.6,
                radius: 0.9,
                height: 2.4,
                damage: 20.0,
                attack_range: 2.6,
                attack_cooldown: 1.6,
                windup: 0.5,
                cost: 5.0,
                gold: 14.0,
                unlock_minute: 9.0,
                spawn_weight: 1.0,
                ..base("Chevalier au bouclier")
            },
            // --- Support / charger ---
            MobKind::Summoner => MobDef {
                hp: 120.0,
                speed: 4.5,
                radius: 0.6,
                height: 1.8,
                damage: 10.0,
                attack_range: 30.0,
                attack_cooldown: 5.0,
                windup: 1.0,
                cost: 4.0,
                gold: 10.0,
                unlock_minute: 6.0,
                spawn_weight: 0.8,
                flying: true,
                ranged: true,
                ..base("Invocateur")
            },
            MobKind::LavaBoar => MobDef {
                hp: 220.0,
                speed: 5.5,
                radius: 0.9,
                height: 1.4,
                damage: 26.0,
                attack_range: 18.0,
                attack_cooldown: 3.2,
                windup: 0.8,
                cost: 4.0,
                gold: 10.0,
                unlock_minute: 8.0,
                spawn_weight: 1.0,
                ranged: true,
                ..base("Sanglier de lave")
            },
            // --- Bosses ---
            MobKind::Broodmother => MobDef {
                hp: 4800.0,
                speed: 4.2,
                radius: 2.6,
                height: 3.6,
                damage: 26.0,
                attack_range: 40.0,
                attack_cooldown: 2.2,
                windup: 0.9,
                cost: 0.0,
                gold: 170.0,
                unlock_minute: 999.0,
                spawn_weight: 0.0,
                ranged: true,
                ..base("Mère-araignée")
            },
            MobKind::AncientGolem => MobDef {
                hp: 6000.0,
                speed: 2.8,
                radius: 2.6,
                height: 8.0,
                damage: 34.0,
                attack_range: 45.0,
                attack_cooldown: 2.8,
                windup: 1.0,
                cost: 0.0,
                gold: 190.0,
                unlock_minute: 999.0,
                spawn_weight: 0.0,
                ranged: true,
                ..base("Golem ancien")
            },
            MobKind::Sandworm => MobDef {
                hp: 5000.0,
                speed: 9.0,
                radius: 1.8,
                height: 7.0,
                damage: 32.0,
                attack_range: 45.0,
                attack_cooldown: 2.4,
                windup: 1.4,
                cost: 0.0,
                gold: 180.0,
                unlock_minute: 999.0,
                spawn_weight: 0.0,
                ranged: true,
                ..base("Ver des sables")
            },
        }
    }

    pub fn role(self) -> Role {
        use MobKind::*;
        match self {
            Skitter | Flea | Burrower | Scarab => Role::Swarmer,
            Jelly | ToxicJelly | MagmaJelly | MegaJelly => Role::Blob,
            Spitter | Wisp | Mortar | Eye | Thornback | SkyJelly => Role::Ranged,
            Bomber | Sporeling | FireBat => Role::Bomber,
            Brute | StoneGolem | CrystalGolem | ShieldKnight => Role::Tank,
            Shaman | Summoner => Role::Support,
            Charger | LavaBoar => Role::Charger,
            Colossus | HiveQueen | StormTitan | Broodmother | AncientGolem | Sandworm => Role::Boss,
            Jellet | WhiteGhost | BlackGhost => Role::Special,
        }
    }

    /// Damage factor of the body itself (heavily armored monsters < 1).
    /// `toppled`: a boss lying on the ground (no resistance left).
    pub fn body_mult(self, toppled: bool) -> f32 {
        match (self, toppled) {
            (MobKind::Broodmother, false) => 0.2,
            (MobKind::Broodmother, true) => 2.0,
            (MobKind::AncientGolem, false) => 0.3,
            (MobKind::AncientGolem, true) => 1.0,
            (MobKind::StoneGolem, _) => 0.5,
            (MobKind::CrystalGolem, _) => 0.6,
            _ => 1.0,
        }
    }

    /// Hit zones (weak points, shields, legs), in the local frame.
    pub fn zones(self) -> &'static [HitZone] {
        use ZoneKind::*;
        match self {
            MobKind::Scarab => zones![zone(0.0, 0.5, -0.6, 0.6, Armor(0.25)), zone(0.0, 0.55, 0.65, 0.42, Weak(2.0))],
            MobKind::Eye => zones![zone(0.0, 0.7, -0.55, 0.38, Weak(2.0))],
            MobKind::StoneGolem => zones![zone(0.0, 2.3, 0.85, 0.6, Weak(3.0))],
            MobKind::CrystalGolem => zones![zone(0.0, 3.25, -0.1, 0.6, Weak(2.5))],
            MobKind::ShieldKnight => zones![zone(0.0, 1.2, -0.95, 0.9, Armor(0.1)), zone(0.0, 2.2, -0.1, 0.45, Weak(2.0))],
            MobKind::Brute => zones![zone(0.0, 2.5, 0.8, 0.6, Weak(2.0))],
            MobKind::Summoner => zones![zone(0.0, 1.5, -0.3, 0.4, Weak(2.0))],
            MobKind::Colossus => zones![zone(0.0, 5.1, -1.6, 1.1, Weak(2.0))],
            MobKind::StormTitan => zones![zone(0.0, 4.5, -1.0, 1.1, Weak(2.0))],
            MobKind::HiveQueen => zones![zone(0.0, 1.8, 2.2, 1.5, Weak(2.0))],
            MobKind::AncientGolem => zones![zone(0.0, 5.0, 1.5, 1.1, Weak(3.0))],
            MobKind::Sandworm => zones![zone(0.0, 6.4, -1.0, 1.2, Weak(2.5))],
            MobKind::Broodmother => zones![
                zone(BROOD_LEGS[0].x, BROOD_LEGS[0].y, BROOD_LEGS[0].z, 1.1, Leg(0)),
                zone(BROOD_LEGS[1].x, BROOD_LEGS[1].y, BROOD_LEGS[1].z, 1.1, Leg(1)),
                zone(BROOD_LEGS[2].x, BROOD_LEGS[2].y, BROOD_LEGS[2].z, 1.1, Leg(2)),
                zone(BROOD_LEGS[3].x, BROOD_LEGS[3].y, BROOD_LEGS[3].z, 1.1, Leg(3)),
            ],
            _ => zones![],
        }
    }

    /// Height factor of a toppled boss (it lies on the ground).
    pub fn topple_squash(self) -> f32 {
        match self {
            MobKind::Broodmother => 0.45,
            MobKind::AncientGolem => 0.6,
            _ => 1.0,
        }
    }

    /// What to know about it (shown the first time it appears in a run).
    pub fn hint(self) -> &'static str {
        match self {
            MobKind::Skitter => "araignée rapide, en meute",
            MobKind::Spitter => "tire de l'acide en cloche",
            MobKind::Wisp => "vole autour de toi et tire des éclairs",
            MobKind::Bomber => "explose au contact : tue-le de loin",
            MobKind::Brute => "tank, frappe le sol — point faible dans le dos (×2)",
            MobKind::Charger => "gratte le sol puis fonce tout droit : écarte-toi",
            MobKind::Jelly => "se divise en deux à sa mort",
            MobKind::Shaman => "soigne les autres monstres : à abattre en premier",
            MobKind::Flea => "bondit sur toi de loin",
            MobKind::Burrower => "invulnérable sous terre : frappe-le quand il sort",
            MobKind::Scarab => "carapace de face (-75%) : vise son dos (×2)",
            MobKind::ToxicJelly => "laisse une flaque d'acide en mourant",
            MobKind::MagmaJelly => "explose une seconde après sa mort : recule !",
            MobKind::MegaJelly => "énorme, se divise en deux gelées",
            MobKind::Mortar => "bombarde un cercle au sol : sors du cercle",
            MobKind::Eye => "charge un laser : bouge pendant qu'il brille — pupille ×2",
            MobKind::Thornback => "projette un anneau d'épines tout autour",
            MobKind::SkyJelly => "flotte au-dessus de toi et lâche de l'acide",
            MobKind::Sporeling => "éclate en nuage toxique",
            MobKind::FireBat => "plonge sur toi et explose",
            MobKind::StoneGolem => "très résistant — noyau lumineux dans le dos (×3)",
            MobKind::CrystalGolem => "tire des éclats — tête de cristal (×2.5)",
            MobKind::ShieldKnight => "bouclier de face (-90%) : contourne-le, tête ×2",
            MobKind::Summoner => "invoque des puces — vise sa lanterne (×2)",
            MobKind::LavaBoar => "charge en laissant de la lave",
            MobKind::Colossus => "frappe au sol et salves — cœur rose ×2",
            MobKind::HiveQueen => "invoque des essaims — abdomen ×2",
            MobKind::StormTitan => "ondes de choc (saute !) — cristal du torse ×2",
            MobKind::Broodmother => "casse ses 4 pattes : elle s'effondre et prend ×2",
            MobKind::AncientGolem => "blindé — noyau dans le dos ×3, s'agenouille après ses frappes",
            MobKind::Sandworm => "invulnérable sous terre, surgit sous toi — gueule ×2.5",
            MobKind::Jellet | MobKind::WhiteGhost | MobKind::BlackGhost => "",
        }
    }

    pub fn is_ghost(self) -> bool {
        matches!(self, MobKind::WhiteGhost | MobKind::BlackGhost)
    }

    pub fn is_boss(self) -> bool {
        Self::BOSSES.contains(&self)
    }
}

/// A zone in world space, for a monster at `pos` facing `yaw`.
pub fn zone_center(z: &HitZone, pos: Vec3, yaw: f32, squash: f32) -> Vec3 {
    let local = Vec3::new(z.offset.x, z.offset.y * squash, z.offset.z);
    pos + Quat::from_rotation_y(yaw) * local
}

/// Distance along a ray (unit `dir`) to a sphere, if it hits.
pub fn ray_sphere(origin: Vec3, dir: Vec3, center: Vec3, r: f32) -> Option<f32> {
    let oc = origin - center;
    let b = oc.dot(dir);
    let c = oc.length_squared() - r * r;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let t = -b - disc.sqrt();
    if t >= 0.0 { Some(t) } else if c <= 0.0 { Some(0.0) } else { None }
}

/// The monsters of a run: for every role, a few of its monsters (at least
/// one that can appear in the first minutes, so the run starts right away).
pub fn roll_roster(rng: &mut Rng) -> Vec<MobKind> {
    let mut roster = Vec::new();
    for role in [Role::Swarmer, Role::Blob, Role::Ranged, Role::Bomber, Role::Tank, Role::Support, Role::Charger] {
        let mut members: Vec<MobKind> = MobKind::SPAWNABLE.into_iter().filter(|k| k.role() == role).collect();
        rng.shuffle(&mut members);
        let mut picked: Vec<MobKind> = members.iter().copied().take(role.roster_size()).collect();
        let early = |k: &MobKind| k.def().unlock_minute <= 1.0;
        if matches!(role, Role::Swarmer | Role::Blob | Role::Ranged)
            && !picked.iter().any(early)
            && let Some(e) = members.iter().copied().find(early)
        {
            picked[0] = e;
        }
        roster.extend(picked);
    }
    roster
}

/// The bosses of a run, in order (all different until each one came once).
pub fn roll_bosses(rng: &mut Rng) -> Vec<MobKind> {
    let mut bosses = MobKind::BOSSES.to_vec();
    rng.shuffle(&mut bosses);
    bosses
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawnable_mobs_have_costs_and_rewards() {
        for kind in MobKind::SPAWNABLE {
            let d = kind.def();
            assert!(d.cost > 0.0, "{}", d.name);
            assert!(d.gold > 0.0);
            assert!(d.hp > 0.0);
            assert!(!kind.is_boss());
            assert!(!kind.hint().is_empty(), "{}", d.name);
        }
        for boss in MobKind::BOSSES {
            assert!(boss.is_boss());
            assert!(boss.def().hp > 2000.0);
            assert_eq!(boss.role(), Role::Boss);
        }
    }

    #[test]
    fn tankier_mobs_pay_more() {
        assert!(MobKind::Brute.def().gold > MobKind::Skitter.def().gold);
        assert!(MobKind::Colossus.def().hp > MobKind::Brute.def().hp);
    }

    #[test]
    fn rosters_vary_and_always_have_early_monsters() {
        let mut seen = std::collections::HashSet::new();
        for seed in 0..40 {
            let roster = roll_roster(&mut Rng::new(seed));
            assert_eq!(roster.len(), 13);
            for role in [Role::Swarmer, Role::Blob, Role::Ranged] {
                assert!(roster.iter().any(|k| k.role() == role && k.def().unlock_minute <= 1.0), "{role:?}");
            }
            assert!(!roster.iter().any(|k| k.is_boss()));
            seen.extend(roster);
        }
        // Over many runs every regular monster shows up.
        assert_eq!(seen.len(), MobKind::SPAWNABLE.len());
    }

    #[test]
    fn bosses_are_shuffled() {
        let a = roll_bosses(&mut Rng::new(1));
        let b = roll_bosses(&mut Rng::new(2));
        assert_eq!(a.len(), 6);
        assert!(a != b || roll_bosses(&mut Rng::new(3)) != a);
    }

    #[test]
    fn zones_follow_the_facing() {
        // The scarab's soft back is behind it: facing -Z (yaw 0) => back at +Z.
        let back = MobKind::Scarab.zones()[1];
        let c = zone_center(&back, Vec3::ZERO, 0.0, 1.0);
        assert!(c.z > 0.0);
        // Turned around (yaw = PI), the back is at -Z.
        let c = zone_center(&back, Vec3::ZERO, std::f32::consts::PI, 1.0);
        assert!(c.z < 0.0);
    }

    #[test]
    fn ray_sphere_hits_in_front_only() {
        let t = ray_sphere(Vec3::ZERO, Vec3::NEG_Z, Vec3::new(0.0, 0.0, -5.0), 1.0).unwrap();
        assert!((t - 4.0).abs() < 1e-4);
        assert!(ray_sphere(Vec3::ZERO, Vec3::NEG_Z, Vec3::new(0.0, 0.0, 5.0), 1.0).is_none());
        assert!(ray_sphere(Vec3::ZERO, Vec3::NEG_Z, Vec3::new(3.0, 0.0, -5.0), 1.0).is_none());
    }
}
