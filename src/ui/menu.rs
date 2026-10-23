//! Main menu: name, solo / host, and the list of open games to join.

use bevy::prelude::*;
use bevy_replicon_iroh::iroh::EndpointAddr;

use super::widgets::*;
use super::AppState;
use crate::config::{AppConfig, LaunchMode};
use crate::net::discovery::Discovery;
use crate::net::{JoinTarget, NetCommand, NetRole, NetStatus};

#[derive(Component)]
struct NameField;

#[derive(Component)]
struct CodeField;

#[derive(Component)]
struct StatusText;

#[derive(Component)]
struct GamesList;

/// Addresses of the games currently displayed (index = `UiAction::JoinListed`).
#[derive(Resource, Default)]
struct ShownGames {
    addrs: Vec<EndpointAddr>,
    signature: String,
}

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShownGames>()
            .add_systems(OnEnter(AppState::Menu), (spawn_menu, |mut s: ResMut<ShownGames>| s.signature.clear()))
            .add_systems(
                Update,
                (handle_actions, update_status, enter_to_join, refresh_games).run_if(in_state(AppState::Menu)),
            );
    }
}

fn spawn_menu(mut commands: Commands, config: Res<AppConfig>) {
    commands.spawn((
        DespawnOnExit(AppState::Menu),
        screen_root(),
        children![
            (Text::new("PRISM RUSH"), bold(84.0), TextColor(ACCENT), TextShadow { offset: Vec2::splat(4.0), ..default() }),
            label("FPS roguelike coopératif · 1 à 4 joueurs", 22.0, TEXT),
            (
                panel(),
                children![
                    label("Ton pseudo", 18.0, TEXT_DIM),
                    (text_field(&config.name, 16, 320.0), NameField),
                    (
                        Node { flex_direction: FlexDirection::Row, margin: UiRect::top(Val::Px(10.0)), ..default() },
                        children![button("Solo", UiAction::Solo, 170.0), button("Héberger", UiAction::Host, 170.0)],
                    ),
                    (label("Parties ouvertes", 22.0, ACCENT), Node { margin: UiRect::top(Val::Px(14.0)), ..default() }),
                    (
                        GamesList,
                        Node {
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Stretch,
                            min_width: Val::Px(520.0),
                            min_height: Val::Px(60.0),
                            row_gap: Val::Px(4.0),
                            ..default()
                        },
                    ),
                    label("Ou avec le code de la partie (Ctrl+V pour coller) :", 16.0, TEXT_DIM),
                    (
                        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, ..default() },
                        children![(text_field("", 70, 330.0), CodeField), button("Rejoindre", UiAction::Join, 150.0)],
                    ),
                    (label("", 18.0, ACCENT), StatusText),
                    button("Quitter", UiAction::Quit, 170.0),
                ],
            ),
            label(
                "ZQSD bouger · Espace sauter · Maj sprint · Ctrl glisser · A dash · E interagir · R recharger · 1-6/molette armes · Échap pause",
                15.0,
                TEXT_DIM
            ),
        ],
    ));
}

/// Rebuilds the list of open games when it changes.
fn refresh_games(
    mut commands: Commands,
    discovery: Res<Discovery>,
    list: Query<(Entity, Option<&Children>), With<GamesList>>,
    mut shown: ResMut<ShownGames>,
    time: Res<Time<Real>>,
) {
    let Ok((list, children)) = list.single() else { return };
    let games = discovery.open_games();
    let searching = games.is_empty();
    let dots = ".".repeat((time.elapsed_secs() * 2.0) as usize % 4);
    let signature = if searching {
        format!("empty{}{}", discovery.connected_brokers, dots)
    } else {
        games.iter().map(|(t, a)| format!("{t}{}{}{}", a.name, a.players, a.in_progress)).collect()
    };
    if signature == shown.signature {
        return;
    }
    shown.signature = signature;
    if let Some(children) = children {
        for c in children.iter() {
            commands.entity(c).try_despawn();
        }
    }
    shown.addrs.clear();
    if searching {
        let text = if discovery.connected_brokers == 0 {
            format!("Connexion à la liste des parties{dots}")
        } else {
            format!("Aucune partie ouverte pour l'instant{dots}\n(demande à ton ami de cliquer sur « Héberger »)")
        };
        commands.spawn((label(text, 17.0, TEXT_DIM), ChildOf(list)));
        return;
    }
    for (i, (_, ad)) in games.iter().take(8).enumerate() {
        let state = if ad.in_progress { " · en cours" } else { "" };
        let full = ad.players >= ad.max_players;
        commands.spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::horizontal(Val::Px(10.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.06)),
            ChildOf(list),
            children![
                label(format!("{}   ({}/{} joueurs{})", ad.name, ad.players, ad.max_players, state), 19.0, TEXT),
                button(if full { "Pleine" } else { "Rejoindre" }, UiAction::JoinListed(i as u8), 140.0),
            ],
        ));
        shown.addrs.push(ad.addr.clone());
    }
}

/// Copies the name field into the config and saves it.
fn store_name(config: &mut AppConfig, name: &Query<&TextField, With<NameField>>) {
    if let Ok(n) = name.single() {
        let trimmed = n.value.trim();
        if !trimmed.is_empty() {
            config.name = trimmed.to_string();
        }
    }
    config.save();
}

#[allow(clippy::too_many_arguments)]
fn handle_actions(
    mut actions: MessageReader<UiAction>,
    mut config: ResMut<AppConfig>,
    name: Query<&TextField, With<NameField>>,
    code: Query<&TextField, With<CodeField>>,
    shown: Res<ShownGames>,
    mut net: MessageWriter<NetCommand>,
    mut exit: MessageWriter<AppExit>,
    role: Res<NetRole>,
) {
    for action in actions.read() {
        if *role != NetRole::Offline && *action != UiAction::Quit {
            continue; // already hosting / connecting
        }
        match action {
            UiAction::Solo | UiAction::Host => {
                store_name(&mut config, &name);
                config.launch = LaunchMode::Host;
                config.autostart = *action == UiAction::Solo;
                net.write(NetCommand::Host);
            }
            UiAction::JoinListed(i) => {
                if let Some(addr) = shown.addrs.get(*i as usize) {
                    store_name(&mut config, &name);
                    config.launch = LaunchMode::Join;
                    net.write(NetCommand::Join(JoinTarget::Addr(addr.clone())));
                }
            }
            UiAction::Join => {
                store_name(&mut config, &name);
                config.launch = LaunchMode::Join;
                let text = code.single().map(|f| f.value.clone()).unwrap_or_default();
                net.write(NetCommand::Join(JoinTarget::Code(text)));
            }
            UiAction::Quit => {
                exit.write(AppExit::Success);
            }
            _ => {}
        }
    }
}

fn enter_to_join(keys: Res<ButtonInput<KeyCode>>, code: Query<&TextField, With<CodeField>>, mut actions: MessageWriter<UiAction>) {
    if keys.just_pressed(KeyCode::Enter) && code.single().is_ok_and(|f| f.focused && !f.value.trim().is_empty()) {
        actions.write(UiAction::Join);
    }
}

fn update_status(
    status: Res<NetStatus>,
    role: Res<NetRole>,
    time: Res<Time<Real>>,
    mut text: Query<&mut Text, With<StatusText>>,
) {
    let Ok(mut text) = text.single_mut() else { return };
    let dots = ".".repeat((time.elapsed_secs() * 3.0) as usize % 4);
    let msg = if let Some(e) = &status.error {
        e.clone()
    } else if *role == NetRole::Client {
        format!("Connexion à la partie en cours{dots} (jusqu'à 20 s)")
    } else if role.is_authority() {
        format!("Lancement de la partie{dots}")
    } else {
        String::new()
    };
    if text.0 != msg {
        text.0 = msg;
    }
}
