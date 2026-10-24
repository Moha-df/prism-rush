//! The card choice shown after opening a chest or on a team level-up:
//! 3 cards (4 with the Crown), pick one with a click or the keys 1-4.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};

use super::widgets::*;
use super::AppState;
use crate::net::protocol::{CardOption, ChooseCard, PendingChoice, Player};
use crate::net::LocalPlayer;
use crate::render::audio::{PlaySfx, Sfx};
use crate::render::icons::Icons;
use crate::shared::build::{describe_offer, Offer};
use crate::shared::items::Rarity;

/// True while our card choice is on screen (frees the mouse cursor).
#[derive(Resource, Default)]
pub struct ChoosingCard(pub bool);

#[derive(Component)]
struct CardsOverlay {
    options: Vec<CardOption>,
    /// `Time::elapsed_secs` when the choice appeared (drives the animations).
    opened: f32,
}

/// Everything a card displays.
struct CardLook {
    tier: Rarity,
    rarity: Color,
    rarity_name: &'static str,
    icon: Handle<Image>,
    title: String,
    category: (&'static str, Color),
    description: String,
}

fn card_look(option: &CardOption, icons: &Icons) -> CardLook {
    match option {
        CardOption::Item(item) => {
            let category = item.category();
            CardLook {
                tier: item.rarity(),
                rarity: rgb3(item.rarity().color()),
                rarity_name: item.rarity().name(),
                icon: icons.items.get(item).cloned().unwrap_or_default(),
                title: item.name().to_string(),
                category: (category.name(), rgb3(category.color())),
                description: item.description().to_string(),
            }
        }
        CardOption::Offer(card) => {
            let (title, description) = describe_offer(card);
            let (icon, category) = match &card.offer {
                Offer::UpgradeMain { .. } => (None, ("Arme principale", Color::srgb(1.0, 0.55, 0.25))),
                Offer::NewAuto(a) => (icons.autos.get(a), ("Nouvelle arme auto", Color::srgb(1.0, 0.85, 0.3))),
                Offer::UpgradeAuto { weapon, .. } => (icons.autos.get(weapon), ("Arme auto", Color::srgb(0.35, 0.9, 1.0))),
                Offer::Tome { tome, .. } => (icons.tomes.get(tome), ("Tome", Color::srgb(0.75, 0.5, 1.0))),
            };
            CardLook {
                tier: card.rarity,
                rarity: rgb3(card.rarity.color()),
                rarity_name: card.rarity.name(),
                icon: icon.cloned().unwrap_or_default(),
                title,
                category,
                description,
            }
        }
    }
}

/// One card: its place in the row (stagger), rarity and smoothed hover amount.
#[derive(Component)]
struct CardButton {
    index: usize,
    tier: Rarity,
    rarity: Color,
    hover: f32,
}

/// The diagonal light streak sweeping across legendary cards.
#[derive(Component)]
struct Shimmer {
    index: usize,
}

/// The "[1]" key cap at the bottom of a card (lights up on hover).
#[derive(Component)]
struct KeyCap {
    index: usize,
}

/// Title block above the cards (pops in).
#[derive(Component)]
struct CardsHeader;

/// Delay between two cards appearing, and the length of one card's entrance:
/// with 4 cards the whole choice is in place after 0.35 s (the game does not pause).
const STAGGER: f32 = 0.05;
const APPEAR: f32 = 0.2;
const CARD_BG: Color = Color::srgba(0.07, 0.05, 0.11, 0.96);
const CARD_BG_HOVER: Color = Color::srgba(0.14, 0.1, 0.21, 0.98);
/// Stat lines ("+24% Taille").
const STAT: Color = Color::srgb(0.6, 1.0, 0.62);

pub struct CardsPlugin;

impl Plugin for CardsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChoosingCard>()
            .add_systems(
                Update,
                (show_cards, keyboard_choice, animate_overlay, animate_cards, animate_shimmer, animate_keycaps)
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(OnExit(AppState::InGame), |mut c: ResMut<ChoosingCard>| c.0 = false);
    }
}

fn rgb3(c: [f32; 3]) -> Color {
    Color::srgb(c[0], c[1], c[2])
}

fn show_cards(
    mut commands: Commands,
    local: Res<LocalPlayer>,
    players: Query<(&Player, Option<&PendingChoice>, Option<&crate::net::protocol::Gear>)>,
    overlay: Query<(Entity, &CardsOverlay)>,
    icons: Res<Icons>,
    time: Res<Time>,
    mut choosing: ResMut<ChoosingCard>,
) {
    let now = time.elapsed_secs();
    let me = players.iter().find(|(p, ..)| Some(p.slot) == local.slot);
    let pending = me.and_then(|(_, c, _)| c);
    // Main weapon upgrades show the icon of our main weapon.
    let main = me.and_then(|(.., g)| g).map(|g| g.0.main);
    let main_icon = (
        main.and_then(|m| icons.mains.get(&m)).cloned().unwrap_or_default(),
        main.map(|m| m.def().name).unwrap_or("Arme principale"),
    );
    let current = overlay.single().ok();
    match (pending, current) {
        (None, Some((e, _))) => {
            commands.entity(e).try_despawn();
            choosing.0 = false;
        }
        (Some(choice), Some((e, o))) if o.options != choice.options => {
            commands.entity(e).try_despawn();
            spawn_overlay(&mut commands, choice, &icons, &main_icon, now);
        }
        (Some(choice), None) => {
            spawn_overlay(&mut commands, choice, &icons, &main_icon, now);
            choosing.0 = true;
        }
        _ => {}
    }
}

fn spawn_overlay(
    commands: &mut Commands,
    choice: &PendingChoice,
    icons: &Icons,
    main_icon: &(Handle<Image>, &str),
    now: f32,
) {
    let root = commands
        .spawn((
            DespawnOnExit(AppState::InGame),
            CardsOverlay { options: choice.options.clone(), opened: now },
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(22.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.0, 0.05, 0.0)),
            GlobalZIndex(10),
        ))
        .id();
    let (title, subtitle, title_color) = match choice.chest {
        Some(chest) => (chest.name().to_uppercase(), "Choisis un objet", Color::srgb(1.0, 0.82, 0.35)),
        None => ("NIVEAU SUPÉRIEUR !".to_string(), "Choisis une amélioration", ACCENT),
    };
    commands.spawn((
        CardsHeader,
        Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() },
        UiTransform::from_scale(Vec2::splat(1.3)),
        ChildOf(root),
        children![
            (Text::new(title), bold(40.0), TextColor(title_color), TextShadow::default()),
            (Text::new(subtitle), bold(19.0), TextColor(TEXT), TextShadow::default()),
        ],
    ));
    let row = commands
        .spawn((Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(22.0), ..default() }, ChildOf(root)))
        .id();
    for (i, option) in choice.options.iter().enumerate() {
        let mut look = card_look(option, icons);
        if matches!(option, CardOption::Offer(c) if matches!(c.offer, Offer::UpgradeMain { .. })) {
            look.icon = main_icon.0.clone();
            look.title = main_icon.1.to_string();
        }
        spawn_card(commands, row, i, look);
    }
    let n = choice.options.len();
    commands.spawn((
        label(format!("Clique sur une carte ou appuie sur 1 – {n}  ·  le jeu continue !"), 17.0, TEXT_DIM),
        ChildOf(root),
    ));
}

fn spawn_card(commands: &mut Commands, row: Entity, i: usize, look: CardLook) {
    let rarity = look.rarity;
    let card = commands
        .spawn((
            Button,
            Hovered::default(),
            CardButton { index: i, tier: look.tier, rarity, hover: 0.0 },
            Node {
                width: Val::Px(236.0),
                min_height: Val::Px(372.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(16.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(rarity),
            BackgroundColor(CARD_BG),
            BoxShadow::new(rarity.with_alpha(0.0), Val::ZERO, Val::ZERO, Val::Px(2.0), Val::Px(16.0)),
            // Starts hidden and small: `animate_cards` brings it in.
            UiTransform::from_scale(Vec2::splat(0.6)),
            Visibility::Hidden,
            observe(move |_: On<Activate>, mut choose: MessageWriter<ChooseCard>, mut sfx: MessageWriter<PlaySfx>| {
                choose.write(ChooseCard { index: i as u8 });
                sfx.write(PlaySfx::ui(Sfx::Click));
            }),
            ChildOf(row),
        ))
        .id();
    // Rarity ribbon across the top.
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(Val::ZERO, Val::Px(6.0)),
            border: UiRect::bottom(Val::Px(2.0)),
            border_radius: BorderRadius::top(Val::Px(13.0)),
            ..default()
        },
        BackgroundColor(rarity.with_alpha(0.22)),
        BorderColor::all(rarity.with_alpha(0.6)),
        ChildOf(card),
        children![(Text::new(look.rarity_name.to_uppercase()), bold(15.0), TextColor(rarity), TextShadow::default())],
    ));
    let body = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::new(Val::Px(14.0), Val::Px(14.0), Val::Px(14.0), Val::Px(12.0)),
                row_gap: Val::Px(8.0),
                ..default()
            },
            ChildOf(card),
        ))
        .id();
    // Icon on a rarity-tinted disc.
    commands.spawn((
        Node {
            width: Val::Px(118.0),
            height: Val::Px(118.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(rarity.with_alpha(0.14)),
        BorderColor::all(rarity.with_alpha(0.45)),
        ChildOf(body),
        children![(ImageNode::new(look.icon), Node { width: Val::Px(94.0), height: Val::Px(94.0), ..default() })],
    ));
    commands.spawn((
        Text::new(look.title),
        bold(21.0),
        TextColor(TEXT),
        TextShadow::default(),
        TextLayout::justify(Justify::Center),
        ChildOf(body),
    ));
    commands.spawn((
        Node {
            padding: UiRect::axes(Val::Px(10.0), Val::Px(2.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(look.category.1.with_alpha(0.18)),
        BorderColor::all(look.category.1.with_alpha(0.5)),
        ChildOf(body),
        children![(Text::new(look.category.0), bold(14.0), TextColor(look.category.1))],
    ));
    commands.spawn((
        Node { width: Val::Percent(55.0), height: Val::Px(2.0), margin: UiRect::vertical(Val::Px(2.0)), ..default() },
        BackgroundColor(rarity.with_alpha(0.35)),
        ChildOf(body),
    ));
    // Description: "+x% Stat" lines stand out as bright stats, the rest is prose.
    let desc = commands
        .spawn((
            Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(2.0), ..default() },
            ChildOf(body),
        ))
        .id();
    for line in look.description.lines() {
        let (text_font, color) = if line.starts_with(['+', '-', '−']) {
            (bold(17.0), STAT)
        } else if line.starts_with('(') {
            (font(15.0), TEXT_DIM)
        } else {
            (font(16.0), Color::srgb(0.85, 0.83, 0.88))
        };
        commands.spawn((
            Text::new(line),
            text_font,
            TextColor(color),
            TextLayout::justify(Justify::Center),
            ChildOf(desc),
        ));
    }
    commands.spawn((Node { flex_grow: 1.0, ..default() }, ChildOf(body)));
    // Key cap "1", "2"... lights up in the rarity color on hover.
    commands.spawn((
        KeyCap { index: i },
        Node {
            min_width: Val::Px(36.0),
            height: Val::Px(36.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::new(Val::Px(2.0), Val::Px(2.0), Val::Px(2.0), Val::Px(4.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(KEY_BG),
        BorderColor::all(TEXT_DIM.with_alpha(0.6)),
        ChildOf(body),
        children![(Text::new(format!("{}", i + 1)), bold(19.0), TextColor(TEXT))],
    ));
    if look.tier == Rarity::Legendary {
        commands.spawn((
            Shimmer { index: i },
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(-25.0),
                left: Val::Percent(-60.0),
                width: Val::Percent(16.0),
                height: Val::Percent(150.0),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 0.95, 0.8, 0.18)),
            UiTransform::from_rotation(Rot2::degrees(20.0)),
            ChildOf(card),
        ));
    }
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

const KEY_BG: Color = Color::srgb(0.12, 0.1, 0.17);

fn mix(a: Color, b: Color, t: f32) -> Color {
    let (a, b, t) = (a.to_srgba(), b.to_srgba(), t.clamp(0.0, 1.0));
    Color::srgba(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
        a.alpha + (b.alpha - a.alpha) * t,
    )
}

fn ease_out_cubic(x: f32) -> f32 {
    1.0 - (1.0 - x).powi(3)
}

/// Overshoots a little past 1 then settles: the "pop" of the cards.
fn ease_out_back(x: f32) -> f32 {
    let c1 = 1.70158;
    1.0 + (c1 + 1.0) * (x - 1.0).powi(3) + c1 * (x - 1.0).powi(2)
}

/// Dims the world behind and slams the title in.
fn animate_overlay(
    time: Res<Time>,
    mut overlay: Query<(&CardsOverlay, &mut BackgroundColor)>,
    mut header: Query<&mut UiTransform, With<CardsHeader>>,
) {
    let Ok((o, mut bg)) = overlay.single_mut() else { return };
    let t = time.elapsed_secs() - o.opened;
    bg.0 = bg.0.with_alpha(0.6 * (t / 0.15).clamp(0.0, 1.0));
    if let Ok(mut h) = header.single_mut() {
        let p = (t / 0.18).clamp(0.0, 1.0);
        h.scale = Vec2::splat(1.0 + 0.3 * (1.0 - ease_out_cubic(p)));
    }
}

/// Staggered entrance (rise + scale pop + rarity flash), rarity glow and hover lift.
fn animate_cards(
    time: Res<Time>,
    overlay: Query<&CardsOverlay>,
    mut cards: Query<(
        &mut CardButton,
        &Hovered,
        &mut UiTransform,
        &mut Visibility,
        &mut BackgroundColor,
        &mut BorderColor,
        &mut BoxShadow,
    )>,
) {
    let Ok(o) = overlay.single() else { return };
    let now = time.elapsed_secs();
    let t = now - o.opened;
    let k = 1.0 - (-time.delta_secs() * 20.0).exp();
    for (mut card, hovered, mut tf, mut vis, mut bg, mut border, mut shadow) in &mut cards {
        let local = (t - STAGGER * card.index as f32) / APPEAR;
        *vis = if local > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
        let p = local.clamp(0.0, 1.0);
        let target = if hovered.get() && p >= 1.0 { 1.0 } else { 0.0 };
        card.hover += (target - card.hover) * k;
        let h = card.hover;
        tf.scale = Vec2::splat((0.6 + 0.4 * ease_out_back(p)) * (1.0 + 0.05 * h));
        tf.translation = Val2::px(0.0, 80.0 * (1.0 - ease_out_cubic(p)) - 14.0 * h);
        // Bright flash of the rarity color as the card lands, fading out.
        let flash = (1.0 - p).powi(2);
        bg.0 = mix(mix(CARD_BG, CARD_BG_HOVER, h), card.rarity.with_alpha(0.95), flash * 0.7);
        let tier = card.tier as usize as f32;
        let mut edge = mix(card.rarity, Color::WHITE, flash.max(0.35 * h));
        // Rarer cards breathe; legendary also flickers to pale gold.
        let pulse = if card.tier >= Rarity::Epic { 0.8 + 0.2 * (now * 4.0 + card.index as f32).sin() } else { 1.0 };
        if card.tier == Rarity::Legendary {
            edge = mix(edge, Color::srgb(1.0, 0.95, 0.6), 0.4 * (0.5 + 0.5 * (now * 5.0).sin()));
        }
        *border = BorderColor::all(edge);
        let glow = (0.22 + 0.11 * tier) * pulse * p + 0.3 * h + 0.6 * flash * p.min(1.0);
        if let Some(s) = shadow.0.first_mut() {
            s.color = card.rarity.with_alpha(glow.min(1.0));
            s.blur_radius = Val::Px(14.0 + 5.0 * tier + 10.0 * h + 18.0 * flash);
            s.spread_radius = Val::Px(1.0 + tier + 4.0 * h);
        }
    }
}

/// A light streak sweeps across legendary cards: once as they land, then every 1.8 s.
fn animate_shimmer(time: Res<Time>, overlay: Query<&CardsOverlay>, mut streaks: Query<(&Shimmer, &mut Node)>) {
    let Ok(o) = overlay.single() else { return };
    for (s, mut node) in &mut streaks {
        let since = time.elapsed_secs() - o.opened - STAGGER * s.index as f32 - APPEAR * 0.6;
        let phase = since.max(0.0) % 1.8 / 0.6;
        node.left = Val::Percent(if since > 0.0 && phase <= 1.0 { -40.0 + 160.0 * phase } else { -60.0 });
    }
}

/// The key cap of the hovered card fills with its rarity color.
fn animate_keycaps(cards: Query<&CardButton>, mut caps: Query<(&KeyCap, &mut BackgroundColor, &mut BorderColor)>) {
    for (cap, mut bg, mut border) in &mut caps {
        let Some(card) = cards.iter().find(|c| c.index == cap.index) else { continue };
        bg.0 = mix(KEY_BG, card.rarity.with_alpha(0.85), card.hover * 0.8);
        *border = BorderColor::all(mix(TEXT_DIM.with_alpha(0.6), card.rarity, 0.4 + 0.6 * card.hover));
    }
}
