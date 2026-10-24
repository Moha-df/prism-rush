//! The arsenal in the lobby: pick your starting main weapon, and spend the
//! silver earned in runs to unlock new weapons (Megabonk-like meta).
//!
//! Unlocks are saved in `settings.txt` (see `config.rs`): they belong to
//! the player, not to the game they join.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};

use super::widgets::*;
use super::AppState;
use crate::config::AppConfig;
use crate::net::protocol::PickLoadout;
use crate::render::audio::{PlaySfx, Sfx};
use crate::render::icons::Icons;
use crate::shared::build::{AutoWeapon, MainWeapon};

/// Container rebuilt whenever the config (selection, silver) changes.
#[derive(Component)]
pub struct ArsenalPanel;

/// What a tile does when clicked.
#[derive(Clone, Copy, Debug)]
enum TileAction {
    Main(MainWeapon),
    Auto(AutoWeapon),
}

pub struct ArsenalPlugin;

impl Plugin for ArsenalPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<TileClick>().add_systems(Update, (rebuild_panel, on_tile_click).run_if(in_state(AppState::Lobby)));
    }
}

#[derive(Message, Clone, Copy, Debug)]
struct TileClick(TileAction);

const GOLD_BORDER: Color = Color::srgb(1.0, 0.8, 0.3);
const LOCKED: Color = Color::srgb(0.35, 0.33, 0.4);

fn tile(commands: &mut Commands, parent: Entity, icon: Handle<Image>, name: &str, status: String, border: Color, dim: bool, action: TileAction) {
    let tint = if dim { Color::srgba(1.0, 1.0, 1.0, 0.35) } else { Color::WHITE };
    commands.spawn((
        Button,
        Hovered::default(),
        MenuButton,
        Node {
            width: Val::Px(104.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(6.0)),
            border: UiRect::all(Val::Px(3.0)),
            border_radius: BorderRadius::all(Val::Px(10.0)),
            ..default()
        },
        BorderColor::all(border),
        BackgroundColor(Color::srgba(0.08, 0.06, 0.12, 0.9)),
        observe(move |_: On<Activate>, mut clicks: MessageWriter<TileClick>| {
            clicks.write(TileClick(action));
        }),
        ChildOf(parent),
        children![
            (ImageNode::new(icon).with_color(tint), Node { width: Val::Px(56.0), height: Val::Px(56.0), ..default() }),
            (Text::new(name), bold(13.0), TextColor(if dim { TEXT_DIM } else { TEXT }), TextLayout::justify(Justify::Center)),
            (Text::new(status), font(12.0), TextColor(if dim { LOCKED.lighter(0.3) } else { TEXT_DIM })),
        ],
    ));
}

fn row(commands: &mut Commands, parent: Entity) -> Entity {
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(6.0),
                max_width: Val::Px(900.0),
                ..default()
            },
            ChildOf(parent),
        ))
        .id()
}

fn rebuild_panel(
    mut commands: Commands,
    config: Res<AppConfig>,
    icons: Res<Icons>,
    panel: Query<(Entity, Option<&Children>), With<ArsenalPanel>>,
    mut built: Local<bool>,
) {
    let Ok((panel, children)) = panel.single() else {
        *built = false;
        return;
    };
    if *built && !config.is_changed() {
        return;
    }
    *built = true;
    if let Some(children) = children {
        for c in children.iter() {
            commands.entity(c).try_despawn();
        }
    }
    commands.spawn((
        label(format!("Ton arme principale (tu vises et tires)   ·   Argent : {}", config.silver), 18.0, ACCENT),
        ChildOf(panel),
    ));
    let mains = row(&mut commands, panel);
    for w in MainWeapon::ALL {
        let def = w.def();
        let unlocked = config.is_unlocked_main(w);
        let selected = config.main_weapon == w;
        let (status, border) = if selected {
            ("Choisie".to_string(), GOLD_BORDER)
        } else if unlocked {
            ("Choisir".to_string(), TEXT_DIM)
        } else {
            (format!("{} argent", def.silver_cost), LOCKED)
        };
        tile(&mut commands, mains, icons.mains.get(&w).cloned().unwrap_or_default(), def.name, status, border, !unlocked, TileAction::Main(w));
    }
    commands.spawn((
        label("Ton arme automatique de départ (les autres débloquées sont proposées pendant la partie)", 16.0, ACCENT),
        ChildOf(panel),
    ));
    let autos = row(&mut commands, panel);
    for a in AutoWeapon::ALL {
        let def = a.def();
        let unlocked = config.is_unlocked_auto(a);
        let selected = config.start_auto == Some(a);
        let (status, border) = if selected {
            ("Choisie".to_string(), GOLD_BORDER)
        } else if unlocked {
            ("Choisir".to_string(), Color::srgb(0.35, 0.9, 1.0))
        } else {
            (format!("{} argent", def.silver_cost), LOCKED)
        };
        tile(&mut commands, autos, icons.autos.get(&a).cloned().unwrap_or_default(), def.name, status, border, !unlocked, TileAction::Auto(a));
    }
}

/// Clicking an unlocked main weapon selects it; clicking a locked weapon
/// buys it if you have enough silver.
fn on_tile_click(
    mut clicks: MessageReader<TileClick>,
    mut config: ResMut<AppConfig>,
    mut pick: MessageWriter<PickLoadout>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    for TileClick(action) in clicks.read() {
        match *action {
            TileAction::Main(w) if config.is_unlocked_main(w) => {
                config.main_weapon = w;
                sfx.write(PlaySfx::ui(Sfx::Switch));
            }
            TileAction::Main(w) => {
                let cost = w.def().silver_cost;
                if config.silver >= cost {
                    config.silver -= cost;
                    config.unlocked_mains.push(w);
                    config.main_weapon = w;
                    sfx.write(PlaySfx::ui(Sfx::ChestOpen));
                } else {
                    sfx.write(PlaySfx::ui(Sfx::Empty));
                }
            }
            TileAction::Auto(a) if config.is_unlocked_auto(a) => {
                config.start_auto = Some(a);
                sfx.write(PlaySfx::ui(Sfx::Switch));
            }
            TileAction::Auto(a) => {
                let cost = a.def().silver_cost;
                if config.silver >= cost {
                    config.silver -= cost;
                    config.unlocked_autos.push(a);
                    config.start_auto = Some(a);
                    sfx.write(PlaySfx::ui(Sfx::ChestOpen));
                } else {
                    sfx.write(PlaySfx::ui(Sfx::Empty));
                }
            }
        }
        config.save();
        // Tell the server (our weapons for the next run, our unlocked weapons).
        pick.write(PickLoadout { main: config.main_weapon, start_auto: config.start_auto, autos: config.auto_pool() });
    }
}
