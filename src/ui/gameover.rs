//! End-of-run screen with per-player statistics.

use bevy::prelude::*;

use super::widgets::*;
use super::AppState;
use crate::net::protocol::{player_color, GameEvent, LobbyCommand, RunSummary};
use crate::net::{NetCommand, NetRole};

/// Statistics of the last run, received from the server.
#[derive(Resource, Default)]
pub struct LastRun(pub Option<RunSummary>);

pub struct GameOverPlugin;

impl Plugin for GameOverPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LastRun>()
            .add_systems(Update, store_summary)
            .add_systems(OnEnter(AppState::GameOver), spawn_screen)
            .add_systems(Update, (handle_actions, fill_when_ready).run_if(in_state(AppState::GameOver)));
    }
}

#[derive(Component)]
struct StatsTable;

fn store_summary(mut events: MessageReader<GameEvent>, mut last: ResMut<LastRun>) {
    for ev in events.read() {
        if let GameEvent::GameOver(summary) = ev {
            last.0 = Some((**summary).clone());
        }
    }
}

fn spawn_screen(mut commands: Commands, role: Res<NetRole>) {
    let root = commands
        .spawn((DespawnOnExit(AppState::GameOver), screen_root(), BackgroundColor(Color::srgba(0.05, 0.0, 0.05, 0.6))))
        .id();
    commands.spawn((Text::new("GAME OVER"), bold(80.0), TextColor(Color::srgb(1.0, 0.35, 0.35)), TextShadow::default(), ChildOf(root)));
    let p = commands.spawn((panel(), ChildOf(root))).id();
    commands.spawn((
        StatsTable,
        Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() },
        ChildOf(p),
    ));
    let row = commands.spawn((Node { flex_direction: FlexDirection::Row, margin: UiRect::top(Val::Px(12.0)), ..default() }, ChildOf(p))).id();
    if *role == NetRole::Host {
        commands.spawn((button("Retour au salon", UiAction::BackToLobby, 240.0), ChildOf(row)));
    } else {
        commands.spawn((label("En attente de l'hôte…", 18.0, TEXT_DIM), ChildOf(row)));
    }
    commands.spawn((button("Menu principal", UiAction::Leave, 200.0), ChildOf(row)));
}

/// The summary may arrive a frame after the state change: fill when ready.
fn fill_when_ready(
    mut commands: Commands,
    last: Res<LastRun>,
    table: Query<(Entity, Option<&Children>), With<StatsTable>>,
) {
    let Ok((table, children)) = table.single() else { return };
    if children.is_some_and(|c| !c.is_empty()) {
        return;
    }
    let Some(run) = &last.0 else { return };
    let secs = run.duration as u32;
    commands.spawn((
        label(format!("Survécu {:02}:{:02} — niveau {}", secs / 60, secs % 60, run.level), 26.0, TEXT),
        ChildOf(table),
    ));
    commands.spawn((
        label("Joueur            Kills   Dégâts   Or gagné   Coffres   Réanimations   À terre   Objets", 16.0, TEXT_DIM),
        ChildOf(table),
    ));
    for p in &run.players {
        commands.spawn((
            label(
                format!(
                    "{:<16}  {:>5}   {:>6.0}   {:>8}   {:>7}   {:>12}   {:>7}   {:>6}",
                    p.name, p.kills, p.damage, p.gold_earned, p.chests, p.revives, p.downs, p.items
                ),
                20.0,
                player_color(p.slot),
            ),
            ChildOf(table),
        ));
    }
}

fn handle_actions(mut actions: MessageReader<UiAction>, mut lobby: MessageWriter<LobbyCommand>, mut net: MessageWriter<NetCommand>) {
    for action in actions.read() {
        match action {
            UiAction::BackToLobby => {
                lobby.write(LobbyCommand::BackToLobby);
            }
            UiAction::Leave => {
                net.write(NetCommand::Leave);
            }
            _ => {}
        }
    }
}
