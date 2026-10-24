//! Lobby screen: who is here, online status of the game, start button.

use bevy::prelude::*;

use super::widgets::*;
use super::AppState;
use crate::config::AppConfig;
use crate::net::discovery::Discovery;
use crate::net::protocol::{player_color, LobbyCommand, Player};
use crate::net::{NetCommand, NetRole, NetStatus};

#[derive(Component)]
struct PlayerList;

#[derive(Component)]
struct OnlineText;

pub struct LobbyUiPlugin;

impl Plugin for LobbyUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Lobby), spawn_lobby).add_systems(
            Update,
            (handle_actions, refresh_players, update_online_text).run_if(in_state(AppState::Lobby)),
        );
    }
}

fn spawn_lobby(mut commands: Commands, role: Res<NetRole>, config: Res<AppConfig>) {
    let is_host = *role == NetRole::Host;
    let root = commands.spawn((DespawnOnExit(AppState::Lobby), screen_root())).id();
    let title = if config.solo { "SOLO" } else { "SALON" };
    commands.spawn((Text::new(title), bold(56.0), TextColor(ACCENT), TextShadow::default(), ChildOf(root)));
    let p = commands.spawn((panel(), ChildOf(root))).id();
    commands.spawn((label("Joueurs (1 à 4)", 20.0, TEXT_DIM), ChildOf(p)));
    commands.spawn((
        PlayerList,
        Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), min_width: Val::Px(380.0), ..default() },
        ChildOf(p),
    ));
    commands.spawn((
        super::arsenal::ArsenalPanel,
        Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(6.0), margin: UiRect::top(Val::Px(10.0)), ..default() },
        ChildOf(p),
    ));
    commands.spawn((
        label("", 18.0, TEXT),
        OnlineText,
        Node { margin: UiRect::vertical(Val::Px(12.0)), max_width: Val::Px(620.0), ..default() },
        ChildOf(p),
    ));
    let row = commands.spawn((Node { flex_direction: FlexDirection::Row, ..default() }, ChildOf(p))).id();
    if is_host {
        commands.spawn((button("Lancer la partie", UiAction::Start, 220.0), ChildOf(row)));
        if !config.solo {
            commands.spawn((button("Copier le code", UiAction::CopyCode, 190.0), ChildOf(row)));
        }
    }
    commands.spawn((button("Quitter", UiAction::Leave, 150.0), ChildOf(row)));
}

fn update_online_text(
    role: Res<NetRole>,
    discovery: Res<Discovery>,
    status: Res<NetStatus>,
    config: Res<AppConfig>,
    time: Res<Time<Real>>,
    mut text: Query<&mut Text, With<OnlineText>>,
) {
    let Ok(mut text) = text.single_mut() else { return };
    let dots = ".".repeat((time.elapsed_secs() * 2.0) as usize % 4);
    let msg = if config.solo {
        "Partie solo : choisis tes armes, puis « Lancer la partie ».".to_string()
    } else if *role != NetRole::Host {
        format!("En attente du lancement par l'hôte{dots}")
    } else if discovery.my_addr.is_none() {
        format!("Mise en ligne de la partie{dots}")
    } else if discovery.connected_brokers == 0 {
        format!(
            "Partie en ligne, mais la liste publique est injoignable pour l'instant{dots}\nTes amis peuvent rejoindre avec le code (bouton « Copier le code »)."
        )
    } else {
        "Ta partie est en ligne ! Tes amis la voient dans « Parties ouvertes » de leur menu :\nils cliquent sur Rejoindre, c'est tout. (Ou envoie-leur le code avec « Copier le code ».)"
            .to_string()
    };
    let msg = match (&status.host_code, *role == NetRole::Host && !config.solo) {
        (Some(code), true) => format!("{msg}\n\nCode : {code}"),
        _ => msg,
    };
    if text.0 != msg {
        text.0 = msg;
    }
}

fn refresh_players(
    mut commands: Commands,
    list: Query<(Entity, Option<&Children>), With<PlayerList>>,
    players: Query<&Player>,
    changed: Query<(), Changed<Player>>,
    mut removed: RemovedComponents<Player>,
) {
    let Ok((list, children)) = list.single() else { return };
    let dirty = !changed.is_empty() || removed.read().count() > 0 || children.is_none_or(|c| c.is_empty());
    if !dirty {
        return;
    }
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
                (
                    Node { width: Val::Px(18.0), height: Val::Px(18.0), border_radius: BorderRadius::all(Val::Px(9.0)), ..default() },
                    BackgroundColor(player_color(p.slot))
                ),
                label(format!("{}{}", p.name, tag), 22.0, TEXT),
            ],
        ));
    }
}

fn handle_actions(
    mut actions: MessageReader<UiAction>,
    mut lobby: MessageWriter<LobbyCommand>,
    mut net: MessageWriter<NetCommand>,
    status: Res<NetStatus>,
    mut clipboard: ResMut<bevy::clipboard::Clipboard>,
) {
    for action in actions.read() {
        match action {
            UiAction::Start => {
                lobby.write(LobbyCommand::Start);
            }
            UiAction::Leave => {
                net.write(NetCommand::Leave);
            }
            UiAction::CopyCode => {
                if let Some(code) = &status.host_code
                    && let Err(e) = clipboard.set_text(code.clone())
                {
                    warn!("clipboard: {e:?}");
                }
            }
            _ => {}
        }
    }
}
