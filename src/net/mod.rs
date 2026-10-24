//! Networking: who we are (host / client / dedicated), starting the server,
//! connecting, and the client side of the netcode (prediction, interpolation).
//!
//! Transport: **iroh** (QUIC peer-to-peer, dialed by public key). It punches
//! through home routers (NAT traversal) and falls back to free public relay
//! servers when a direct path is impossible: no port to open, no account, no
//! VPN. Friends find the host through the public game list (`discovery`).

pub mod client;
pub mod discovery;
pub mod interp;
pub mod protocol;

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::str::FromStr;
use crossbeam_channel::{unbounded as channel, Receiver};
use std::time::Duration;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_iroh::iroh::endpoint::{presets, EndpointHooks};
use bevy_replicon_iroh::iroh::{EndpointAddr, EndpointId, SecretKey};
use bevy_replicon_iroh::{IrohClient, IrohClientConfig, IrohServer, IrohServerConfig, IrohTokioHandle};

use crate::config::{AppConfig, LaunchMode};
use discovery::Discovery;
use protocol::{Hello, Welcome, PROTOCOL_VERSION};

/// Our role in the network.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NetRole {
    #[default]
    Offline,
    /// Listen server: server + local player in the same process.
    Host,
    /// Remote client.
    Client,
    /// Server without a local player.
    Dedicated,
}

impl NetRole {
    pub fn is_authority(self) -> bool {
        matches!(self, NetRole::Host | NetRole::Dedicated)
    }

    pub fn has_local_player(self) -> bool {
        matches!(self, NetRole::Host | NetRole::Client)
    }
}

/// Run condition: are we the server?
pub fn is_authority(role: Res<NetRole>) -> bool {
    role.is_authority()
}

/// Our own player slot, once known.
#[derive(Resource, Default, Debug)]
pub struct LocalPlayer {
    pub slot: Option<u8>,
}

/// Who to join.
#[derive(Clone, Debug)]
pub enum JoinTarget {
    /// Full address from the public game list (fastest: includes relay + IPs).
    Addr(EndpointAddr),
    /// A host code typed or pasted by hand (the host's public key).
    Code(String),
    /// Join the first game found in the public list (bots, tests).
    FirstListed,
}

/// Requests coming from the UI (or the command line).
#[derive(Message, Clone, Debug)]
pub enum NetCommand {
    Host,
    Join(JoinTarget),
    Leave,
}

/// Last network error / progress, displayed by the menu.
#[derive(Resource, Default, Debug)]
pub struct NetStatus {
    pub error: Option<String>,
    pub connecting: bool,
    /// The code friends can type to join us (our public key), once hosting.
    pub host_code: Option<String>,
}

/// The tokio runtime iroh runs on. Must stay alive as long as the app.
#[derive(Resource)]
pub struct TokioRuntime(pub tokio::runtime::Runtime);

/// Results of background network operations (they can take seconds: NAT
/// traversal, relay connection...) so the game never freezes.
#[derive(Resource, Default)]
struct Pending {
    connect: Option<Receiver<Result<IrohClient, String>>>,
    host_addr: Option<Receiver<EndpointAddr>>,
    join_first: bool,
}

/// iroh needs "hooks" (callbacks on connections); we don't use any.
#[derive(Debug, Clone, Copy)]
struct NoHooks;
impl EndpointHooks for NoHooks {}

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetRole>()
            .init_resource::<LocalPlayer>()
            .init_resource::<NetStatus>()
            .init_resource::<Pending>()
            .add_message::<NetCommand>()
            .add_systems(Startup, auto_launch)
            .add_systems(Update, (handle_net_commands, poll_pending).chain())
            .add_systems(OnEnter(ClientState::Connected), send_hello)
            .add_systems(OnExit(ClientState::Connected), on_connection_lost)
            .add_systems(PreUpdate, receive_welcome.after(ClientSystems::Receive))
            .add_systems(Last, close_on_exit)
            .add_plugins((client::ClientNetPlugin, interp::InterpPlugin, discovery::DiscoveryPlugin));
    }
}

/// Applies the launch mode chosen on the command line.
fn auto_launch(config: Res<AppConfig>, mut commands: MessageWriter<NetCommand>) {
    match config.launch {
        LaunchMode::Menu => {}
        LaunchMode::Host | LaunchMode::Dedicated => {
            commands.write(NetCommand::Host);
        }
        LaunchMode::Join | LaunchMode::BotClient => {
            let target = if config.last_address.trim().is_empty() {
                JoinTarget::FirstListed
            } else {
                JoinTarget::Code(config.last_address.clone())
            };
            commands.write(NetCommand::Join(target));
        }
    }
}

/// Parses a host code (iroh public key). Spaces and line breaks are ignored
/// so a code pasted from a chat still works.
pub fn parse_code(code: &str) -> Result<EndpointId, String> {
    let cleaned: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    EndpointId::from_str(&cleaned).map_err(|_| "Code de partie invalide".to_string())
}

fn any_addr(port: u16) -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port))
}

#[allow(clippy::too_many_arguments)]
fn handle_net_commands(
    mut commands_in: MessageReader<NetCommand>,
    mut commands: Commands,
    mut role: ResMut<NetRole>,
    mut status: ResMut<NetStatus>,
    mut local: ResMut<LocalPlayer>,
    mut pending: ResMut<Pending>,
    mut discovery: ResMut<Discovery>,
    config: Res<AppConfig>,
    handle: Res<IrohTokioHandle>,
) {
    for cmd in commands_in.read() {
        match cmd {
            NetCommand::Host => {
                // A fresh identity each time we host: the code changes, but
                // friends simply click the game in the list.
                let result = IrohServer::bind_with(
                    &handle,
                    presets::N0,
                    SecretKey::generate(),
                    any_addr(config.port),
                    Vec::new(),
                    NoHooks,
                    IrohServerConfig::default(),
                )
                // Port busy (another instance)? Let the OS pick one.
                .or_else(|_| {
                    IrohServer::bind_with(
                        &handle,
                        presets::N0,
                        SecretKey::generate(),
                        any_addr(0),
                        Vec::new(),
                        NoHooks,
                        IrohServerConfig::default(),
                    )
                });
                match result {
                    Ok(server) => {
                        *role = if config.launch == LaunchMode::Dedicated { NetRole::Dedicated } else { NetRole::Host };
                        status.error = None;
                        status.host_code = Some(server.id().to_string());
                        info!("hosting: code {} on {:?}", server.id(), server.bound_sockets());
                        // Wait (in the background) until a relay is reachable,
                        // then publish our full address in the game list.
                        let endpoint = server.endpoint().clone();
                        let (tx, rx) = channel();
                        handle.0.spawn(async move {
                            if tokio::time::timeout(Duration::from_secs(8), endpoint.online()).await.is_err() {
                                warn!("no relay reachable yet: only direct connections will work");
                            }
                            let _ = tx.send(endpoint.addr());
                        });
                        pending.host_addr = Some(rx);
                        commands.insert_resource(server);
                    }
                    Err(e) => {
                        error!("cannot host: {e:#}");
                        status.error = Some(format!("Impossible d'héberger : {e}"));
                    }
                }
            }
            NetCommand::Join(target) => {
                let addr = match target {
                    JoinTarget::Addr(a) => a.clone(),
                    JoinTarget::Code(code) => match parse_code(code) {
                        // Only the key: iroh finds the host through its public
                        // address lookup service.
                        Ok(id) => EndpointAddr::new(id),
                        Err(e) => {
                            status.error = Some(e);
                            continue;
                        }
                    },
                    JoinTarget::FirstListed => {
                        pending.join_first = true;
                        *role = NetRole::Client;
                        status.connecting = true;
                        status.error = None;
                        info!("waiting for a game in the public list...");
                        continue;
                    }
                };
                start_connect(&handle, addr, &mut pending);
                *role = NetRole::Client;
                status.connecting = true;
                status.error = None;
            }
            NetCommand::Leave => {
                info!("leaving the game");
                discovery.withdraw();
                *role = NetRole::Offline;
                local.slot = None;
                status.connecting = false;
                status.host_code = None;
                pending.connect = None;
                pending.host_addr = None;
                pending.join_first = false;
                // Close the connection in the background (it may wait a bit
                // for a clean goodbye) so the game doesn't freeze.
                commands.queue(|world: &mut World| {
                    let handle = world.resource::<IrohTokioHandle>().clone();
                    if let Some(server) = world.remove_resource::<IrohServer>() {
                        let h = handle.clone();
                        std::thread::spawn(move || server.shutdown(&h));
                    }
                    if let Some(client) = world.remove_resource::<IrohClient>() {
                        std::thread::spawn(move || client.disconnect(&handle));
                    }
                });
                // Server-side entities must be cleaned by us.
                commands.run_system_cached(crate::game::lobby::despawn_all_game_entities);
            }
        }
    }
}

fn start_connect(handle: &IrohTokioHandle, addr: EndpointAddr, pending: &mut Pending) {
    info!("connecting to {}", addr.id);
    let handle = handle.clone();
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let result = IrohClient::connect_with(
            &handle,
            presets::N0,
            SecretKey::generate(),
            any_addr(0),
            addr,
            Vec::new(),
            NoHooks,
            IrohClientConfig { connect_timeout: Duration::from_secs(25), ..default() },
        )
        .map_err(|e| format!("{e:#}"));
        let _ = tx.send(result);
    });
    pending.connect = Some(rx);
}

/// Picks up the results of background operations.
fn poll_pending(
    mut commands: Commands,
    mut pending: ResMut<Pending>,
    mut status: ResMut<NetStatus>,
    mut role: ResMut<NetRole>,
    mut discovery: ResMut<Discovery>,
    handle: Res<IrohTokioHandle>,
) {
    if let Some(rx) = &pending.host_addr
        && let Ok(addr) = rx.try_recv()
    {
        info!("server online: relay {:?}, {} direct address(es)", addr.relay_urls().next(), addr.ip_addrs().count());
        discovery.my_addr = Some(addr);
        pending.host_addr = None;
    }
    if pending.join_first {
        if let Some((_, ad)) = discovery.open_games().into_iter().find(|(_, ad)| ad.players < ad.max_players) {
            info!("joining listed game '{}'", ad.name);
            pending.join_first = false;
            start_connect(&handle, ad.addr, &mut pending);
        }
        return;
    }
    if let Some(rx) = &pending.connect
        && let Ok(result) = rx.try_recv()
    {
        pending.connect = None;
        status.connecting = false;
        if *role != NetRole::Client {
            // The player cancelled while we were connecting.
            if let Ok(client) = result {
                let h = handle.clone();
                std::thread::spawn(move || client.disconnect(&h));
            }
            return;
        }
        match result {
            Ok(client) => {
                info!("connected to the host");
                commands.insert_resource(client);
            }
            Err(e) => {
                warn!("connection failed: {e}");
                status.error = Some(
                    "Impossible de rejoindre la partie (l'hôte est-il toujours en ligne ? partie pleine ?)".into(),
                );
                *role = NetRole::Offline;
            }
        }
    }
}

/// On quit, close the connection cleanly (tells the other side right away
/// instead of letting it time out). Exclusive system: needs the whole world.
fn close_on_exit(world: &mut World) {
    let quitting = world.resource::<Messages<AppExit>>().iter_current_update_messages().next().is_some();
    if !quitting {
        return;
    }
    let handle = world.resource::<IrohTokioHandle>().clone();
    if let Some(server) = world.remove_resource::<IrohServer>() {
        server.shutdown(&handle);
    }
    if let Some(client) = world.remove_resource::<IrohClient>() {
        client.disconnect(&handle);
    }
}

fn send_hello(mut hello: MessageWriter<Hello>, config: Res<AppConfig>) {
    info!("connected, sending hello as '{}'", config.name);
    hello.write(Hello {
        name: config.name.clone(),
        version: PROTOCOL_VERSION,
        main: config.main_weapon,
        autos: config.auto_pool(),
    });
}

fn receive_welcome(
    mut welcomes: MessageReader<Welcome>,
    mut local: ResMut<LocalPlayer>,
    mut status: ResMut<NetStatus>,
    mut commands: MessageWriter<NetCommand>,
) {
    for w in welcomes.read() {
        if w.accepted {
            info!("welcome! we are player slot {}", w.slot);
            local.slot = Some(w.slot);
        } else {
            warn!("refused by server: {}", w.reason);
            status.error = Some(format!("Refusé par l'hôte : {}", w.reason));
            commands.write(NetCommand::Leave);
        }
    }
}

fn on_connection_lost(role: Res<NetRole>, mut status: ResMut<NetStatus>, mut commands: MessageWriter<NetCommand>) {
    if *role == NetRole::Client {
        warn!("connection to the host lost");
        status.error = Some("Connexion à l'hôte perdue.".into());
        commands.write(NetCommand::Leave);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_host_codes() {
        let id = SecretKey::generate().public();
        let code = id.to_string();
        assert_eq!(parse_code(&code).unwrap(), id);
        // Pasted with spaces / line breaks.
        let messy = format!(" {}\n{} ", &code[..10], &code[10..]);
        assert_eq!(parse_code(&messy).unwrap(), id);
        assert!(parse_code("pas un code").is_err());
    }
}
