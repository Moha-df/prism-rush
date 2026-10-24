//! Lobby and match lifecycle (server side): players joining/leaving,
//! starting a run, game over, going back to the lobby.

use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use bevy_replicon::prelude::*;

use super::chests::ChestRegistry;
use super::mobs::Director;
use super::players::{InputQueue, Intent, Owner, PlayerScore};
pub use crate::net::protocol::PendingChoice;
use super::world::TerrainRes;
use super::SimTick;
use crate::bot::BotBrain;
use crate::config::AppConfig;
use crate::net::protocol::*;
use crate::net::{LocalPlayer, NetRole};
use crate::shared::build::{Loadout, MainWeapon};
use crate::shared::economy;
use crate::shared::items::{Inventory, PlayerStats};
use crate::shared::sim::PlayerSim;
use crate::shared::worldgen::Terrain;

pub struct LobbyPlugin;

impl Plugin for LobbyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameOverTimer>()
            .add_systems(OnEnter(ServerState::Running), setup_server_match)
            .add_systems(
                PreUpdate,
                (handle_hello, handle_lobby_commands, handle_pick_main).after(ServerSystems::Receive).run_if(in_state(ServerState::Running)),
            )
            .add_systems(
                FixedUpdate,
                (advance_match_clock, check_game_over).chain().in_set(super::ServerSet::World),
            )
            .add_systems(Update, dedicated_automation.run_if(in_state(ServerState::Running)))
            .add_systems(Update, quit_after_duration)
            .add_observer(on_client_disconnected);
    }
}

/// For dedicated servers: time spent in game over before restarting.
#[derive(Resource, Default)]
struct GameOverTimer(f32);

/// Creates the match entity and the host's own player (and bots).
fn setup_server_match(
    mut commands: Commands,
    role: Res<NetRole>,
    config: Res<AppConfig>,
    mut local: ResMut<LocalPlayer>,
    existing: Query<Entity, With<MatchInfo>>,
) {
    if !existing.is_empty() {
        return;
    }
    commands.spawn((
        Replicated,
        MatchInfo {
            phase: Phase::Lobby,
            seed: 0,
            elapsed: 0.0,
            difficulty: 1.0,
            level: 1,
            tick: 0,
            boss_alive: false,
            team_level: 1,
            xp: 0.0,
            xp_next: economy::xp_to_next(1),
            swarm: 0,
            enemies_left: 0,
        },
    ));
    let mut slot = 0u8;
    if *role == NetRole::Host {
        commands.spawn((
            player_bundle(slot, config.name.clone(), false, Owner { client: Some(ClientId::Server) }, config.main_weapon),
            super::progress::AutoPool(config.auto_pool()),
        ));
        local.slot = Some(slot);
        slot += 1;
    }
    for i in 0..config.bots.min(MAX_PLAYERS as u8 - slot) {
        let name = format!("Bot {}", ["Alpha", "Bravo", "Charlie", "Delta"][i as usize % 4]);
        // Bots take a starter weapon each (variety).
        let starters: Vec<MainWeapon> = MainWeapon::ALL.into_iter().filter(|w| w.def().starter).collect();
        let main = starters[i as usize % starters.len()];
        commands.spawn((
            player_bundle(slot, name, true, Owner { client: None }, main),
            super::progress::AutoPool::default(),
            BotBrain::default(),
        ));
        slot += 1;
    }
    info!("lobby open ({slot} players including bots)");
}

fn player_bundle(slot: u8, name: String, is_bot: bool, owner: Owner, main: MainWeapon) -> impl Bundle {
    (
        Replicated,
        Player { slot, name, is_bot, main },
        owner,
        InputQueue::default(),
        Intent::default(),
        PlayerScore::default(),
        super::players::HurtClock::default(),
        super::progress::LevelUps::default(),
    )
}

fn free_slot(players: &Query<&Player>) -> Option<u8> {
    (0..MAX_PLAYERS as u8).find(|s| !players.iter().any(|p| p.slot == *s))
}

/// Spawn point for a slot: a small circle around the origin.
pub fn spawn_point(terrain: &Terrain, slot: u8) -> Vec3 {
    let angle = f32::from(slot) * std::f32::consts::FRAC_PI_2;
    let x = angle.cos() * 3.0;
    let z = angle.sin() * 3.0;
    Vec3::new(x, terrain.wgen.ground_height(x, z) + 0.1, z)
}

/// Components a player needs during a run.
fn run_components(terrain: &Terrain, slot: u8, main: MainWeapon) -> impl Bundle {
    let stats = PlayerStats::from_inventory(&Inventory::default());
    (
        PlayerState { sim: PlayerSim::spawn_at(spawn_point(terrain, slot)), tick: 0, yaw: 0.0, pitch: 0.0 },
        Gear(Loadout::new(main)),
        super::autos::AutoState::default(),
        Health::new(stats.max_hp),
        Wallet::default(),
        Items::default(),
    )
}

fn handle_hello(
    mut hellos: MessageReader<FromClient<Hello>>,
    mut commands: Commands,
    players: Query<&Player>,
    info: Single<&MatchInfo>,
    terrain: Res<TerrainRes>,
    mut welcome: MessageWriter<ToClients<Welcome>>,
) {
    for hello in hellos.read() {
        let client_id = hello.client_id;
        let refuse = |reason: &str, welcome: &mut MessageWriter<ToClients<Welcome>>| {
            welcome.write(ToClients {
                targets: SendTargets::Single(client_id),
                message: Welcome { slot: 0, accepted: false, reason: reason.into() },
            });
        };
        if hello.version != PROTOCOL_VERSION {
            refuse("version du jeu différente", &mut welcome);
            continue;
        }
        let Some(slot) = free_slot(&players) else {
            refuse("partie pleine (4 joueurs max)", &mut welcome);
            continue;
        };
        let name: String = hello.name.chars().filter(|c| !c.is_control()).take(16).collect();
        let name = if name.trim().is_empty() { format!("Joueur {}", slot + 1) } else { name };
        info!("player '{name}' joined as slot {slot} ({client_id})");
        let main = hello.main;
        // Only automatic weapons that exist are kept (and at least the starters).
        let mut pool = super::progress::AutoPool::default();
        for a in &hello.autos {
            if !pool.0.contains(a) {
                pool.0.push(*a);
            }
        }
        let mut entity = commands.spawn((player_bundle(slot, name, false, Owner { client: Some(client_id) }, main), pool));
        // Join in progress: drop the newcomer into the world right away.
        if info.phase == Phase::Playing {
            entity.insert(run_components(&terrain, slot, main));
        }
        welcome.write(ToClients {
            targets: SendTargets::Single(client_id),
            message: Welcome { slot, accepted: true, reason: String::new() },
        });
    }
}

fn on_client_disconnected(
    remove: On<Remove<ConnectedClient>>,
    mut commands: Commands,
    players: Query<(Entity, &Owner, &Player)>,
) {
    let id = ClientId::Client(remove.entity);
    for (entity, owner, player) in &players {
        if owner.client == Some(id) {
            info!("player '{}' left", player.name);
            commands.entity(entity).try_despawn();
        }
    }
}

fn handle_lobby_commands(
    mut cmds: MessageReader<FromClient<LobbyCommand>>,
    role: Res<NetRole>,
    mut commands: Commands,
) {
    for cmd in cmds.read() {
        // On a listen server only the host decides; on a dedicated server anyone can.
        if *role == NetRole::Host && cmd.client_id != ClientId::Server {
            warn!("{} tried a lobby command but is not the host", cmd.client_id);
            continue;
        }
        match cmd.message {
            LobbyCommand::Start => commands.run_system_cached(start_match),
            LobbyCommand::BackToLobby => commands.run_system_cached(back_to_lobby),
        }
    }
}

/// A player changes their starting weapon (or unlocked weapons) in the lobby.
fn handle_pick_main(
    mut picks: MessageReader<FromClient<PickLoadout>>,
    mut players: Query<(&Owner, &mut Player, &mut super::progress::AutoPool)>,
) {
    for pick in picks.read() {
        if let Some((_, mut player, mut pool)) = players.iter_mut().find(|(o, ..)| o.client == Some(pick.client_id)) {
            player.main = pick.message.main;
            for a in &pick.message.autos {
                if !pool.0.contains(a) {
                    pool.0.push(*a);
                }
            }
        }
    }
}

/// Starts a new run: new seed, everyone spawns, monsters reset.
#[allow(clippy::too_many_arguments)]
pub fn start_match(
    mut commands: Commands,
    mut info: Single<&mut MatchInfo>,
    players: Query<(Entity, &Player)>,
    mut terrain: ResMut<TerrainRes>,
    mut director: ResMut<Director>,
    mut chests: ResMut<ChestRegistry>,
    tick: Res<SimTick>,
    config: Res<AppConfig>,
) {
    if info.phase == Phase::Playing {
        return;
    }
    commands.run_system_cached(despawn_world_entities);
    let seed = config.seed.unwrap_or_else(|| {
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1)
    });
    terrain.0 = Terrain::new(seed);
    *director = Director::default();
    // Bosses whose time is already past (debug --start-time) are skipped.
    director.next_boss = crate::shared::economy::BOSS_TIMES.iter().filter(|t| **t <= config.start_time).count();
    *chests = ChestRegistry::default();

    info.phase = Phase::Playing;
    info.seed = seed;
    info.elapsed = config.start_time;
    info.difficulty = 1.0;
    info.level = 1;
    info.tick = tick.0;
    info.boss_alive = false;
    info.team_level = 1;
    info.xp = 0.0;
    info.xp_next = economy::xp_to_next(1);
    info.swarm = 0;
    info.enemies_left = 0;

    for (entity, player) in &players {
        commands
            .entity(entity)
            .insert(run_components(&terrain, player.slot, player.main))
            .insert((PlayerScore::default(), InputQueue::default(), super::progress::LevelUps::default()));
    }
    info!("=== run started (seed {seed}, {} players) ===", players.iter().count());
}

/// Back to the lobby after a game over.
pub fn back_to_lobby(mut commands: Commands, mut info: Single<&mut MatchInfo>, players: Query<Entity, With<Player>>) {
    if info.phase == Phase::Lobby {
        return;
    }
    commands.run_system_cached(despawn_world_entities);
    for e in &players {
        commands.entity(e).remove::<(PlayerState, Health, Wallet, Items, PendingChoice, Gear)>();
    }
    info.phase = Phase::Lobby;
    info!("back to lobby");
}

/// Despawns monsters, chests and projectiles (the run's world).
pub fn despawn_world_entities(
    mut commands: Commands,
    q: Query<Entity, Or<(With<Mob>, With<Chest>, With<Projectile>, With<Hazard>, With<Shrine>, With<Powerup>, With<Statue>, With<XpGem>)>>,
) {
    for e in &q {
        commands.entity(e).try_despawn();
    }
}

/// Removes every networked entity (when leaving a game).
pub fn despawn_all_game_entities(
    mut commands: Commands,
    q: Query<Entity, Or<(With<Replicated>, With<Remote>)>>,
) {
    for e in &q {
        commands.entity(e).try_despawn();
    }
}

fn advance_match_clock(
    mut info: Single<&mut MatchInfo>,
    mut tick: ResMut<SimTick>,
    time: Res<Time>,
    players: Query<&Player, With<PlayerState>>,
) {
    tick.0 += 1;
    let n = players.iter().count() as u32;
    info.elapsed += time.delta_secs();
    info.difficulty = economy::time_difficulty(info.elapsed, n);
    info.level = economy::time_monster_level(info.elapsed);
    info.tick = tick.0;
}

/// Everyone down => game over, with a summary for the end screen.
fn check_game_over(
    mut info: Single<&mut MatchInfo>,
    players: Query<(&Player, &Health, &PlayerScore, &Items)>,
    mut events: MessageWriter<ToClients<GameEvent>>,
) {
    let mut any = false;
    let mut all_down = true;
    for (_, health, _, _) in &players {
        any = true;
        all_down &= health.downed;
    }
    if !any || !all_down {
        return;
    }
    info.phase = Phase::GameOver;
    let mut summary = RunSummary { duration: info.elapsed, level: info.team_level, players: Vec::new() };
    for (player, _, score, items) in &players {
        summary.players.push(PlayerSummary {
            slot: player.slot,
            name: player.name.clone(),
            kills: score.kills,
            damage: score.damage,
            gold_earned: score.gold_earned,
            chests: score.chests,
            revives: score.revives,
            downs: score.downs,
            items: items.0.total(),
        });
    }
    summary.players.sort_by_key(|p| p.slot);
    info!(
        "=== GAME OVER after {:.0}s, team level {} — {} ===",
        summary.duration,
        summary.level,
        summary
            .players
            .iter()
            .map(|p| format!("{}: {} kills, {:.0} dmg, {} gold, {} chests", p.name, p.kills, p.damage, p.gold_earned, p.chests))
            .collect::<Vec<_>>()
            .join(" | ")
    );
    events.write(ToClients { targets: SendTargets::All, message: GameEvent::GameOver(Box::new(summary)) });
}

/// Dedicated server conveniences: autostart, and restart after a game over.
fn dedicated_automation(
    role: Res<NetRole>,
    config: Res<AppConfig>,
    info: Query<&MatchInfo>,
    players: Query<&Player>,
    mut timer: ResMut<GameOverTimer>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let Ok(info) = info.single() else { return };
    let autostart = config.autostart && (*role == NetRole::Dedicated || *role == NetRole::Host);
    match info.phase {
        Phase::Lobby if autostart && !players.is_empty() => commands.run_system_cached(start_match),
        Phase::GameOver if *role == NetRole::Dedicated => {
            timer.0 += time.delta_secs();
            if timer.0 > 10.0 {
                timer.0 = 0.0;
                commands.run_system_cached(back_to_lobby);
            }
        }
        _ => {}
    }
}

fn quit_after_duration(config: Res<AppConfig>, time: Res<Time<Real>>, mut exit: MessageWriter<AppExit>) {
    if let Some(d) = config.duration
        && time.elapsed_secs() > d
    {
        info!("--duration reached, exiting");
        exit.write(AppExit::Success);
    }
}
