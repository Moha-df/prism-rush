//! Lobby screen: who is here, the address to give to friends, start button.

use std::net::UdpSocket;
use std::process::Command;

use bevy::prelude::*;

use super::widgets::*;
use super::AppState;
use crate::config::AppConfig;
use crate::net::protocol::{player_color, LobbyCommand, Player};
use crate::net::{NetCommand, NetRole};

#[derive(Component)]
struct PlayerList;

#[derive(Component)]
struct AddressText;

/// Addresses friends can use to join this host.
#[derive(Resource, Default, Clone, Debug)]
pub struct HostAddresses {
    pub tailscale: Option<String>,
    pub lan: Option<String>,
}

pub struct LobbyUiPlugin;

impl Plugin for LobbyUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HostAddresses>()
            .add_systems(OnEnter(AppState::Lobby), (detect_addresses, spawn_lobby).chain())
            .add_systems(Update, (handle_actions, refresh_players).run_if(in_state(AppState::Lobby)));
    }
}

/// Our Tailscale IPv4 (100.x.y.z), if Tailscale is installed and running.
pub fn tailscale_ip() -> Option<String> {
    let candidates = ["tailscale", r"C:\Program Files\Tailscale\tailscale.exe"];
    for exe in candidates {
        let mut cmd = Command::new(exe);
        cmd.args(["ip", "-4"]);
        #[cfg(windows)]
        {
            // Don't flash a console window.
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        if let Ok(out) = cmd.output()
            && out.status.success()
        {
            let ip = String::from_utf8_lossy(&out.stdout).lines().next().unwrap_or("").trim().to_string();
            if ip.starts_with("100.") {
                return Some(ip);
            }
        }
    }
    None
}

/// Our main LAN address: "connect" a UDP socket (no packet is sent) and read
/// which local interface the OS picked.
pub fn lan_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

fn detect_addresses(role: Res<NetRole>, mut addresses: ResMut<HostAddresses>) {
    if *role == NetRole::Host && addresses.lan.is_none() && addresses.tailscale.is_none() {
        *addresses = HostAddresses { tailscale: tailscale_ip(), lan: lan_ip() };
        info!("host addresses: {addresses:?}");
    }
}

fn spawn_lobby(mut commands: Commands, role: Res<NetRole>, addresses: Res<HostAddresses>, config: Res<AppConfig>) {
    let is_host = *role == NetRole::Host;
    let port = config.port;
    let address_text = if is_host {
        let mut lines = vec!["Donne cette adresse à tes amis :".to_string()];
        match &addresses.tailscale {
            Some(ip) => lines.push(format!("Internet (Tailscale) : {ip}")),
            None => lines.push("Tailscale non détecté — voir le README pour jouer par Internet".into()),
        }
        if let Some(ip) = &addresses.lan {
            lines.push(format!("Même réseau local : {ip}"));
        }
        if port != crate::net::protocol::DEFAULT_PORT {
            lines.push(format!("(port {port})"));
        }
        lines.join("\n")
    } else {
        "En attente du lancement par l'hôte…".to_string()
    };

    let root = commands
        .spawn((DespawnOnExit(AppState::Lobby), screen_root()))
        .id();
    commands.spawn((Text::new("SALON"), bold(56.0), TextColor(ACCENT), TextShadow::default(), ChildOf(root)));
    let p = commands.spawn((panel(), ChildOf(root))).id();
    commands.spawn((label("Joueurs (1 à 4)", 20.0, TEXT_DIM), ChildOf(p)));
    commands.spawn((
        PlayerList,
        Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), min_width: Val::Px(360.0), ..default() },
        ChildOf(p),
    ));
    commands.spawn((label(address_text, 20.0, TEXT), AddressText, Node { margin: UiRect::vertical(Val::Px(12.0)), ..default() }, ChildOf(p)));
    let row = commands.spawn((Node { flex_direction: FlexDirection::Row, ..default() }, ChildOf(p))).id();
    if is_host {
        commands.spawn((button("Lancer la partie", UiAction::Start, 220.0), ChildOf(row)));
    }
    commands.spawn((button("Quitter", UiAction::Leave, 160.0), ChildOf(row)));
}

fn refresh_players(
    mut commands: Commands,
    list: Query<(Entity, Option<&Children>), With<PlayerList>>,
    players: Query<&Player>,
    changed: Query<(), Changed<Player>>,
    mut removed: RemovedComponents<Player>,
    mut first: Local<bool>,
) {
    let Ok((list, children)) = list.single() else { return };
    let dirty = !changed.is_empty() || removed.read().count() > 0 || children.is_none_or(|c| c.is_empty()) || !*first;
    if !dirty {
        return;
    }
    *first = true;
    if let Some(children) = children {
        for c in children.iter() {
            commands.entity(c).try_despawn();
        }
    }
    let mut sorted: Vec<&Player> = players.iter().collect();
    sorted.sort_by_key(|p| p.slot);
    for p in sorted {
        let tag = if p.is_bot { " (bot)" } else if p.slot == 0 { " (hôte)" } else { "" };
        commands.spawn((
            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(10.0), ..default() },
            ChildOf(list),
            children![
                (Node { width: Val::Px(18.0), height: Val::Px(18.0), border_radius: BorderRadius::all(Val::Px(9.0)), ..default() }, BackgroundColor(player_color(p.slot))),
                label(format!("{}{}", p.name, tag), 22.0, TEXT),
            ],
        ));
    }
}

fn handle_actions(
    mut actions: MessageReader<UiAction>,
    mut lobby: MessageWriter<LobbyCommand>,
    mut net: MessageWriter<NetCommand>,
) {
    for action in actions.read() {
        match action {
            UiAction::Start => {
                lobby.write(LobbyCommand::Start);
            }
            UiAction::Leave => {
                net.write(NetCommand::Leave);
            }
            _ => {}
        }
    }
}
