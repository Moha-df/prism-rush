//! Rules of a run chosen in the lobby: difficulty level and map, plus the
//! totems activated during the run (the "curse").
//!
//! Everything here is a pure function, so balance numbers live in one place:
//! tweak the tables below to make a level easier or harder.

use serde::{Deserialize, Serialize};

use super::mobs::MobKind;

/// The 5 difficulty levels, from relaxed to hopeless.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Difficulty {
    Easy,
    #[default]
    Normal,
    Hard,
    Nightmare,
    Impossible,
}

impl Difficulty {
    pub const ALL: [Difficulty; 5] = [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard, Difficulty::Nightmare, Difficulty::Impossible];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        ["Facile", "Normal", "Difficile", "Cauchemar", "Impossible"][self.index()]
    }

    pub fn description(self) -> &'static str {
        [
            "Monstres fragiles, lents et moins nombreux : pour découvrir le jeu.",
            "L'expérience prévue : tenir 8 minutes demande un bon build.",
            "Les monstres foncent plus vite, les tireurs tirent plus souvent, ça tape plus fort.",
            "Hordes rapides et nombreuses, tirs en rafale. Pour les équipes rodées.",
            "Tout va vite, tout fait très mal. Bonne chance. Vraiment.",
        ][self.index()]
    }

    /// Monster HP multiplier. Kept between 0.5 and 1.5 on purpose: harder
    /// levels are about speed and danger, not about bullet sponges.
    pub fn hp(self) -> f32 {
        [0.5, 1.0, 1.15, 1.3, 1.5][self.index()]
    }

    /// Monster damage multiplier.
    pub fn damage(self) -> f32 {
        [0.5, 1.0, 1.4, 1.9, 2.6][self.index()]
    }

    /// Movement speed of the monsters that rush you (melee, chargers,
    /// bombers, ghosts).
    pub fn rush_speed(self) -> f32 {
        [0.85, 1.0, 1.12, 1.25, 1.4][self.index()]
    }

    /// Attack rate of the monsters that shoot (cooldown divided by this).
    pub fn fire_rate(self) -> f32 {
        [0.75, 1.0, 1.2, 1.45, 1.75][self.index()]
    }

    /// Spawn speed multiplier (more monsters, more ghosts).
    pub fn spawns(self) -> f32 {
        [0.75, 1.0, 1.2, 1.45, 1.8][self.index()]
    }

    /// Elite chance multiplier.
    pub fn elites(self) -> f32 {
        [0.5, 1.0, 1.3, 1.7, 2.2][self.index()]
    }

    /// Silver earned at the end of the run.
    pub fn silver(self) -> f32 {
        [0.5, 1.0, 1.4, 1.9, 2.6][self.index()]
    }

    pub fn harder(self) -> Self {
        Self::ALL[(self.index() + 1).min(4)]
    }

    pub fn easier(self) -> Self {
        Self::ALL[self.index().saturating_sub(1)]
    }
}

/// The 3 maps. Same generator (infinite, from the seed), different look,
/// different monsters and bosses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MapKind {
    /// Green hills at dusk (the original map).
    #[default]
    Meadow,
    /// Orange dunes, red cliffs and lava monsters.
    Embers,
    /// Dark swamp under a green fog: poison and flying horrors.
    Swamp,
}

impl MapKind {
    pub const ALL: [MapKind; 3] = [MapKind::Meadow, MapKind::Embers, MapKind::Swamp];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        ["Prairie du crépuscule", "Désert de braise", "Marais maudit"][self.index()]
    }

    pub fn description(self) -> &'static str {
        [
            "Collines vertes, araignées, gelées et colosses.",
            "Dunes brûlantes : fouisseurs, golems de pierre, magma et ver des sables.",
            "Brume toxique : poison, yeux volants, cristaux et titan de foudre.",
        ][self.index()]
    }

    /// The map after this one (unlocked by surviving until the Final Swarm here).
    pub fn next(self) -> Option<MapKind> {
        Self::ALL.get(self.index() + 1).copied()
    }

    /// Later maps are a bit faster and hit a bit harder, whatever the
    /// difficulty (same HP: see `Difficulty::hp`).
    pub fn damage(self) -> f32 {
        [1.0, 1.1, 1.2][self.index()]
    }

    pub fn rush_speed(self) -> f32 {
        [1.0, 1.04, 1.08][self.index()]
    }

    pub fn fire_rate(self) -> f32 {
        [1.0, 1.05, 1.1][self.index()]
    }

    pub fn silver(self) -> f32 {
        [1.0, 1.3, 1.6][self.index()]
    }

    /// Regular monsters that live on this map (the run's roster is drawn
    /// from them, a few per role).
    pub fn monsters(self) -> &'static [MobKind] {
        use MobKind::*;
        match self {
            MapKind::Meadow => &[
                Skitter, Flea, Scarab, // swarmers
                Jelly, MegaJelly, // blobs
                Spitter, Wisp, Eye, // ranged
                Bomber, Sporeling, // bombers
                Brute, ShieldKnight, // tanks
                Shaman, Charger,
            ],
            MapKind::Embers => &[
                Skitter, Burrower, Scarab, //
                Jelly, MagmaJelly, //
                Spitter, Mortar, Thornback, //
                Bomber, FireBat, //
                Brute, StoneGolem, //
                Summoner, LavaBoar,
            ],
            MapKind::Swamp => &[
                Skitter, Flea, Burrower, //
                Jelly, ToxicJelly, MegaJelly, //
                Spitter, Wisp, Eye, SkyJelly, //
                Sporeling, FireBat, //
                CrystalGolem, ShieldKnight, //
                Shaman, Summoner, Charger,
            ],
        }
    }

    /// Bosses of this map (in a random order each run).
    pub fn bosses(self) -> &'static [MobKind] {
        use MobKind::*;
        match self {
            MapKind::Meadow => &[Colossus, HiveQueen, Broodmother],
            MapKind::Embers => &[Sandworm, AncientGolem, Colossus],
            MapKind::Swamp => &[StormTitan, Broodmother, HiveQueen],
        }
    }
}

/// Each activated totem makes monsters this much faster, deadlier and more
/// numerous (not tougher: HP stay within 0.5-1.5)...
pub const TOTEM_DAMAGE: f32 = 0.08;
pub const TOTEM_SPAWNS: f32 = 0.10;
pub const TOTEM_SPEED: f32 = 0.03;
pub const TOTEM_FIRE_RATE: f32 = 0.05;
/// ...and gives the team this much luck (1 luck = one Clover) and rewards.
pub const TOTEM_LUCK: f32 = 0.4;
pub const TOTEM_REWARDS: f32 = 0.08;

/// Rules of the current run (replicated in `MatchInfo`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RunRules {
    pub difficulty: Difficulty,
    pub map: MapKind,
    /// Totems activated this run.
    pub totems: u32,
}

impl RunRules {
    pub fn hp_mult(&self) -> f32 {
        self.difficulty.hp()
    }

    pub fn damage_mult(&self) -> f32 {
        self.difficulty.damage() * self.map.damage() * (1.0 + TOTEM_DAMAGE * self.totems as f32)
    }

    /// Speed of the monsters that rush you (capped: past that it is unplayable).
    pub fn rush_speed_mult(&self) -> f32 {
        (self.difficulty.rush_speed() * self.map.rush_speed() * (1.0 + TOTEM_SPEED * self.totems as f32)).min(1.8)
    }

    /// Attack rate of the monsters that shoot.
    pub fn fire_rate_mult(&self) -> f32 {
        (self.difficulty.fire_rate() * self.map.fire_rate() * (1.0 + TOTEM_FIRE_RATE * self.totems as f32)).min(2.5)
    }

    pub fn spawn_mult(&self) -> f32 {
        self.difficulty.spawns() * (1.0 + TOTEM_SPAWNS * self.totems as f32)
    }

    pub fn elite_mult(&self) -> f32 {
        self.difficulty.elites()
    }

    /// Luck added to every player's rolls (chests, level-ups, shrines).
    pub fn luck_bonus(&self) -> f32 {
        TOTEM_LUCK * self.totems as f32
    }

    /// Gold and XP multiplier.
    pub fn reward_mult(&self) -> f32 {
        1.0 + TOTEM_REWARDS * self.totems as f32
    }

    pub fn silver_mult(&self) -> f32 {
        self.difficulty.silver() * self.map.silver()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::mobs::Role;

    #[test]
    fn difficulties_go_from_easy_to_impossible() {
        for pair in Difficulty::ALL.windows(2) {
            assert!(pair[1].hp() > pair[0].hp());
            assert!(pair[1].damage() > pair[0].damage());
            assert!(pair[1].spawns() > pair[0].spawns());
            assert!(pair[1].rush_speed() > pair[0].rush_speed());
            assert!(pair[1].fire_rate() > pair[0].fire_rate());
        }
        // HP always between 0.5 and 1.5, whatever the map and the totems.
        for difficulty in Difficulty::ALL {
            for map in MapKind::ALL {
                let r = RunRules { difficulty, map, totems: 20 };
                assert!((0.5..=1.5).contains(&r.hp_mult()), "{r:?}");
            }
        }
        assert_eq!(Difficulty::Impossible.harder(), Difficulty::Impossible);
        assert_eq!(Difficulty::Easy.easier(), Difficulty::Easy);
    }

    #[test]
    fn totems_trade_danger_for_luck() {
        let calm = RunRules::default();
        let cursed = RunRules { totems: 3, ..calm };
        assert!(cursed.damage_mult() > calm.damage_mult());
        assert!(cursed.rush_speed_mult() > calm.rush_speed_mult());
        assert!(cursed.fire_rate_mult() > calm.fire_rate_mult());
        assert!(cursed.spawn_mult() > calm.spawn_mult());
        assert!(cursed.luck_bonus() > calm.luck_bonus());
        assert!(cursed.reward_mult() > calm.reward_mult());
    }

    #[test]
    fn every_map_has_a_full_cast() {
        let mut all = std::collections::HashSet::new();
        for map in MapKind::ALL {
            for role in [Role::Swarmer, Role::Blob, Role::Ranged, Role::Bomber, Role::Tank, Role::Support, Role::Charger] {
                assert!(map.monsters().iter().any(|k| k.role() == role), "{map:?} {role:?}");
            }
            // Something to fight from the first second.
            assert!(map.monsters().iter().any(|k| k.def().unlock_minute <= 0.0), "{map:?}");
            assert_eq!(map.bosses().len(), 3);
            assert!(map.bosses().iter().all(|b| b.is_boss()));
            all.extend(map.monsters().iter().copied());
        }
        // Every monster lives somewhere.
        assert_eq!(all.len(), MobKind::SPAWNABLE.len());
        assert_eq!(MapKind::Meadow.next(), Some(MapKind::Embers));
        assert_eq!(MapKind::Swamp.next(), None);
    }
}
