//! Networking: who we are (host / client / dedicated), starting the server,
//! connecting, and the client side of the netcode (prediction, interpolation).

pub mod client;
pub mod interp;
pub mod protocol;

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};

use bevy::prelude::*;
use bevy_quinnet::client::{
    certificate::CertificateVerificationMode, connection::ClientAddrConfiguration,
    ClientConnectionConfiguration, ClientConnectionConfigurationDefaultables, QuinnetClient,
};
use bevy_quinnet::server::{
    certificate::CertificateRetrievalMode, EndpointAddrConfiguration, QuinnetServer,
    ServerEndpointConfiguration, ServerEndpointConfigurationDefaultables,
};
use bevy_replicon::prelude::*;
use bevy_replicon_quinnet::ChannelsConfigurationExt;

use crate::config::{AppConfig, LaunchMode};
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

/// Requests coming from the UI (or the command line).
#[derive(Message, Clone, Debug)]
pub enum NetCommand {
    Host,
    Join(String),
    Leave,
}

/// Last network error, displayed by the menu.
#[derive(Resource, Default, Debug)]
pub struct NetStatus {
    pub error: Option<String>,
    pub connecting_since: Option<f64>,
}

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetRole>()
            .init_resource::<LocalPlayer>()
            .init_resource::<NetStatus>()
            .add_message::<NetCommand>()
            .add_systems(Startup, auto_launch)
            .add_systems(Update, handle_net_commands)
            .add_systems(OnEnter(ClientState::Connected), send_hello)
            .add_systems(OnExit(ClientState::Connected), on_connection_lost)
            .add_systems(Update, check_connect_timeout)
            .add_systems(PreUpdate, receive_welcome.after(ClientSystems::Receive))
            .add_plugins((client::ClientNetPlugin, interp::InterpPlugin));
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
            commands.write(NetCommand::Join(config.last_address.clone()));
        }
    }
}

/// Resolves "1.2.3.4", "1.2.3.4:7777", "my-pc" or "my-pc:7777".
pub fn resolve_address(input: &str, default_port: u16) -> Result<SocketAddr, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Adresse vide".into());
    }
    if let Ok(addr) = input.parse::<SocketAddr>() {
        return Ok(addr);
    }
    if let Ok(ip) = input.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, default_port));
    }
    let with_port = if input.contains(':') { input.to_string() } else { format!("{input}:{default_port}") };
    let addrs: Vec<SocketAddr> = with_port
        .to_socket_addrs()
        .map_err(|e| format!("Adresse introuvable '{input}': {e}"))?
        .collect();
    // Prefer IPv4: simpler with VPNs like Tailscale.
    addrs
        .iter()
        .find(|a| a.is_ipv4())
        .or(addrs.first())
        .copied()
        .ok_or_else(|| format!("Adresse introuvable '{input}'"))
}

#[allow(clippy::too_many_arguments)]
fn handle_net_commands(
    mut commands_in: MessageReader<NetCommand>,
    mut commands: Commands,
    mut role: ResMut<NetRole>,
    mut status: ResMut<NetStatus>,
    mut local: ResMut<LocalPlayer>,
    mut config: ResMut<AppConfig>,
    channels: Res<RepliconChannels>,
    mut server: ResMut<QuinnetServer>,
    mut client: ResMut<QuinnetClient>,
    time: Res<Time<Real>>,
) {
    for cmd in commands_in.read() {
        match cmd {
            NetCommand::Host => {
                let result = server.start_endpoint(ServerEndpointConfiguration {
                    // 0.0.0.0 = listen on every network interface (LAN, Tailscale...).
                    addr_config: EndpointAddrConfiguration::from_ip(Ipv4Addr::UNSPECIFIED, config.port),
                    cert_mode: CertificateRetrievalMode::GenerateSelfSigned {
                        server_hostname: "prism-rush".to_string(),
                    },
                    defaultables: ServerEndpointConfigurationDefaultables {
                        send_channels_cfg: channels.server_configs(),
                    },
                });
                match result {
                    Ok(_certificate) => {
                        *role = if config.launch == LaunchMode::Dedicated { NetRole::Dedicated } else { NetRole::Host };
                        status.error = None;
                        info!("server listening on UDP port {} as {:?}", config.port, *role);
                    }
                    Err(e) => {
                        error!("cannot start server: {e}");
                        status.error = Some(format!("Impossible d'héberger (port {} occupé ?) : {e}", config.port));
                    }
                }
            }
            NetCommand::Join(address) => {
                let addr = match resolve_address(address, config.port) {
                    Ok(a) => a,
                    Err(e) => {
                        status.error = Some(e);
                        continue;
                    }
                };
                let bind_ip: IpAddr =
                    if addr.is_ipv4() { Ipv4Addr::UNSPECIFIED.into() } else { Ipv6Addr::UNSPECIFIED.into() };
                let result = client.open_connection(ClientConnectionConfiguration {
                    addr_config: ClientAddrConfiguration::from_ips(addr.ip(), addr.port(), bind_ip, 0),
                    // Self-signed certificate on the host: we skip verification
                    // (fine for a game between friends; the link is still encrypted).
                    cert_mode: CertificateVerificationMode::SkipVerification,
                    defaultables: ClientConnectionConfigurationDefaultables {
                        send_channels_cfg: channels.client_configs(),
                    },
                });
                match result {
                    Ok(_) => {
                        *role = NetRole::Client;
                        status.error = None;
                        status.connecting_since = Some(time.elapsed_secs_f64());
                        config.last_address = address.clone();
                        if !config.headless {
                            config.save();
                        }
                        info!("connecting to {addr}");
                    }
                    Err(e) => status.error = Some(format!("Connexion impossible : {e}")),
                }
            }
            NetCommand::Leave => {
                info!("leaving the game");
                if server.is_listening() {
                    let _ = server.stop_endpoint();
                }
                client.close_all_connections();
                *role = NetRole::Offline;
                local.slot = None;
                status.connecting_since = None;
                // Server-side entities must be cleaned by us (clients' copies are
                // removed by replicon when the connection closes).
                commands.run_system_cached(crate::game::lobby::despawn_all_game_entities);
            }
        }
    }
}

fn send_hello(mut hello: MessageWriter<Hello>, config: Res<AppConfig>, mut status: ResMut<NetStatus>) {
    info!("connected, sending hello as '{}'", config.name);
    status.connecting_since = None;
    hello.write(Hello { name: config.name.clone(), version: PROTOCOL_VERSION });
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

fn on_connection_lost(
    role: Res<NetRole>,
    mut status: ResMut<NetStatus>,
    mut commands: MessageWriter<NetCommand>,
) {
    if *role == NetRole::Client {
        warn!("connection to the host lost");
        status.error = Some("Connexion à l'hôte perdue.".into());
        commands.write(NetCommand::Leave);
    }
}

fn check_connect_timeout(
    role: Res<NetRole>,
    state: Res<State<ClientState>>,
    time: Res<Time<Real>>,
    mut status: ResMut<NetStatus>,
    mut commands: MessageWriter<NetCommand>,
) {
    let Some(since) = status.connecting_since else { return };
    if *role != NetRole::Client || *state.get() == ClientState::Connected {
        return;
    }
    if time.elapsed_secs_f64() - since > 8.0 {
        status.error = Some(
            "Délai de connexion dépassé. Vérifie l'adresse, que l'hôte a lancé la partie, et Tailscale.".into(),
        );
        status.connecting_since = None;
        commands.write(NetCommand::Leave);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_addresses() {
        assert_eq!(resolve_address("127.0.0.1", 7777).unwrap(), "127.0.0.1:7777".parse().unwrap());
        assert_eq!(resolve_address("10.0.0.2:9000", 7777).unwrap(), "10.0.0.2:9000".parse().unwrap());
        assert_eq!(resolve_address("localhost", 7777).unwrap().port(), 7777);
        assert!(resolve_address("", 7777).is_err());
    }
}
