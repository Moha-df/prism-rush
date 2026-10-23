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
    /// Seconds into the run before it can spawn.
    pub unlock_time: f32,
    pub flying: bool,
    pub ranged: bool,
}

impl MobKind {
    pub const SPAWNABLE: [MobKind; 5] =
        [MobKind::Skitter, MobKind::Spitter, MobKind::Wisp, MobKind::Bomber, MobKind::Brute];

    pub fn def(self) -> MobDef {
        match self {
            MobKind::Skitter => MobDef {
                name: "Trotteur",
                hp: 45.0,
                speed: 8.5,
                radius: 0.5,
                height: 0.8,
                damage: 7.0,
                attack_range: 1.6,
                attack_cooldown: 0.9,
                windup: 0.25,
                cost: 1.0,
                gold: 3.0,
                unlock_time: 0.0,
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
                unlock_time: 45.0,
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
                unlock_time: 100.0,
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
                unlock_time: 160.0,
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
                unlock_time: 210.0,
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
                unlock_time: 0.0,
                flying: false,
                ranged: true,
            },
        }
    }

    pub fn is_boss(self) -> bool {
        self == MobKind::Colossus
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
        assert!(MobKind::Colossus.is_boss());
    }

    #[test]
    fn tankier_mobs_pay_more() {
        assert!(MobKind::Brute.def().gold > MobKind::Skitter.def().gold);
        assert!(MobKind::Colossus.def().hp > MobKind::Brute.def().hp);
    }
}
