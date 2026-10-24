//! Monster definitions (stats only — models live in `render::models`).

use serde::{Deserialize, Serialize};

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
}

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

impl MobKind {
    pub const SPAWNABLE: [MobKind; 8] = [
        MobKind::Skitter,
        MobKind::Spitter,
        MobKind::Wisp,
        MobKind::Bomber,
        MobKind::Brute,
        MobKind::Charger,
        MobKind::Jelly,
        MobKind::Shaman,
    ];
    pub const BOSSES: [MobKind; 3] = [MobKind::Colossus, MobKind::HiveQueen, MobKind::StormTitan];

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
        }
    }

    pub fn is_ghost(self) -> bool {
        matches!(self, MobKind::WhiteGhost | MobKind::BlackGhost)
    }

    pub fn is_boss(self) -> bool {
        Self::BOSSES.contains(&self)
    }
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
        }
        for boss in MobKind::BOSSES {
            assert!(boss.is_boss());
            assert!(boss.def().hp > 2000.0);
        }
    }

    #[test]
    fn tankier_mobs_pay_more() {
        assert!(MobKind::Brute.def().gold > MobKind::Skitter.def().gold);
        assert!(MobKind::Colossus.def().hp > MobKind::Brute.def().hp);
    }
}
