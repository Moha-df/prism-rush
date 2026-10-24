//! Network protocol: every replicated component and every message.
//!
//! Replicon requires the registration order to be identical on server and
//! clients, which is guaranteed here because both call `register_protocol`.

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::shared::economy::ChestKind;
use crate::shared::items::{Inventory, ItemKind};
use crate::shared::mobs::MobKind;
use crate::shared::movement::PlayerInput;
use crate::shared::sim::PlayerSim;
use crate::shared::weapons::WeaponKind;

pub const DEFAULT_PORT: u16 = 7777;
pub const MAX_PLAYERS: usize = 4;
/// Protocol version: bump when messages change, so old clients are refused.
pub const PROTOCOL_VERSION: u32 = 2;

// ---------------------------------------------------------------------------
// Replicated components
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Lobby,
    Playing,
    GameOver,
}

/// Global state of the match, on a single replicated entity.
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct MatchInfo {
    pub phase: Phase,
    pub seed: u64,
    /// Seconds since the run started.
    pub elapsed: f32,
    pub difficulty: f32,
    pub level: u32,
    /// Server simulation tick (60 Hz), used by clients to sync their clock.
    pub tick: u32,
    pub boss_alive: bool,
    /// Team level (everyone levels up together) and XP progress.
    pub team_level: u32,
    pub xp: f32,
    pub xp_next: f32,
    /// Final Swarm: 0 = not yet, 1 = white ghosts, 2 = black ghosts too.
    pub swarm: u8,
    /// Monsters alive right now.
    pub enemies_left: u16,
}

/// Identity of a player: slot (0..4, also picks the color) and name.
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub slot: u8,
    pub name: String,
    pub is_bot: bool,
}

/// Authoritative simulation state of a player (+ tick it was produced at).
#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlayerState {
    pub sim: PlayerSim,
    pub tick: u32,
    /// Look angles, for remote players' models.
    pub yaw: f32,
    pub pitch: f32,
}

/// Health for players and monsters.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Health {
    pub hp: f32,
    pub max: f32,
    /// Players only: knocked down, waiting for a revive.
    pub downed: bool,
    /// Revive progress in [0, 1].
    pub revive: f32,
    /// Players only: energy shield absorbing damage before health.
    pub shield: f32,
    pub shield_max: f32,
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self { hp: max, max, downed: false, revive: 0.0, shield: 0.0, shield_max: 0.0 }
    }
}

/// Cards offered by an opened chest, waiting for the player's choice.
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct PendingChoice {
    pub chest: ChestKind,
    pub options: Vec<ItemKind>,
}

#[derive(Component, Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Wallet {
    pub gold: u32,
    /// Chests this player opened: each one makes the next more expensive.
    pub chests_opened: u32,
}

/// An XP gem dropped by a monster. Walk near it to collect it (the whole
/// team gains the XP).
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct XpGem {
    pub pos: Vec3,
    pub value: f32,
}

#[derive(Component, Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Items(pub Inventory);

/// Monster identity (set once).
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Mob {
    pub kind: MobKind,
    pub level: u32,
    /// Elite: bigger, tougher, glowing, pays more.
    pub elite: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HazardKind {
    /// Expanding ring on the ground: jump over it!
    Shockwave,
    /// Marked spot hit by lightning after a delay.
    Lightning,
}

/// A boss attack area, replicated once; clients animate it from its spawn tick.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Hazard {
    pub kind: HazardKind,
    pub pos: Vec3,
    pub spawn_tick: u32,
}

pub const SHOCKWAVE_SPEED: f32 = 13.0;
pub const SHOCKWAVE_MAX_RADIUS: f32 = 38.0;
pub const LIGHTNING_DELAY: f32 = 1.2;
pub const LIGHTNING_RADIUS: f32 = 3.2;

/// A boss summoning statue found in the world.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Statue {
    pub spot_id: u64,
    pub pos: Vec3,
    pub boss: MobKind,
}

/// Mutable statue state.
#[derive(Component, Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct StatueUsed(pub bool);

/// What a monster is doing, for animations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MobAnim {
    #[default]
    Move,
    Windup,
    Attack,
}

/// Monster state replicated every tick.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct MobState {
    pub pos: Vec3,
    pub yaw: f32,
    pub tick: u32,
    pub anim: MobAnim,
}

/// Static chest data (replicated once).
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Chest {
    pub spot_id: u64,
    pub kind: ChestKind,
    pub pos: Vec3,
    pub yaw: f32,
}

/// Mutable chest state.
#[derive(Component, Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct ChestOpen {
    pub opened: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectileKind {
    Rocket,
    Acid,
    Bolt,
    BossOrb,
    Grenade,
    Plasma,
}

/// A weapon lying on the ground (from a weapon crate, or dropped by a
/// player taking another one). Press E to take it.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct WeaponPickup {
    pub kind: WeaponKind,
    pub pos: Vec3,
}

/// Ballistic projectile: replicated once, clients compute its trajectory.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Projectile {
    pub kind: ProjectileKind,
    pub origin: Vec3,
    pub vel: Vec3,
    pub gravity: f32,
    pub spawn_tick: u32,
    /// Shooter slot (players) or 255 (monsters).
    pub owner_slot: u8,
}

impl Projectile {
    pub fn pos_at(&self, tick: f64) -> Vec3 {
        let t = ((tick - f64::from(self.spawn_tick)) as f32 / crate::shared::movement::TICK_HZ as f32).max(0.0);
        self.origin + self.vel * t + Vec3::NEG_Y * (0.5 * self.gravity * t * t)
    }
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

/// Client -> server: first message after connecting.
#[derive(Message, Clone, Debug, Serialize, Deserialize)]
pub struct Hello {
    pub name: String,
    pub version: u32,
}

/// Client -> server: the latest inputs. Sent unreliably every tick; we resend
/// the last few inputs each time so a lost packet costs nothing.
#[derive(Message, Clone, Debug, Serialize, Deserialize)]
pub struct InputBatch {
    pub inputs: Vec<PlayerInput>,
    /// The server tick the client is currently *rendering* other entities at
    /// (for lag compensation of hitscan shots).
    pub view_tick: u32,
}

/// Client -> server: pick one of the offered cards.
#[derive(Message, Clone, Debug, Serialize, Deserialize)]
pub struct ChooseCard {
    pub index: u8,
}

/// Client -> server lobby actions (only the host's are accepted).
#[derive(Message, Clone, Debug, Serialize, Deserialize)]
pub enum LobbyCommand {
    Start,
    BackToLobby,
}

/// Server -> one client: your player slot.
#[derive(Message, Clone, Debug, Serialize, Deserialize)]
pub struct Welcome {
    pub slot: u8,
    pub accepted: bool,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reward {
    Item(ItemKind),
    Weapon(WeaponKind),
}

/// Server -> clients: transient visual/audio effects. Losing one is harmless.
#[derive(Message, Clone, Debug, Serialize, Deserialize)]
pub enum Fx {
    Hit { pos: Vec3, amount: u16, crit: bool, killed: bool, by_slot: u8 },
    Tracer { slot: u8, weapon: WeaponKind, from: Vec3, to: Vec3 },
    Explosion { pos: Vec3, radius: f32 },
    MobAttack { pos: Vec3, kind: MobKind },
    PlayerHurt { slot: u8, amount: u16 },
    Gold { amount: u32 },
    /// XP gems collected by a player (visual: gems fly to them).
    XpCollected { from: Vec3, slot: u8 },
    /// A straggler monster was brought back near the players.
    Warp { from: Vec3, to: Vec3 },
    /// Electric arc between two points (Tesla coil).
    Arc { from: Vec3, to: Vec3 },
    /// A shaman healed the monsters around it.
    Heal { pos: Vec3, radius: f32 },
    /// A lightning bolt struck the ground.
    Lightning { pos: Vec3 },
}

/// Server -> clients: important events (reliable).
#[derive(Message, Clone, Debug, Serialize, Deserialize)]
pub enum GameEvent {
    ChestOpened { pos: Vec3, slot: u8, kind: ChestKind },
    /// A player got an item (card picked) or a weapon.
    Gained { slot: u8, reward: Reward },
    /// An Ankh saved a player.
    AnkhUsed { slot: u8 },
    Downed { slot: u8 },
    Revived { slot: u8 },
    BossSpawned,
    StatueAwakened { pos: Vec3 },
    BossKilled,
    /// The team reached a new level.
    LevelUp { level: u32 },
    /// The Final Swarm begins (white ghosts), or black ghosts join.
    SwarmStarted { black: bool },
    GameOver(Box<RunSummary>),
}

/// End-of-run statistics for one player.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlayerSummary {
    pub slot: u8,
    pub name: String,
    pub kills: u32,
    pub damage: f32,
    pub gold_earned: u32,
    pub chests: u32,
    pub revives: u32,
    pub downs: u32,
    pub items: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RunSummary {
    pub duration: f32,
    pub level: u32,
    pub players: Vec<PlayerSummary>,
}

/// Registers everything. Must be called in the same order on all peers.
pub fn register_protocol(app: &mut App) {
    app.replicate::<MatchInfo>()
        .replicate::<Player>()
        .replicate::<PlayerState>()
        .replicate::<Health>()
        .replicate::<Wallet>()
        .replicate::<Items>()
        .replicate::<PendingChoice>()
        .replicate_once::<Mob>()
        .replicate::<MobState>()
        .replicate_once::<Chest>()
        .replicate::<ChestOpen>()
        .replicate_once::<Projectile>()
        .replicate_once::<WeaponPickup>()
        .replicate_once::<XpGem>()
        .replicate_once::<Hazard>()
        .replicate_once::<Statue>()
        .replicate::<StatueUsed>()
        .add_client_message::<Hello>(Channel::Ordered)
        .add_client_message::<InputBatch>(Channel::Unreliable)
        .add_client_message::<LobbyCommand>(Channel::Ordered)
        .add_client_message::<ChooseCard>(Channel::Ordered)
        .add_server_message::<Welcome>(Channel::Ordered)
        .add_server_message::<Fx>(Channel::Unreliable)
        .add_server_message::<GameEvent>(Channel::Ordered);
}

/// Distinct, readable colors for the 4 player slots (sRGB).
pub const PLAYER_COLORS: [[f32; 3]; MAX_PLAYERS] = [
    [0.95, 0.42, 0.32], // coral
    [0.2, 0.75, 0.75],  // teal
    [0.98, 0.78, 0.25], // gold
    [0.62, 0.45, 0.95], // violet
];

pub fn player_color(slot: u8) -> Color {
    let c = PLAYER_COLORS[slot as usize % MAX_PLAYERS];
    Color::srgb(c[0], c[1], c[2])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::movement::buttons;

    /// Replicon serializes with postcard: make sure our types round-trip.
    fn roundtrip<T: Serialize + for<'de> Deserialize<'de>>(v: &T) -> T {
        let bytes = postcard::to_allocvec(v).expect("serialize");
        postcard::from_bytes(&bytes).expect("deserialize")
    }

    #[test]
    fn input_batch_roundtrip_and_size() {
        let batch = InputBatch {
            inputs: (0..4)
                .map(|i| PlayerInput {
                    seq: 1000 + i,
                    move_axis: Vec2::new(0.5, -1.0),
                    yaw: 1.25,
                    pitch: -0.3,
                    buttons: buttons::FIRE | buttons::JUMP,
                    select_slot: 255,
                })
                .collect(),
            view_tick: 123_456,
        };
        let bytes = postcard::to_allocvec(&batch).unwrap();
        // 4 inputs must fit comfortably in a single small UDP packet.
        assert!(bytes.len() < 140, "{} bytes", bytes.len());
        let back: InputBatch = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(back.inputs, batch.inputs);
        assert_eq!(back.view_tick, batch.view_tick);
    }

    #[test]
    fn player_state_roundtrip() {
        let mut state = PlayerState::default();
        state.sim.mv.pos = Vec3::new(1.0, 2.0, 3.0);
        state.sim.last_seq = 42;
        state.tick = 99;
        let back = roundtrip(&state);
        assert_eq!(back.sim, state.sim);
        assert_eq!(back.tick, 99);
    }

    #[test]
    fn events_roundtrip() {
        let ev = GameEvent::Gained { slot: 2, reward: Reward::Item(ItemKind::FrogLegs) };
        match roundtrip(&ev) {
            GameEvent::Gained { slot, reward } => {
                assert_eq!(slot, 2);
                assert_eq!(reward, Reward::Item(ItemKind::FrogLegs));
            }
            _ => panic!("wrong variant"),
        }
        let choice = PendingChoice { chest: ChestKind::Large, options: vec![ItemKind::Ankh, ItemKind::Crown, ItemKind::Clover] };
        assert_eq!(roundtrip(&choice).options, choice.options);
        let summary = GameEvent::GameOver(Box::new(RunSummary {
            duration: 300.0,
            level: 5,
            players: vec![PlayerSummary { name: "Zoé".into(), kills: 12, ..Default::default() }],
        }));
        match roundtrip(&summary) {
            GameEvent::GameOver(s) => assert_eq!(s.players[0].name, "Zoé"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn projectile_trajectory() {
        let p = Projectile {
            kind: ProjectileKind::Acid,
            origin: Vec3::ZERO,
            vel: Vec3::new(10.0, 10.0, 0.0),
            gravity: 10.0,
            spawn_tick: 0,
            owner_slot: 255,
        };
        let one_sec = p.pos_at(60.0);
        assert!((one_sec.x - 10.0).abs() < 1e-3);
        assert!((one_sec.y - 5.0).abs() < 1e-3);
    }
}
