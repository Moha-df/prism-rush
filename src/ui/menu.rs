//! Main menu: name, solo / host / join.

use bevy::prelude::*;

use super::widgets::*;
use super::AppState;
use crate::config::{AppConfig, LaunchMode};
use crate::net::{NetCommand, NetRole, NetStatus};

#[derive(Component)]
struct NameField;

#[derive(Component)]
struct AddressField;

#[derive(Component)]
struct StatusText;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Menu), spawn_menu)
            .add_systems(Update, (handle_actions, update_status, enter_to_join).run_if(in_state(AppState::Menu)));
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
                        children![button("Solo", UiAction::Solo, 160.0), button("Héberger", UiAction::Host, 160.0)],
                    ),
                    label("Rejoindre un ami (IP Tailscale ou locale) :", 18.0, TEXT_DIM),
                    (
                        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, ..default() },
                        children![
                            (text_field(&config.last_address, 40, 230.0), AddressField),
                            button("Rejoindre", UiAction::Join, 140.0)
                        ],
                    ),
                    (label("", 18.0, ACCENT), StatusText),
                    button("Quitter", UiAction::Quit, 160.0),
                ],
            ),
            label(
                "ZQSD/WASD bouger · Espace sauter · Maj sprint · Ctrl glisser · A/Q dash · E interagir · R recharger · 1-6/molette armes · Échap pause",
                15.0,
                TEXT_DIM
            ),
        ],
    ));
}

/// Copies the text fields into the config and saves it.
fn store_fields(
    config: &mut AppConfig,
    name: &Query<&TextField, With<NameField>>,
    address: &Query<&TextField, With<AddressField>>,
) {
    if let Ok(n) = name.single() {
        let trimmed = n.value.trim();
        if !trimmed.is_empty() {
            config.name = trimmed.to_string();
        }
    }
    if let Ok(a) = address.single() {
        config.last_address = a.value.trim().to_string();
    }
    config.save();
}

#[allow(clippy::too_many_arguments)]
fn handle_actions(
    mut actions: MessageReader<UiAction>,
    mut config: ResMut<AppConfig>,
    name: Query<&TextField, With<NameField>>,
    address: Query<&TextField, With<AddressField>>,
    mut net: MessageWriter<NetCommand>,
    mut exit: MessageWriter<AppExit>,
    role: Res<NetRole>,
) {
    for action in actions.read() {
        if *role != NetRole::Offline && *action != UiAction::Quit {
            continue; // already connecting
        }
        match action {
            UiAction::Solo => {
                store_fields(&mut config, &name, &address);
                config.launch = LaunchMode::Host;
                config.autostart = true;
                net.write(NetCommand::Host);
            }
            UiAction::Host => {
                store_fields(&mut config, &name, &address);
                config.launch = LaunchMode::Host;
                config.autostart = false;
                net.write(NetCommand::Host);
            }
            UiAction::Join => {
                store_fields(&mut config, &name, &address);
                config.launch = LaunchMode::Join;
                net.write(NetCommand::Join(config.last_address.clone()));
            }
            UiAction::Quit => {
                exit.write(AppExit::Success);
            }
            _ => {}
        }
    }
}

fn enter_to_join(keys: Res<ButtonInput<KeyCode>>, address: Query<&TextField, With<AddressField>>, mut actions: MessageWriter<UiAction>) {
    if keys.just_pressed(KeyCode::Enter) && address.single().is_ok_and(|f| f.focused) {
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
    let msg = if let Some(e) = &status.error {
        e.clone()
    } else if *role == NetRole::Client {
        let dots = ".".repeat((time.elapsed_secs() * 3.0) as usize % 4);
        format!("Connexion en cours{dots}")
    } else {
        String::new()
    };
    if text.0 != msg {
        text.0 = msg;
    }
}
