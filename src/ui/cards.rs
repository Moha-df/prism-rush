//! The card choice shown after opening a chest: 3 upgrades (4 with the
//! Crown), pick one with a click or the keys 1-4.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};

use super::widgets::*;
use super::AppState;
use crate::net::protocol::{ChooseCard, PendingChoice, Player};
use crate::net::LocalPlayer;
use crate::render::audio::{PlaySfx, Sfx};
use crate::render::icons::Icons;
use crate::shared::items::ItemKind;

/// True while our card choice is on screen (frees the mouse cursor).
#[derive(Resource, Default)]
pub struct ChoosingCard(pub bool);

#[derive(Component)]
struct CardsOverlay {
    options: Vec<ItemKind>,
}

#[derive(Component)]
struct CardButton;

pub struct CardsPlugin;

impl Plugin for CardsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChoosingCard>()
            .add_systems(Update, (show_cards, keyboard_choice, card_hover).run_if(in_state(AppState::InGame)))
            .add_systems(OnExit(AppState::InGame), |mut c: ResMut<ChoosingCard>| c.0 = false);
    }
}

fn rgb3(c: [f32; 3]) -> Color {
    Color::srgb(c[0], c[1], c[2])
}

fn show_cards(
    mut commands: Commands,
    local: Res<LocalPlayer>,
    players: Query<(&Player, Option<&PendingChoice>)>,
    overlay: Query<(Entity, &CardsOverlay)>,
    icons: Res<Icons>,
    mut choosing: ResMut<ChoosingCard>,
) {
    let pending = players.iter().find(|(p, _)| Some(p.slot) == local.slot).and_then(|(_, c)| c);
    let current = overlay.single().ok();
    match (pending, current) {
        (None, Some((e, _))) => {
            commands.entity(e).try_despawn();
            choosing.0 = false;
        }
        (Some(choice), Some((e, o))) if o.options != choice.options => {
            commands.entity(e).try_despawn();
            spawn_overlay(&mut commands, choice, &icons);
        }
        (Some(choice), None) => {
            spawn_overlay(&mut commands, choice, &icons);
            choosing.0 = true;
        }
        _ => {}
    }
}

fn spawn_overlay(commands: &mut Commands, choice: &PendingChoice, icons: &Icons) {
    let root = commands
        .spawn((
            DespawnOnExit(AppState::InGame),
            CardsOverlay { options: choice.options.clone() },
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(18.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.0, 0.05, 0.55)),
            GlobalZIndex(10),
        ))
        .id();
    commands.spawn((
        Text::new(format!("Choisis une amélioration — {}", choice.chest.name())),
        bold(30.0),
        TextColor(ACCENT),
        TextShadow::default(),
        ChildOf(root),
    ));
    let row = commands
        .spawn((Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(18.0), ..default() }, ChildOf(root)))
        .id();
    for (i, item) in choice.options.iter().enumerate() {
        let rarity = rgb3(item.rarity().color());
        let category = item.category();
        let card = commands
            .spawn((
                Button,
                Hovered::default(),
                CardButton,
                Node {
                    width: Val::Px(230.0),
                    height: Val::Px(340.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(12.0)),
                    row_gap: Val::Px(8.0),
                    border: UiRect::all(Val::Px(4.0)),
                    border_radius: BorderRadius::all(Val::Px(14.0)),
                    ..default()
                },
                BorderColor::all(rarity),
                BackgroundColor(Color::srgba(0.08, 0.06, 0.12, 0.95)),
                observe(move |_: On<Activate>, mut choose: MessageWriter<ChooseCard>, mut sfx: MessageWriter<PlaySfx>| {
                    choose.write(ChooseCard { index: i as u8 });
                    sfx.write(PlaySfx::ui(Sfx::Click));
                }),
                ChildOf(row),
            ))
            .id();
        commands.spawn((Text::new(item.rarity().name().to_uppercase()), bold(15.0), TextColor(rarity), ChildOf(card)));
        commands.spawn((
            ImageNode::new(icons.items.get(item).cloned().unwrap_or_default()),
            Node { width: Val::Px(110.0), height: Val::Px(110.0), ..default() },
            ChildOf(card),
        ));
        commands.spawn((Text::new(item.name()), bold(21.0), TextColor(TEXT), TextLayout::justify(Justify::Center), ChildOf(card)));
        commands.spawn((
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb3(category.color()).with_alpha(0.25)),
            ChildOf(card),
            children![(Text::new(category.name()), bold(14.0), TextColor(rgb3(category.color())))],
        ));
        commands.spawn((
            Text::new(item.description()),
            font(16.0),
            TextColor(TEXT_DIM),
            TextLayout::justify(Justify::Center),
            ChildOf(card),
        ));
        commands.spawn((
            Node { flex_grow: 1.0, ..default() },
            ChildOf(card),
        ));
        commands.spawn((Text::new(format!("[{}]", i + 1)), bold(18.0), TextColor(TEXT_DIM), ChildOf(card)));
    }
    commands.spawn((
        label("Clique sur une carte ou appuie sur 1, 2, 3… (le jeu continue !)", 16.0, TEXT_DIM),
        ChildOf(root),
    ));
}

fn keyboard_choice(
    keys: Res<ButtonInput<KeyCode>>,
    overlay: Query<&CardsOverlay>,
    mut choose: MessageWriter<ChooseCard>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let Ok(o) = overlay.single() else { return };
    let digits = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4];
    for (i, key) in digits.iter().enumerate().take(o.options.len()) {
        if keys.just_pressed(*key) {
            choose.write(ChooseCard { index: i as u8 });
            sfx.write(PlaySfx::ui(Sfx::Click));
        }
    }
}

fn card_hover(mut cards: Query<(&Hovered, &mut Node, &mut BackgroundColor), With<CardButton>>) {
    for (hovered, mut node, mut bg) in &mut cards {
        let h = hovered.get();
        node.top = Val::Px(if h { -10.0 } else { 0.0 });
        bg.0 = if h { Color::srgba(0.16, 0.12, 0.24, 0.98) } else { Color::srgba(0.08, 0.06, 0.12, 0.95) };
    }
}
