//! Lobby screen: who is here, map and difficulty (picked by the host),
//! online status of the game, start button.

use bevy::prelude::*;

use super::widgets::*;
use super::AppState;
use crate::config::AppConfig;
use crate::net::discovery::Discovery;
use crate::net::protocol::{player_color, LobbyCommand, MatchInfo, Player};
use crate::net::{NetCommand, NetRole, NetStatus};
use crate::shared::rules::{Difficulty, MapKind};

#[derive(Component)]
struct PlayerList;

#[derive(Component)]
struct OnlineText;

/// Map + difficulty block, rebuilt when they change.
#[derive(Component)]
struct RulesPanel;

pub struct LobbyUiPlugin;

impl Plugin for LobbyUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Lobby), (spawn_lobby, send_rules)).add_systems(
            Update,
            (handle_actions, refresh_players, update_online_text, refresh_rules).run_if(in_state(AppState::Lobby)),
        );
    }
}

fn spawn_lobby(mut commands: Commands, role: Res<NetRole>, config: Res<AppConfig>) {
    let is_host = *role == NetRole::Host;
    let root = commands.spawn((DespawnOnExit(AppState::Lobby), screen_root())).id();
    let title = if config.solo { "SOLO" } else { "SALON" };
    commands.spawn((Text::new(title), bold(44.0), TextColor(ACCENT), TextShadow::default(), ChildOf(root)));
    let p = commands.spawn((panel(), ChildOf(root))).id();
    // Players on the left, map and difficulty on the right.
    let top = commands
        .spawn((Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(30.0), align_items: AlignItems::Center, ..default() }, ChildOf(p)))
        .id();
    let left = commands.spawn((Node { flex_direction: FlexDirection::Column, min_width: Val::Px(220.0), ..default() }, ChildOf(top))).id();
    commands.spawn((label("Joueurs (1 à 4)", 20.0, TEXT_DIM), ChildOf(left)));
    commands.spawn((
        PlayerList,
        Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() },
        ChildOf(left),
    ));
    commands.spawn((
        RulesPanel,
        Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(2.0), max_width: Val::Px(560.0), ..default() },
        ChildOf(top),
    ));
    commands.spawn((
        super::arsenal::ArsenalPanel,
        Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(6.0), margin: UiRect::top(Val::Px(10.0)), ..default() },
        ChildOf(p),
    ));
    commands.spawn((
        label("", 16.0, TEXT),
        OnlineText,
        Node { margin: UiRect::vertical(Val::Px(8.0)), max_width: Val::Px(900.0), ..default() },
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

/// The host's saved choice becomes the server's rules.
fn send_rules(role: Res<NetRole>, config: Res<AppConfig>, mut lobby: MessageWriter<LobbyCommand>) {
    if *role == NetRole::Host {
        lobby.write(LobbyCommand::SetRules { difficulty: config.difficulty, map: config.map });
    }
}

/// One "< value >" line (arrows only for the host).
fn picker(commands: &mut Commands, parent: Entity, title: &str, value: &str, prev: Option<(UiAction, UiAction)>) {
    let row = commands
        .spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }, ChildOf(parent)))
        .id();
    commands.spawn((label(title, 18.0, TEXT_DIM), Node { width: Val::Px(90.0), ..default() }, ChildOf(row)));
    if let Some((left, _)) = prev {
        commands.spawn((button("<", left, 46.0), ChildOf(row)));
    }
    commands.spawn((
        Text::new(value),
        bold(24.0),
        TextColor(ACCENT),
        TextLayout::justify(Justify::Center),
        Node { width: Val::Px(250.0), ..default() },
        ChildOf(row),
    ));
    if let Some((_, right)) = prev {
        commands.spawn((button(">", right, 46.0), ChildOf(row)));
    }
}

fn refresh_rules(
    mut commands: Commands,
    role: Res<NetRole>,
    config: Res<AppConfig>,
    info: Query<&MatchInfo>,
    panel: Query<(Entity, Option<&Children>), With<RulesPanel>>,
    mut shown: Local<Option<(Difficulty, MapKind, usize)>>,
) {
    let Ok((panel, children)) = panel.single() else {
        *shown = None;
        return;
    };
    let is_host = *role == NetRole::Host;
    // The host sees its own choice right away; the others see the server's.
    let (difficulty, map) = if is_host {
        (config.difficulty, config.map)
    } else {
        match info.single() {
            Ok(m) => (m.rules.difficulty, m.rules.map),
            Err(_) => return,
        }
    };
    let key = (difficulty, map, config.maps_unlocked);
    if *shown == Some(key) && children.is_some_and(|c| !c.is_empty()) {
        return;
    }
    *shown = Some(key);
    if let Some(children) = children {
        for c in children.iter() {
            commands.entity(c).try_despawn();
        }
    }
    let arrows = |a, b| is_host.then_some((a, b));
    picker(&mut commands, panel, "Carte", map.name(), arrows(UiAction::MapPrev, UiAction::MapNext));
    commands.spawn((label(map.description(), 15.0, TEXT_DIM), ChildOf(panel)));
    if is_host
        && let Some(locked) = MapKind::ALL.into_iter().find(|m| !config.is_unlocked_map(*m))
    {
        let before = MapKind::ALL[locked.index() - 1];
        commands.spawn((
            label(format!("Prochaine carte : {} — tiens 8 minutes sur {} pour la débloquer", locked.name(), before.name()), 15.0, TEXT_DIM),
            ChildOf(panel),
        ));
    }
    picker(&mut commands, panel, "Difficulté", difficulty.name(), arrows(UiAction::DiffPrev, UiAction::DiffNext));
    commands.spawn((label(difficulty.description(), 15.0, TEXT_DIM), ChildOf(panel)));
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
        "Ta partie est en ligne ! Tes amis la voient dans « Parties ouvertes » de leur menu et cliquent sur Rejoindre.\n(Ou envoie-leur le code avec « Copier le code ».)"
            .to_string()
    };
    let msg = match (&status.host_code, *role == NetRole::Host && !config.solo) {
        (Some(code), true) => format!("{msg}\nCode : {code}"),
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
    mut config: ResMut<AppConfig>,
) {
    for action in actions.read() {
        // Map / difficulty: saved for next time and sent to the server.
        let rules_changed = match action {
            UiAction::MapPrev | UiAction::MapNext => {
                let unlocked: Vec<MapKind> = MapKind::ALL.into_iter().filter(|m| config.is_unlocked_map(*m)).collect();
                let i = unlocked.iter().position(|m| *m == config.map).unwrap_or(0);
                let n = unlocked.len();
                let j = if *action == UiAction::MapNext { (i + 1) % n } else { (i + n - 1) % n };
                config.map = unlocked[j];
                true
            }
            UiAction::DiffPrev => {
                config.difficulty = config.difficulty.easier();
                true
            }
            UiAction::DiffNext => {
                config.difficulty = config.difficulty.harder();
                true
            }
            _ => false,
        };
        if rules_changed {
            config.save();
            lobby.write(LobbyCommand::SetRules { difficulty: config.difficulty, map: config.map });
            continue;
        }
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
