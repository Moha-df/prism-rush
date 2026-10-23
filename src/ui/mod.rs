//! Menus and HUD.
//!
//! The screen shown (`AppState`) is *derived* from the network state: our
//! role and the replicated match phase. The server decides when a run starts
//! or ends; every client simply follows.

pub mod cards;
pub mod gameover;
pub mod hud;
pub mod lobby;
pub mod menu;
pub mod widgets;

use bevy::prelude::*;

use crate::net::protocol::{MatchInfo, Phase};
use crate::net::{LocalPlayer, NetRole};

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppState {
    #[default]
    Menu,
    Lobby,
    InGame,
    GameOver,
}

/// Pause menu open (the game keeps running: it is multiplayer).
#[derive(Resource, Default)]
pub struct Paused(pub bool);

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .init_resource::<Paused>()
            .add_systems(Update, follow_network_state)
            .add_systems(OnExit(AppState::InGame), |mut p: ResMut<Paused>| p.0 = false)
            .add_plugins((
                widgets::WidgetsPlugin,
                cards::CardsPlugin,
                menu::MenuPlugin,
                lobby::LobbyUiPlugin,
                hud::HudPlugin,
                gameover::GameOverPlugin,
            ));
    }
}

fn follow_network_state(
    role: Res<NetRole>,
    local: Res<LocalPlayer>,
    info: Query<&MatchInfo>,
    state: Res<State<AppState>>,
    mut next: ResMut<NextState<AppState>>,
) {
    let target = match (*role, info.single()) {
        (NetRole::Offline, _) | (_, Err(_)) => AppState::Menu,
        // A client stays on the menu ("connecting...") until welcomed.
        (NetRole::Client, Ok(_)) if local.slot.is_none() => AppState::Menu,
        (_, Ok(m)) => match m.phase {
            Phase::Lobby => AppState::Lobby,
            Phase::Playing => AppState::InGame,
            Phase::GameOver => AppState::GameOver,
        },
    };
    if *state.get() != target {
        next.set(target);
    }
}
