//! In-game HUD and pause menu.

use bevy::prelude::*;

use super::widgets::*;
use super::{AppState, Paused};
use crate::config::AppConfig;
use crate::game::chests::INTERACT_RANGE;
use crate::game::players::REVIVE_RANGE;
use crate::net::client::Prediction;
use crate::net::protocol::*;
use crate::net::{LocalPlayer, NetCommand};
use crate::render::fx::HudFeedback;
use crate::render::palette::{GLOW_CYAN, GLOW_MAGENTA, GOLD};
use crate::render::MainCamera;
use crate::shared::economy;
use crate::shared::items::{ItemKind, PlayerStats};
use crate::shared::build::{auto_params, main_params, owned_tomes, tome_points, Offer};

/// Every HUD text rewritten by a system, by role (one query serves them all).
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum HudText {
    Hp,
    Gold,
    Timer,
    Map,
    Swarm,
    Mobs,
    Alive,
    Totems,
    Level,
    Weapon,
    WeaponLevel,
    Ammo,
    Slots,
    Jumps,
}

/// The layers of the health bar.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum HpPart {
    Fill,
    /// Pale bar that lags behind the fill after a hit (shows the damage taken).
    Trail,
    Shield,
}

#[derive(Component)]
struct HpFrame;
#[derive(Component)]
struct GoldRow;
#[derive(Component)]
struct TotemChip;
#[derive(Component)]
struct XpFill;
#[derive(Component)]
struct LevelBadge;
#[derive(Component)]
struct ReloadFill;
#[derive(Component)]
struct SlotsPanel;
#[derive(Component)]
struct DashPip(usize);
#[derive(Component)]
struct ItemsRow;
#[derive(Component)]
struct PromptText;
#[derive(Component)]
struct PromptBar;
#[derive(Component)]
struct DownedText;
#[derive(Component)]
struct ToastList;
/// A toast: slides in from the right with a colored flash, then fades out.
#[derive(Component)]
struct Toast {
    age: f32,
    life: f32,
    color: Color,
}
#[derive(Component)]
struct ToastAccent;
#[derive(Component)]
struct ToastText;
#[derive(Component)]
struct BossBar;
#[derive(Component)]
struct BossFill;
#[derive(Component)]
struct BossName;
#[derive(Component)]
struct TeamText;
#[derive(Component)]
struct HitmarkerRoot;
#[derive(Component)]
struct Hitmarker;
#[derive(Component)]
struct HurtVignette;
#[derive(Component)]
struct PauseMenu;
#[derive(Component)]
struct PauseValues;
#[derive(Component)]
struct WaveBanner(f32);
/// Marker following one of the last monsters of a wave.
#[derive(Component)]
struct EnemyMarker(Entity);

/// A label following a 3D entity on screen (names, chest prices).
#[derive(Component)]
struct WorldLabel {
    target: Entity,
    height: f32,
    max_distance: f32,
}

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), spawn_hud)
            .add_systems(
                Update,
                (
                    update_vitals,
                    update_weapon,
                    update_items,
                    update_prompt,
                    update_team_and_boss,
                    update_feedback,
                    toasts_from_events,
                    unlock_next_map,
                    age_toasts,
                    spawn_world_labels,
                    place_world_labels,
                    toggle_pause,
                    pause_actions,
                    wave_banner,
                    enemy_markers,
                    update_xp_bar,
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

/// HUD colors (on top of `widgets.rs`).
const HUD_BG: Color = Color::srgba(0.06, 0.04, 0.09, 0.72);
const HUD_EDGE: Color = Color::srgba(0.0, 0.0, 0.0, 0.85);
const XP_COLOR: Color = Color::srgb(0.30, 0.78, 1.0);
const XP_LIGHT: Color = Color::srgb(0.72, 0.93, 1.0);
const HP_GREEN: Color = Color::srgb(0.33, 0.86, 0.42);
const HP_AMBER: Color = Color::srgb(0.98, 0.70, 0.22);
const HP_RED: Color = Color::srgb(0.86, 0.16, 0.16);
const HP_RED_HOT: Color = Color::srgb(1.0, 0.45, 0.38);
const HP_TRAIL: Color = Color::srgb(1.0, 0.88, 0.68);
const DARK_INK: Color = Color::srgb(0.12, 0.08, 0.10);

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_srgba(), b.to_srgba());
    let t = t.clamp(0.0, 1.0);
    Color::srgba(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
        a.alpha + (b.alpha - a.alpha) * t,
    )
}

/// A tight drop shadow: the default one (4 px) smears small HUD text.
fn small_shadow() -> TextShadow {
    TextShadow { offset: Vec2::splat(1.5), color: Color::srgba(0.0, 0.0, 0.0, 0.85) }
}

fn hud_text(kind: HudText, size: f32, strong: bool, color: Color) -> impl Bundle {
    (Text::new(""), if strong { bold(size) } else { font(size) }, TextColor(color), small_shadow(), kind)
}

/// A small rounded tag ("Monstres niv. 3").
fn chip(kind: HudText, color: Color) -> impl Bundle {
    (
        Node {
            padding: UiRect::axes(Val::Px(8.0), Val::Px(1.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
        BorderColor::all(color.with_alpha(0.45)),
        children![hud_text(kind, 13.0, true, color)],
    )
}

fn drop_shadow() -> BoxShadow {
    BoxShadow::new(Color::srgba(0.0, 0.0, 0.0, 0.45), Val::Px(0.0), Val::Px(3.0), Val::Px(0.0), Val::Px(8.0))
}

fn abs_node(left: Option<f32>, right: Option<f32>, top: Option<f32>, bottom: Option<f32>) -> Node {
    let v = |o: Option<f32>| o.map(Val::Px).unwrap_or(Val::Auto);
    Node {
        position_type: PositionType::Absolute,
        left: v(left),
        right: v(right),
        top: v(top),
        bottom: v(bottom),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(4.0),
        ..default()
    }
}

fn bar(width: f32, height: f32, fill_color: Color, marker: impl Component) -> impl Bundle {
    (
        Node { width: Val::Px(width), height: Val::Px(height), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        Outline::new(Val::Px(1.0), Val::ZERO, Color::srgba(0.0, 0.0, 0.0, 0.6)),
        children![(
            marker,
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(fill_color),
        )],
    )
}

/// A glossy highlight over the top of a bar fill.
fn gloss() -> impl Bundle {
    (
        Node { position_type: PositionType::Absolute, top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(42.0), ..default() },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.22)),
    )
}

/// The root node of the in-game HUD (the studio hides it).
#[derive(Component)]
pub struct HudRoot;

fn spawn_hud(mut commands: Commands) {
    let root = commands
        .spawn((
            HudRoot,
            DespawnOnExit(AppState::InGame),
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        ))
        .id();
    let child = |commands: &mut Commands, bundle: (Node,)| commands.spawn((bundle, ChildOf(root))).id();

    // Hurt vignette (full screen, transparent red border).
    commands.spawn((
        HurtVignette,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            border: UiRect::all(Val::Px(70.0)),
            ..default()
        },
        BorderColor::all(Color::srgba(0.8, 0.0, 0.05, 0.0)),
        ChildOf(root),
    ));

    // Team XP bar along the top edge, with the team level hanging below it.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            left: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Px(10.0),
            border: UiRect::bottom(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.03, 0.02, 0.07, 0.8)),
        BorderColor::all(XP_COLOR.with_alpha(0.4)),
        ChildOf(root),
        children![(
            XpFill,
            Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() },
            BackgroundColor(XP_COLOR),
            children![gloss()],
        )],
    ));
    commands.spawn((
        Node { position_type: PositionType::Absolute, top: Val::Px(10.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() },
        ChildOf(root),
        children![(
            LevelBadge,
            Node {
                padding: UiRect::new(Val::Px(18.0), Val::Px(18.0), Val::Px(0.0), Val::Px(2.0)),
                border: UiRect::new(Val::Px(1.0), Val::Px(1.0), Val::Px(0.0), Val::Px(1.0)),
                border_radius: BorderRadius::bottom(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.02, 0.07, 0.8)),
            BorderColor::all(XP_COLOR.with_alpha(0.4)),
            UiTransform::IDENTITY,
            children![hud_text(HudText::Level, 15.0, true, XP_LIGHT)],
        )],
    ));

    // Crosshair: a dot + 4 ticks, outlined so it reads on any background.
    let center = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                width: Val::Px(0.0),
                height: Val::Px(0.0),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    for (x, y, w, h) in [(-1.5, -1.5, 3.0, 3.0), (-13.0, -1.0, 7.0, 2.0), (6.0, -1.0, 7.0, 2.0), (-1.0, -13.0, 2.0, 7.0), (-1.0, 6.0, 2.0, 7.0)] {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x),
                top: Val::Px(y),
                width: Val::Px(w),
                height: Val::Px(h),
                border_radius: BorderRadius::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.95)),
            Outline::new(Val::Px(1.0), Val::ZERO, Color::srgba(0.0, 0.0, 0.0, 0.55)),
            ChildOf(center),
        ));
    }
    // Hitmarker: 4 diagonal strokes around the crosshair (pops on hit).
    let hit = commands
        .spawn((
            HitmarkerRoot,
            Node { position_type: PositionType::Absolute, width: Val::Px(0.0), height: Val::Px(0.0), ..default() },
            UiTransform::IDENTITY,
            ChildOf(center),
        ))
        .id();
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let angle: f32 = if sx * sy > 0.0 { -45.0 } else { 45.0 };
        commands.spawn((
            Hitmarker,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(sx * 11.0 - 1.5),
                top: Val::Px(sy * 11.0 - 6.0),
                width: Val::Px(3.0),
                height: Val::Px(12.0),
                border_radius: BorderRadius::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiTransform::from_rotation(Rot2::degrees(angle)),
            ChildOf(hit),
        ));
    }

    // Bottom-left: items, gold, health.
    let bl = child(&mut commands, (Node { row_gap: Val::Px(8.0), ..abs_node(Some(24.0), None, None, Some(24.0)) },));
    commands.spawn((
        ItemsRow,
        Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(5.0), row_gap: Val::Px(5.0), max_width: Val::Px(400.0), ..default() },
        ChildOf(bl),
    ));
    // Gold: a coin drawn from two circles + the amount.
    commands.spawn((
        GoldRow,
        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() },
        UiTransform::IDENTITY,
        ChildOf(bl),
        children![
            (
                Node {
                    width: Val::Px(24.0),
                    height: Val::Px(24.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::MAX,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(GOLD),
                BorderColor::all(Color::srgb(0.62, 0.38, 0.08)),
                drop_shadow(),
                children![(
                    Node { width: Val::Px(10.0), height: Val::Px(10.0), border: UiRect::all(Val::Px(2.0)), border_radius: BorderRadius::MAX, ..default() },
                    BorderColor::all(Color::srgb(1.0, 0.93, 0.62)),
                )],
            ),
            hud_text(HudText::Gold, 28.0, true, GOLD),
        ],
    ));
    // Health: framed bar with a damage trail, a shield strip and quarter ticks.
    let frame = commands
        .spawn((
            HpFrame,
            Node {
                width: Val::Px(380.0),
                height: Val::Px(32.0),
                border: UiRect::all(Val::Px(2.0)),
                padding: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(HUD_BG),
            BorderColor::all(HUD_EDGE),
            drop_shadow(),
            ChildOf(bl),
        ))
        .id();
    let inner = commands
        .spawn((Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, ChildOf(frame)))
        .id();
    let layer = |w: f32| Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        width: Val::Percent(w),
        height: Val::Percent(100.0),
        border_radius: BorderRadius::all(Val::Px(5.0)),
        ..default()
    };
    commands.spawn((HpPart::Trail, layer(100.0), BackgroundColor(HP_TRAIL), ChildOf(inner)));
    commands.spawn((HpPart::Fill, layer(100.0), BackgroundColor(HP_GREEN), ChildOf(inner), children![gloss()]));
    commands.spawn((
        HpPart::Shield,
        Node { top: Val::Auto, bottom: Val::Px(0.0), height: Val::Px(5.0), ..layer(0.0) },
        BackgroundColor(GLOW_CYAN),
        ChildOf(inner),
    ));
    for i in 1..4 {
        commands.spawn((
            Node { position_type: PositionType::Absolute, left: Val::Percent(i as f32 * 25.0), top: Val::Px(0.0), width: Val::Px(1.0), height: Val::Percent(100.0), ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.3)),
            ChildOf(inner),
        ));
    }
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            padding: UiRect::horizontal(Val::Px(8.0)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        },
        ChildOf(inner),
        children![(Text::new("PV"), bold(14.0), TextColor(TEXT), small_shadow()), hud_text(HudText::Hp, 17.0, true, TEXT)],
    ));

    // Bottom-right: automatic weapons, then the main weapon card.
    let br = child(&mut commands, (Node { align_items: AlignItems::FlexEnd, row_gap: Val::Px(8.0), ..abs_node(None, Some(24.0), None, Some(24.0)) },));
    commands.spawn((
        SlotsPanel,
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            border: UiRect::right(Val::Px(3.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.06, 0.04, 0.09, 0.5)),
        BorderColor::all(TEXT_DIM.with_alpha(0.6)),
        ChildOf(br),
        children![(hud_text(HudText::Slots, 15.0, false, TEXT), TextLayout::justify(Justify::Right))],
    ));
    let card = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                min_width: Val::Px(290.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                row_gap: Val::Px(5.0),
                border: UiRect::right(Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(HUD_BG),
            BorderColor::all(ACCENT),
            drop_shadow(),
            ChildOf(br),
        ))
        .id();
    commands.spawn((
        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(10.0), ..default() },
        ChildOf(card),
        children![
            hud_text(HudText::Weapon, 24.0, true, TEXT),
            (
                Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(0.0)), border_radius: BorderRadius::MAX, ..default() },
                BackgroundColor(ACCENT),
                children![(Text::new(""), bold(14.0), TextColor(DARK_INK), HudText::WeaponLevel)],
            ),
        ],
    ));
    commands.spawn((hud_text(HudText::Ammo, 16.0, false, TEXT), ChildOf(card)));
    commands.spawn((bar(258.0, 6.0, ACCENT, ReloadFill), ChildOf(card)));
    let dash = commands
        .spawn((
            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), margin: UiRect::top(Val::Px(2.0)), ..default() },
            ChildOf(card),
        ))
        .id();
    commands.spawn((Text::new("DASH"), bold(12.0), TextColor(TEXT_DIM), small_shadow(), ChildOf(dash)));
    for i in 0..6 {
        commands.spawn((
            DashPip(i),
            Node { width: Val::Px(16.0), height: Val::Px(7.0), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.12)),
            ChildOf(dash),
        ));
    }
    commands.spawn((Node { margin: UiRect::left(Val::Px(10.0)), ..default() }, ChildOf(dash), children![hud_text(HudText::Jumps, 12.0, true, TEXT_DIM)]));

    // Top-left: run info (timer, map, swarm countdown, tags).
    let tl = child(&mut commands, (abs_node(Some(24.0), None, Some(22.0), None),));
    let run = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(14.0),
                padding: UiRect::new(Val::Px(14.0), Val::Px(16.0), Val::Px(8.0), Val::Px(9.0)),
                border: UiRect::left(Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(HUD_BG),
            BorderColor::all(ACCENT),
            drop_shadow(),
            ChildOf(tl),
        ))
        .id();
    commands.spawn((hud_text(HudText::Timer, 38.0, true, TEXT), ChildOf(run)));
    let info = commands
        .spawn((Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() }, ChildOf(run)))
        .id();
    commands.spawn((hud_text(HudText::Map, 16.0, true, TEXT), ChildOf(info)));
    commands.spawn((hud_text(HudText::Swarm, 15.0, false, TEXT_DIM), ChildOf(info)));
    commands.spawn((
        Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), margin: UiRect::top(Val::Px(2.0)), ..default() },
        ChildOf(info),
        children![
            chip(HudText::Mobs, Color::srgb(0.85, 0.8, 1.0)),
            chip(HudText::Alive, Color::srgb(1.0, 0.62, 0.55)),
            (TotemChip, chip(HudText::Totems, Color::srgb(1.0, 0.4, 0.35))),
        ],
    ));

    // Top-right: team.
    let tr = child(&mut commands, (Node { align_items: AlignItems::FlexEnd, ..abs_node(None, Some(24.0), Some(22.0), None) },));
    commands.spawn((label("", 18.0, TEXT), TeamText, ChildOf(tr)));

    // Top-center: boss bar (below the team level badge).
    let top = commands
        .spawn((
            BossBar,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(46.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                ..default()
            },
            Visibility::Hidden,
            ChildOf(root),
        ))
        .id();
    commands.spawn((Text::new(""), bold(24.0), TextColor(crate::render::palette::GLOW_ORANGE), TextShadow::default(), BossName, ChildOf(top)));
    commands.spawn((bar(520.0, 14.0, GLOW_MAGENTA, BossFill), ChildOf(top)));

    // Center: prompts and downed message.
    let mid = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(58.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(6.0),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    commands.spawn((Text::new(""), bold(40.0), TextColor(Color::srgb(1.0, 0.3, 0.3)), TextShadow::default(), DownedText, ChildOf(mid)));
    commands.spawn((label("", 22.0, TEXT), PromptText, ChildOf(mid)));
    commands.spawn((bar(220.0, 8.0, Color::srgb(0.4, 0.95, 0.6), PromptBar), ChildOf(mid)));

    // Center-top: big wave banner (fades out).
    commands.spawn((
        WaveBanner(0.0),
        Text::new(""),
        bold(64.0),
        TextColor(Color::NONE),
        TextShadow { offset: Vec2::splat(4.0), ..default() },
        Node { position_type: PositionType::Absolute, top: Val::Percent(22.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() },
        TextLayout::justify(Justify::Center),
        ChildOf(root),
    ));

    // Right: toasts.
    commands.spawn((
        ToastList,
        Node { align_items: AlignItems::FlexEnd, row_gap: Val::Px(6.0), ..abs_node(None, Some(24.0), Some(200.0), None) },
        ChildOf(root),
    ));

    // Pause menu (hidden).
    let pause = commands
        .spawn((
            PauseMenu,
            Node { position_type: PositionType::Absolute, ..screen_root() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            Visibility::Hidden,
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        panel(),
        ChildOf(pause),
        children![
            (Text::new("PAUSE"), bold(44.0), TextColor(ACCENT)),
            label("(la partie continue pour les autres)", 16.0, TEXT_DIM),
            (label("", 20.0, TEXT), PauseValues),
            (
                Node { flex_direction: FlexDirection::Row, ..default() },
                children![button("Sensib. −", UiAction::SensDown, 150.0), button("Sensib. +", UiAction::SensUp, 150.0)]
            ),
            (
                Node { flex_direction: FlexDirection::Row, ..default() },
                children![button("Volume −", UiAction::VolDown, 150.0), button("Volume +", UiAction::VolUp, 150.0)]
            ),
            (
                Node { flex_direction: FlexDirection::Row, ..default() },
                children![button("FOV −", UiAction::FovDown, 150.0), button("FOV +", UiAction::FovUp, 150.0)]
            ),
            button("Reprendre", UiAction::Resume, 308.0),
            button("Quitter la partie", UiAction::Leave, 308.0),
        ],
    ));
}

pub fn chest_color(kind: economy::ChestKind) -> Color {
    match kind {
        economy::ChestKind::Small => crate::render::palette::GOLD,
        economy::ChestKind::Large => crate::render::palette::GLOW_ORANGE,
        economy::ChestKind::Legendary => crate::render::palette::GLOW_MAGENTA,
        economy::ChestKind::Weapon => crate::render::palette::TEAL_GLOW,
    }
}

/// Only touches the text if it really changed: rewriting a `Text` every frame
/// forces a new text layout (and, with system fonts, leaked a GPU texture
/// per frame until the graphics card ran out of memory).
fn set_text(text: &mut Text, value: impl Into<String>) {
    let value = value.into();
    if text.0 != value {
        text.0 = value;
    }
}

type LocalQuery<'a> = (&'a Player, &'a Health, &'a Wallet, &'a Items);

fn local_player<'a>(local: &LocalPlayer, q: &'a Query<LocalQuery>) -> Option<(&'a Player, &'a Health, &'a Wallet, &'a Items)> {
    q.iter().find(|(p, ..)| Some(p.slot) == local.slot)
}

/// Animation state of the health bar and gold counter.
#[derive(Default)]
struct VitalsAnim {
    /// Where the damage trail stands (fraction of max health).
    trail: f32,
    /// Seconds before the trail starts to catch up with the fill.
    hold: f32,
    gold: Option<u32>,
    /// Gold "pop" when the amount changes (1 -> 0).
    pop: f32,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_vitals(
    time: Res<Time>,
    local: Res<LocalPlayer>,
    players: Query<LocalQuery>,
    info: Query<&MatchInfo>,
    mut texts: Query<(&mut Text, &mut TextColor, &HudText)>,
    mut parts: Query<(&mut Node, &mut BackgroundColor, &HpPart), Without<TotemChip>>,
    mut frame: Query<&mut BorderColor, With<HpFrame>>,
    mut gold_row: Query<&mut UiTransform, With<GoldRow>>,
    mut totem_chip: Query<&mut Node, (With<TotemChip>, Without<HpPart>)>,
    mut anim: Local<VitalsAnim>,
) {
    let dt = time.delta_secs();
    let mut values: Vec<(HudText, String, Option<Color>)> = Vec::new();
    if let Some((_, health, wallet, _)) = local_player(&local, &players) {
        let frac = (health.hp / health.max.max(1.0)).clamp(0.0, 1.0) * 0.22; anim.trail = anim.trail.max(0.55); anim.hold = 9.0; // TEMPDBG
        // Damage trail: holds a moment, then slides down to the new value.
        if frac >= anim.trail {
            anim.trail = frac;
            anim.hold = 0.45;
        } else if anim.hold > 0.0 {
            anim.hold -= dt;
        } else {
            anim.trail = (anim.trail - dt * 0.6).max(frac);
        }
        // Low health: the bar and its frame pulse red.
        let low = frac < 0.3;
        let pulse = if low { 0.5 + 0.5 * (time.elapsed_secs() * 7.0).sin() } else { 0.0 };
        let fill_color = if low {
            lerp_color(HP_RED, HP_RED_HOT, pulse)
        } else if frac < 0.5 {
            lerp_color(HP_AMBER, HP_GREEN, (frac - 0.3) / 0.2 * 0.35)
        } else {
            HP_GREEN
        };
        for (mut node, mut bg, part) in &mut parts {
            let (width, color) = match part {
                HpPart::Fill => (frac, fill_color),
                HpPart::Trail => (anim.trail, HP_TRAIL),
                HpPart::Shield => (0.15, GLOW_CYAN), // TEMPDBG
            };
            node.width = Val::Percent(width * 100.0);
            if bg.0 != color {
                bg.0 = color;
            }
        }
        if let Ok(mut border) = frame.single_mut() {
            *border = BorderColor::all(if low { lerp_color(HUD_EDGE, Color::srgb(1.0, 0.2, 0.15), 0.4 + 0.6 * pulse) } else { HUD_EDGE });
        }
        let hp = if health.shield > 0.5 {
            format!("{:.0} / {:.0}  +{:.0}", health.hp.max(0.0), health.max, health.shield)
        } else {
            format!("{:.0} / {:.0}", health.hp.max(0.0), health.max)
        };
        values.push((HudText::Hp, hp, Some(if low { lerp_color(TEXT, Color::srgb(1.0, 0.75, 0.7), pulse) } else { TEXT })));

        // Gold: the counter pops when it changes.
        if anim.gold != Some(wallet.gold) {
            if anim.gold.is_some() {
                anim.pop = 1.0;
            }
            anim.gold = Some(wallet.gold);
        }
        anim.pop = (anim.pop - dt * 4.0).max(0.0);
        if let Ok(mut tr) = gold_row.single_mut() {
            let s = 1.0 + 0.22 * anim.pop * anim.pop; let s = if s > 0.0 { 1.2 } else { s }; // TEMPDBG
            if tr.scale.x != s {
                tr.scale = Vec2::splat(s);
            }
        }
        values.push((HudText::Gold, format!("{}", wallet.gold), None));
    }
    if let Ok(m) = info.single() {
        let secs = m.elapsed as u32;
        values.push((HudText::Timer, format!("{:02}:{:02}", secs / 60, secs % 60), None));
        values.push((HudText::Map, format!("{}  ·  {}", m.rules.map.name(), m.rules.difficulty.name()), None));
        let (swarm, color) = match m.swarm {
            0 if m.elapsed < economy::FINAL_SWARM_TIME => {
                let left = (economy::FINAL_SWARM_TIME - m.elapsed) as u32;
                let color = if left < 60 { ACCENT } else { TEXT_DIM };
                (format!("Essaim final dans {:02}:{:02}", left / 60, left % 60), color)
            }
            1 => {
                let left = (economy::BLACK_GHOSTS_TIME - m.elapsed).max(0.0) as u32;
                (format!("ESSAIM FINAL — fantômes blancs (noirs dans {:02}:{:02})", left / 60, left % 60), GLOW_MAGENTA)
            }
            _ => ("ESSAIM FINAL — fantômes noirs".to_string(), GLOW_MAGENTA),
        };
        values.push((HudText::Swarm, swarm, Some(color)));
        values.push((HudText::Mobs, format!("Monstres niv. {}", m.level), None));
        values.push((HudText::Alive, format!("{} en vie", m.enemies_left), None));
        if m.rules.totems > 0 {
            values.push((HudText::Totems, format!("Totems ×{}  ·  chance +{:.0}%", m.rules.totems, m.rules.luck_bonus() * 35.0), None));
        }
        if let Ok(mut node) = totem_chip.single_mut() {
            let want = if m.rules.totems > 0 { Display::Flex } else { Display::None };
            if node.display != want {
                node.display = want;
            }
        }
    }
    for (mut text, mut color, kind) in &mut texts {
        if let Some((_, value, c)) = values.iter().find(|(k, ..)| k == kind) {
            set_text(&mut text, value.as_str());
            if let Some(c) = c
                && color.0 != *c
            {
                color.0 = *c;
            }
        }
    }
}

/// The team XP bar at the very top of the screen, and the level badge (flashes on level up).
#[allow(clippy::type_complexity)]
fn update_xp_bar(
    time: Res<Time>,
    info: Query<&MatchInfo>,
    mut fill: Query<(&mut Node, &mut BackgroundColor), With<XpFill>>,
    mut badge: Query<(&mut UiTransform, &mut BorderColor), With<LevelBadge>>,
    mut texts: Query<(&mut Text, &HudText)>,
    mut flash: Local<(u32, f32)>,
) {
    let Ok(m) = info.single() else { return };
    if flash.0 != m.team_level {
        if flash.0 != 0 {
            flash.1 = 1.0;
        }
        flash.0 = m.team_level;
    }
    flash.1 = (flash.1 - time.delta_secs() * 1.5).max(0.0);
    let f = flash.1;
    if let Ok((mut node, mut bg)) = fill.single_mut() {
        node.width = Val::Percent((m.xp / m.xp_next.max(1.0)).clamp(0.0, 1.0) * 100.0);
        bg.0 = lerp_color(XP_COLOR, Color::WHITE, f * 0.8);
    }
    if let Ok((mut tr, mut border)) = badge.single_mut() {
        let s = 1.0 + 0.3 * f * f;
        if tr.scale.x != s {
            tr.scale = Vec2::splat(s);
        }
        *border = BorderColor::all(lerp_color(XP_COLOR.with_alpha(0.4), XP_LIGHT, f));
    }
    for (mut t, kind) in &mut texts {
        if *kind == HudText::Level {
            set_text(&mut t, format!("NIVEAU {}", m.team_level));
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_weapon(
    pred: Res<Prediction>,
    local: Res<LocalPlayer>,
    players: Query<LocalQuery>,
    mut texts: Query<(&mut Text, &HudText)>,
    mut reload: Query<(&mut Node, &mut BackgroundColor), (With<ReloadFill>, Without<DashPip>)>,
    mut pips: Query<(&DashPip, &mut Node, &mut BackgroundColor), Without<ReloadFill>>,
    mut slots_panel: Query<&mut Node, (With<SlotsPanel>, Without<ReloadFill>, Without<DashPip>)>,
    gears: Query<(&Player, &Gear)>,
) {
    if !pred.active {
        return;
    }
    let Some((me, _, _, items)) = local_player(&local, &players) else { return };
    let Some((_, gear)) = gears.iter().find(|(p, _)| p.slot == me.slot) else { return };
    let stats = PlayerStats::from_inventory(&items.0);
    let g = &gear.0;
    let p = main_params(g.main, &g.main_up, &stats);
    // Automatic weapons, then tomes.
    let mut lines: Vec<String> = g
        .autos
        .iter()
        .map(|(a, up)| {
            let ap = auto_params(*a, up, &stats);
            format!("{}  Nv {}  ·  {:.0}", a.def().name, up.level, ap.damage)
        })
        .collect();
    let tomes: Vec<String> = owned_tomes(&items.0)
        .into_iter()
        .map(|t| format!("{} ×{:.1}", t.name().trim_start_matches("Tome "), tome_points(&items.0, t)))
        .collect();
    if !tomes.is_empty() {
        lines.push(format!("Tomes : {}", tomes.join(", ")));
    }
    if let Ok(mut node) = slots_panel.single_mut() {
        let want = if lines.is_empty() { Display::None } else { Display::Flex };
        if node.display != want {
            node.display = want;
        }
    }
    let values = [
        (HudText::Weapon, g.main.def().name.to_string()),
        (HudText::WeaponLevel, format!("NV {}", g.main_up.level + 1)),
        (HudText::Ammo, format!("{:.0} dégâts  ·  ×{}  ·  {:.1} tirs/s", p.damage, p.count, 1.0 / p.interval.max(0.01))),
        (HudText::Slots, lines.join("\n")),
        (HudText::Jumps, format!("SAUTS {}", stats.movement.air_jumps + 1)),
    ];
    for (mut t, kind) in &mut texts {
        if let Some((_, v)) = values.iter().find(|(k, _)| k == kind) {
            set_text(&mut t, v.as_str());
        }
    }
    if let Ok((mut node, mut bg)) = reload.single_mut() {
        // Main weapon cooldown: dim while charging, bright when ready.
        let frac = (1.0 - pred.sim.gun.cooldown / p.interval.max(0.01)).clamp(0.0, 1.0);
        node.width = Val::Percent(frac * 100.0);
        let c = if frac >= 1.0 { ACCENT } else { ACCENT.with_alpha(0.55) };
        if bg.0 != c {
            bg.0 = c;
        }
    }
    // Dash charges: full pips glow, the recharging one fills up.
    let energy = pred.sim.mv.dash_energy.max(0.0);
    let max = stats.movement.dash_charges as usize;
    for (pip, mut node, mut bg) in &mut pips {
        let want = if pip.0 < max { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
        let fill = (energy - pip.0 as f32).clamp(0.0, 1.0);
        let c = if fill >= 1.0 { GLOW_CYAN } else { lerp_color(Color::srgba(1.0, 1.0, 1.0, 0.12), GLOW_CYAN.with_alpha(0.5), fill) };
        if bg.0 != c {
            bg.0 = c;
        }
    }
}

fn update_items(
    mut commands: Commands,
    local: Res<LocalPlayer>,
    icons: Res<crate::render::icons::Icons>,
    players: Query<(&Player, Ref<Items>)>,
    row: Query<(Entity, Option<&Children>), With<ItemsRow>>,
    mut shown: Local<Option<crate::shared::items::Inventory>>,
) {
    let Some((_, items)) = players.iter().find(|(p, _)| Some(p.slot) == local.slot) else { return };
    let Ok((row, children)) = row.single() else { return };
    if shown.as_ref() == Some(&items.0) && children.is_some() {
        return;
    }
    *shown = Some(items.0);
    if let Some(children) = children {
        for c in children.iter() {
            commands.entity(c).try_despawn();
        }
    }
    for item in ItemKind::ALL {
        let n = items.0.count(item);
        if n == 0 {
            continue;
        }
        let [r, g, b] = item.rarity().color();
        let rarity = Color::srgb(r, g, b);
        let cell = commands
            .spawn((
                Node {
                    width: Val::Px(42.0),
                    height: Val::Px(42.0),
                    border: UiRect::all(Val::Px(2.0)),
                    padding: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(7.0)),
                    ..default()
                },
                BorderColor::all(rarity),
                BackgroundGradient::from(LinearGradient::to_top(vec![
                    ColorStop::auto(Color::srgba(r * 0.45, g * 0.45, b * 0.45, 0.85)),
                    ColorStop::auto(Color::srgba(0.05, 0.04, 0.08, 0.8)),
                ])),
                drop_shadow(),
                ChildOf(row),
            ))
            .id();
        commands.spawn((
            ImageNode::new(icons.items.get(&item).cloned().unwrap_or_default()),
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            ChildOf(cell),
        ));
        if n > 1 {
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(-6.0),
                    bottom: Val::Px(-6.0),
                    min_width: Val::Px(17.0),
                    padding: UiRect::horizontal(Val::Px(4.0)),
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(rarity),
                Outline::new(Val::Px(1.5), Val::ZERO, Color::srgba(0.0, 0.0, 0.0, 0.7)),
                ChildOf(cell),
                children![(Text::new(format!("{n}")), bold(12.0), TextColor(DARK_INK))],
            ));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn update_prompt(
    local: Res<LocalPlayer>,
    pred: Res<Prediction>,
    players: Query<LocalQuery>,
    states: Query<(&Player, &PlayerState, &Health)>,
    chests: Query<(&Chest, &ChestOpen)>,
    statues: Query<(&Statue, &StatueUsed)>,
    shrines: Query<(&Shrine, &ShrineCharge)>,
    totems: Query<(&Totem, &TotemUsed)>,
    pending: Query<(&Player, &Items, Has<PendingChoice>)>,
    info: Query<&MatchInfo>,
    mut prompt: Query<&mut Text, (With<PromptText>, Without<DownedText>)>,
    mut downed: Query<&mut Text, (With<DownedText>, Without<PromptText>)>,
    mut bar: Query<(&mut Node, &ChildOf), With<PromptBar>>,
    mut bar_bg: Query<&mut Visibility, Without<PromptBar>>,
) {
    let (Ok(mut prompt), Ok(mut downed_text)) = (prompt.single_mut(), downed.single_mut()) else { return };
    let mut text = String::new();
    let mut big = String::new();
    let mut progress: Option<f32> = None;
    if let Some((me, health, wallet, _)) = local_player(&local, &players) {
        let pos = pred.sim.mv.pos;
        if health.downed {
            big = "À TERRE".into();
            text = "Un coéquipier peut te réanimer en restant près de toi (E)".into();
            progress = Some(health.revive);
        } else if let Some((p, h)) = states
            .iter()
            .filter(|(p, _, h)| p.slot != me.slot && h.downed)
            .map(|(p, s, h)| (p, s.sim.mv.pos.distance(pos), h))
            .filter(|(_, d, _)| *d < REVIVE_RANGE + 1.0)
            .map(|(p, _, h)| (p, h))
            .next()
        {
            text = format!("[E] Maintenir pour réanimer {}", p.name);
            progress = Some(h.revive);
        } else if let Some((_, charge)) = shrines.iter().find(|(s, c)| !c.used && s.pos.distance(pos) < SHRINE_RADIUS) {
            text = format!("Sanctuaire de charge : reste dans le cercle ! {:.0}%", charge.charge * 100.0);
            progress = Some(charge.charge);
        } else if totems.iter().any(|(t, u)| !u.0 && t.pos.distance(pos) < TOTEM_RANGE) {
            let n = info.single().map(|m| m.rules.totems).unwrap_or(0);
            text = format!(
                "[E] Activer le totem — monstres +{:.0}% dégâts, plus rapides et plus nombreux, mais chance +{:.0}% et récompenses +{:.0}% (actifs : {n})",
                crate::shared::rules::TOTEM_DAMAGE * 100.0,
                crate::shared::rules::TOTEM_LUCK * 35.0,
                crate::shared::rules::TOTEM_REWARDS * 100.0
            );
        } else if let Some(statue) = statues
            .iter()
            .filter(|(s, u)| !u.0 && s.pos.distance(pos) < crate::game::chests::STATUE_RANGE)
            .map(|(s, _)| s)
            .next()
        {
            let boss_alive = info.single().is_ok_and(|m| m.boss_alive);
            text = if boss_alive {
                "Statue : un boss est déjà en vie".to_string()
            } else {
                format!("[E] Réveiller la statue — invoque : {} (coffre légendaire à la clé)", statue.boss.def().name)
            };
        } else if let Some((chest, _)) = chests
            .iter()
            .filter(|(c, o)| !o.opened && c.pos.distance(pos + Vec3::Y * 0.5) < INTERACT_RANGE)
            .min_by(|a, b| a.0.pos.distance(pos).total_cmp(&b.0.pos.distance(pos)))
        {
            let elapsed = info.single().map(|m| m.elapsed).unwrap_or(0.0);
            let (items, choosing) = pending
                .iter()
                .find(|(p, ..)| p.slot == me.slot)
                .map(|(_, i, c)| (i.0, c))
                .unwrap_or_default();
            let stats = PlayerStats::from_inventory(&items);
            let price = crate::game::chests::player_price(chest.kind, wallet.chests_opened, elapsed, &stats);
            let what = chest.kind.name();
            text = if choosing {
                "Choisis d'abord ta carte !".to_string()
            } else if wallet.gold >= price {
                format!("[E] Ouvrir le {what} — {price} or")
            } else {
                format!("{what} : il te faut {price} or (tu as {})", wallet.gold)
            };
        }
    }
    if prompt.0 != text {
        prompt.0 = text;
    }
    if downed_text.0 != big {
        downed_text.0 = big;
    }
    if let Ok((mut node, parent)) = bar.single_mut() {
        node.width = Val::Percent(progress.unwrap_or(0.0).clamp(0.0, 1.0) * 100.0);
        if let Ok(mut vis) = bar_bg.get_mut(parent.parent()) {
            *vis = if progress.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        }
    }
}

#[allow(clippy::type_complexity)]
fn update_team_and_boss(
    local: Res<LocalPlayer>,
    players: Query<(&Player, Option<&Health>)>,
    bosses: Query<(&Mob, &Health)>,
    mut team: Query<&mut Text, With<TeamText>>,
    mut boss_bar: Query<&mut Visibility, With<BossBar>>,
    mut boss_fill: Query<&mut Node, With<BossFill>>,
    mut boss_name: Query<&mut Text, (With<BossName>, Without<TeamText>)>,
) {
    if let Ok(mut t) = team.single_mut() {
        let mut sorted: Vec<_> = players.iter().filter(|(p, _)| Some(p.slot) != local.slot).collect();
        sorted.sort_by_key(|(p, _)| p.slot);
        set_text(&mut t, sorted
            .iter()
            .map(|(p, h)| match h {
                Some(h) if h.downed => format!("{} — À TERRE", p.name),
                Some(h) => format!("{} — {:.0} PV", p.name, h.hp),
                None => p.name.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n"));
    }
    let boss = bosses.iter().find(|(m, _)| m.kind.is_boss());
    if let (Some((m, _)), Ok(mut name)) = (boss, boss_name.single_mut()) {
        set_text(&mut name, m.kind.def().name.to_uppercase());
    }
    if let Ok(mut vis) = boss_bar.single_mut() {
        *vis = if boss.is_some() { Visibility::Inherited } else { Visibility::Hidden };
    }
    if let (Some((_, h)), Ok(mut node)) = (boss, boss_fill.single_mut()) {
        node.width = Val::Percent((h.hp / h.max).clamp(0.0, 1.0) * 100.0);
    }
}

fn update_feedback(
    feedback: Res<HudFeedback>,
    local: Res<LocalPlayer>,
    players: Query<LocalQuery>,
    mut hitmarker: Query<&mut BackgroundColor, With<Hitmarker>>,
    mut hit_root: Query<&mut UiTransform, With<HitmarkerRoot>>,
    mut vignette: Query<&mut BorderColor, With<HurtVignette>>,
) {
    let a = (feedback.hitmarker / 0.18).clamp(0.0, 1.0);
    let c = if feedback.hitmarker_kill {
        Color::srgba(1.0, 0.3, 0.3, a)
    } else if feedback.hitmarker_crit {
        Color::srgba(1.0, 0.85, 0.2, a)
    } else {
        Color::srgba(1.0, 1.0, 1.0, a)
    };
    for mut b in &mut hitmarker {
        if b.0 != c {
            b.0 = c;
        }
    }
    if let Ok(mut tr) = hit_root.single_mut() {
        // Strokes start wide and snap in; kills hit bigger.
        let s = 1.0 + a * a * if feedback.hitmarker_kill { 0.6 } else { 0.3 };
        if tr.scale.x != s {
            tr.scale = Vec2::splat(s);
        }
    }
    let low_hp = local_player(&local, &players).map(|(_, h, ..)| 1.0 - (h.hp / h.max).clamp(0.0, 1.0)).unwrap_or(0.0);
    if let Ok(mut b) = vignette.single_mut() {
        let a = (feedback.hurt_flash * 0.9).max(if low_hp > 0.7 { (low_hp - 0.7) * 1.2 } else { 0.0 });
        *b = BorderColor::all(Color::srgba(0.8, 0.0, 0.05, a.min(0.6)));
    }
}

/// Surviving until the Final Swarm (15:00) unlocks the next map, in our own
/// settings file (like silver, unlocks belong to each player).
fn unlock_next_map(
    mut commands: Commands,
    info: Query<&MatchInfo>,
    mut config: ResMut<AppConfig>,
    list: Query<Entity, With<ToastList>>,
) {
    let Ok(m) = info.single() else { return };
    if m.phase != Phase::Playing || m.elapsed < economy::FINAL_SWARM_TIME {
        return;
    }
    let Some(next) = m.rules.map.next() else { return };
    if config.is_unlocked_map(next) {
        return;
    }
    config.maps_unlocked = next.index() + 1;
    config.save();
    info!("map unlocked: {}", next.name());
    if let Ok(list) = list.single() {
        spawn_toast(&mut commands, list, format!("Nouvelle carte débloquée : {} !", next.name()), GOLD, 9.0, 20.0);
    }
}

/// One toast line: a colored accent bar + the message (animated by `age_toasts`).
fn spawn_toast(commands: &mut Commands, list: Entity, text: String, color: Color, life: f32, size: f32) {
    commands.spawn((
        Toast { age: 0.0, life, color },
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            max_width: Val::Px(560.0),
            padding: UiRect::new(Val::Px(0.0), Val::Px(14.0), Val::Px(0.0), Val::Px(0.0)),
            border_radius: BorderRadius::all(Val::Px(7.0)),
            ..default()
        },
        BackgroundColor(Color::NONE),
        UiTransform::from_translation(Val2::px(80.0, 0.0)),
        ChildOf(list),
        children![
            (
                ToastAccent,
                Node { width: Val::Px(5.0), align_self: AlignSelf::Stretch, border_radius: BorderRadius::left(Val::Px(7.0)), ..default() },
                BackgroundColor(Color::NONE),
            ),
            (
                ToastText,
                Text::new(text),
                bold(size),
                TextColor(Color::NONE),
                small_shadow(),
                Node { margin: UiRect::vertical(Val::Px(6.0)), ..default() },
            ),
        ],
    ));
}

fn toasts_from_events(
    mut commands: Commands,
    mut events: MessageReader<GameEvent>,
    players: Query<&Player>,
    list: Query<Entity, With<ToastList>>,
) {
    let Ok(list) = list.single() else { return };
    let name = |slot: u8| players.iter().find(|p| p.slot == slot).map(|p| p.name.clone()).unwrap_or_default();
    for ev in events.read() {
        let (text, color) = match ev {
            GameEvent::ChestOpened { .. } => continue,
            GameEvent::Gained { slot, reward } => match reward {
                Reward::Item(i) => {
                    let [r, g, b] = i.rarity().color();
                    (format!("{} obtient {} — {}", name(*slot), i.name(), i.description()), Color::srgb(r, g, b))
                }
                Reward::Offer(card) => {
                    // Only new weapons are worth telling everyone about.
                    let Offer::NewAuto(a) = card.offer else { continue };
                    let [r, g, b] = card.rarity.color();
                    (format!("{} obtient l'arme {}", name(*slot), a.def().name), Color::srgb(r, g, b))
                }
            },
            GameEvent::AnkhUsed { slot } => (format!("L'Ankh a sauvé {} !", name(*slot)), crate::render::palette::GOLD),
            GameEvent::Downed { slot } => (format!("{} est à terre !", name(*slot)), Color::srgb(1.0, 0.35, 0.3)),
            GameEvent::Revived { slot } => (format!("{} est de retour !", name(*slot)), Color::srgb(0.4, 1.0, 0.6)),
            GameEvent::BossSpawned => ("Un BOSS approche !".to_string(), crate::render::palette::GLOW_ORANGE),
            GameEvent::StatueAwakened { .. } => ("Une statue s'éveille… un boss arrive !".to_string(), crate::render::palette::GLOW_MAGENTA),
            GameEvent::BossKilled => ("Le boss est tombé ! Un coffre légendaire l'attend".to_string(), crate::render::palette::GOLD),
            GameEvent::LevelUp { level } => (format!("Niveau {level} !"), Color::srgb(0.4, 1.0, 0.6)),
            GameEvent::PowerupTaken { slot, kind, .. } => (format!("{} — {}", name(*slot), kind.name()), crate::render::palette::GOLD),
            GameEvent::ShrineCharged { .. } => ("Sanctuaire chargé : amélioration gratuite !".to_string(), crate::render::palette::GLOW_CYAN),
            GameEvent::SwarmStarted { black } => (
                if *black {
                    "20:00 — Plus que des fantômes NOIRS, de plus en plus rapides… Bravo d'être encore là !".to_string()
                } else {
                    "15:00 — ESSAIM FINAL : les fantômes blancs arrivent. Survis !".to_string()
                },
                crate::render::palette::GLOW_MAGENTA,
            ),
            GameEvent::TotemActivated { slot, totems, .. } => (
                format!("{} active un totem : les monstres se renforcent, la chance aussi ! (×{totems})", name(*slot)),
                Color::srgb(1.0, 0.3, 0.3),
            ),
            GameEvent::NewMonster { kind } => (
                format!("Nouveau : {} — {}", kind.def().name, kind.hint()),
                if kind.is_boss() { crate::render::palette::GLOW_ORANGE } else { Color::srgb(0.85, 0.8, 1.0) },
            ),
            GameEvent::BossToppled { kind } => {
                (format!("{} s'effondre : TIREZ, dégâts doublés !", kind.def().name), crate::render::palette::GOLD)
            }
            GameEvent::GameOver(_) => continue,
        };
        spawn_toast(&mut commands, list, text, color, 6.0, 16.0);
    }
}

fn wave_banner(
    mut events: MessageReader<GameEvent>,
    time: Res<Time>,
    mut banner: Query<(&mut WaveBanner, &mut Text, &mut TextColor, &mut TextShadow, &mut Visibility)>,
) {
    let Ok((mut b, mut text, mut color, mut shadow, mut vis)) = banner.single_mut() else { return };
    for ev in events.read() {
        match ev {
            GameEvent::BossSpawned => {
                text.0 = "BOSS".to_string();
                color.0 = crate::render::palette::GLOW_ORANGE;
                b.0 = 2.5;
            }
            GameEvent::SwarmStarted { black } => {
                text.0 = if *black { "FANTÔMES NOIRS".to_string() } else { "ESSAIM FINAL".to_string() };
                color.0 = crate::render::palette::GLOW_MAGENTA;
                b.0 = 3.0;
            }
            _ => {}
        }
    }
    b.0 = (b.0 - time.delta_secs()).max(0.0);
    // The shadow has its own color: fade it too, or it stays behind alone.
    let alpha = b.0.min(1.0);
    color.0.set_alpha(alpha);
    shadow.color = Color::srgba(0.0, 0.0, 0.0, 0.75 * alpha);
    let want = if b.0 > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
    if *vis != want {
        *vis = want;
    }
}

/// Bosses are always marked on screen (even through walls).
fn enemy_markers(
    mut commands: Commands,
    info: Query<&MatchInfo>,
    mobs: Query<(Entity, &GlobalTransform, &Mob)>,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut markers: Query<(Entity, &EnemyMarker, &mut Node, &mut Visibility, &mut Text)>,
) {
    let show = info.single().is_ok();
    let Ok((camera, cam_t)) = camera.single() else { return };
    // Spawn a marker per boss when needed.
    if show {
        for (e, _, _) in mobs.iter().filter(|(_, _, m)| m.kind.is_boss()) {
            if !markers.iter().any(|(_, m, ..)| m.0 == e) {
                commands.spawn((
                    DespawnOnExit(AppState::InGame),
                    EnemyMarker(e),
                    Text::new("▼"),
                    bold(22.0),
                    TextColor(Color::srgb(1.0, 0.3, 0.3)),
                    TextShadow::default(),
                    Node { position_type: PositionType::Absolute, ..default() },
                    Visibility::Hidden,
                ));
            }
        }
    }
    for (marker_entity, marker, mut node, mut vis, mut text) in &mut markers {
        let Ok((_, t, mob)) = mobs.get(marker.0) else {
            commands.entity(marker_entity).try_despawn();
            continue;
        };
        if !show {
            *vis = Visibility::Hidden;
            continue;
        }
        let world = t.translation() + Vec3::Y * (mob.kind.def().height + 0.8);
        let dist = world.distance(cam_t.translation());
        match camera.world_to_viewport(cam_t, world) {
            Ok(p) => {
                node.left = Val::Px(p.x - 12.0);
                node.top = Val::Px(p.y - 30.0);
                *vis = Visibility::Inherited;
                set_text(&mut text, format!("▼ {dist:.0} m"));
            }
            Err(_) => *vis = Visibility::Hidden,
        }
    }
}

/// Toasts slide in from the right with a flash of their color, then slide
/// out while fading.
#[allow(clippy::type_complexity)]
fn age_toasts(
    mut commands: Commands,
    time: Res<Time>,
    mut toasts: Query<(Entity, &mut Toast, &mut BackgroundColor, &mut UiTransform, &Children)>,
    mut accents: Query<&mut BackgroundColor, (With<ToastAccent>, Without<Toast>)>,
    mut texts: Query<(&mut TextColor, &mut TextShadow), With<ToastText>>,
) {
    for (e, mut t, mut bg, mut tr, children) in &mut toasts {
        t.age += time.delta_secs();
        let left = t.life - t.age;
        if left <= 0.0 {
            commands.entity(e).try_despawn();
            continue;
        }
        let k = (t.age / 0.28).min(1.0);
        let ease_in = 1.0 - (1.0 - k).powi(3);
        let out = (left / 0.45).min(1.0);
        let alpha = k.min(out);
        tr.translation = Val2::px((1.0 - ease_in) * 80.0 + (1.0 - out) * 40.0, 0.0);
        let flash = (1.0 - t.age / 0.6).max(0.0);
        let base = Color::srgba(0.05, 0.03, 0.08, 0.78);
        bg.0 = lerp_color(base, t.color.with_alpha(0.78), flash * 0.45).with_alpha(0.78 * alpha);
        for c in children.iter() {
            if let Ok(mut a) = accents.get_mut(c) {
                a.0 = t.color.with_alpha(alpha);
            }
            if let Ok((mut text, mut shadow)) = texts.get_mut(c) {
                // Colored text, nudged toward white so it reads on the dark plate.
                text.0 = lerp_color(t.color, TEXT, 0.25).with_alpha(alpha);
                shadow.color = Color::srgba(0.0, 0.0, 0.0, 0.85 * alpha);
            }
        }
    }
}

/// Name tags above teammates, price tags above chests.
#[allow(clippy::type_complexity)]
fn spawn_world_labels(
    mut commands: Commands,
    local: Res<LocalPlayer>,
    players: Query<(Entity, &Player), Added<PlayerState>>,
    chests: Query<(Entity, &Chest), Added<Chest>>,
    info: Query<&MatchInfo>,
) {
    for (e, p) in &players {
        if Some(p.slot) == local.slot {
            continue;
        }
        commands.spawn((
            DespawnOnExit(AppState::InGame),
            WorldLabel { target: e, height: 2.3, max_distance: 200.0 },
            Text::new(p.name.clone()),
            bold(18.0),
            TextColor(player_color(p.slot)),
            TextShadow::default(),
            Node { position_type: PositionType::Absolute, ..default() },
            Visibility::Hidden,
        ));
    }
    let _ = &info;
    for (e, c) in &chests {
        let color = chest_color(c.kind);
        commands.spawn((
            DespawnOnExit(AppState::InGame),
            WorldLabel { target: e, height: 1.4, max_distance: 25.0 },
            Text::new(c.kind.name()),
            bold(16.0),
            TextColor(color),
            TextShadow::default(),
            Node { position_type: PositionType::Absolute, ..default() },
            Visibility::Hidden,
        ));
    }
}

#[allow(clippy::type_complexity)]
fn place_world_labels(
    mut commands: Commands,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    targets: Query<(&GlobalTransform, Option<&Chest>, Option<&ChestOpen>)>,
    info: Query<&MatchInfo>,
    local: Res<LocalPlayer>,
    me: Query<(&Player, &Wallet, &Items)>,
    mut labels: Query<(Entity, &WorldLabel, &mut Node, &mut Visibility, &mut Text, &ComputedNode)>,
) {
    let Ok((camera, cam_t)) = camera.single() else { return };
    let elapsed = info.single().map(|m| m.elapsed).unwrap_or(0.0);
    // Prices are personal: they grow with every chest *you* opened.
    let (opened, stats) = me
        .iter()
        .find(|(p, ..)| Some(p.slot) == local.slot)
        .map(|(_, w, i)| (w.chests_opened, PlayerStats::from_inventory(&i.0)))
        .unwrap_or_else(|| (0, PlayerStats::from_inventory(&Default::default())));
    for (e, label, mut node, mut vis, mut text, computed) in &mut labels {
        let Ok((target, chest, open)) = targets.get(label.target) else {
            commands.entity(e).try_despawn();
            continue;
        };
        if open.is_some_and(|o| o.opened) {
            *vis = Visibility::Hidden;
            continue;
        }
        let world = target.translation() + Vec3::Y * label.height;
        let dist = world.distance(cam_t.translation());
        if dist > label.max_distance {
            *vis = Visibility::Hidden;
            continue;
        }
        if let Some(c) = chest {
            let s = format!("{} or", crate::game::chests::player_price(c.kind, opened, elapsed, &stats));
            if text.0 != s {
                text.0 = s;
            }
        }
        match camera.world_to_viewport(cam_t, world) {
            Ok(p) => {
                let size = computed.size() * computed.inverse_scale_factor();
                node.left = Val::Px(p.x - size.x * 0.5);
                node.top = Val::Px(p.y - size.y * 0.5);
                *vis = Visibility::Inherited;
            }
            Err(_) => *vis = Visibility::Hidden,
        }
    }
}

fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    mut paused: ResMut<Paused>,
    mut menu: Query<&mut Visibility, With<PauseMenu>>,
    config: Res<crate::config::AppConfig>,
) {
    // (Screenshot runs ignore the keyboard: the window may get stray keys.)
    if keys.just_pressed(KeyCode::Escape) && config.shots.is_none() {
        paused.0 = !paused.0;
    }
    if let Ok(mut vis) = menu.single_mut() {
        let want = if paused.0 { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
}

fn pause_actions(
    mut actions: MessageReader<UiAction>,
    mut paused: ResMut<Paused>,
    mut config: ResMut<AppConfig>,
    mut net: MessageWriter<NetCommand>,
    mut values: Query<&mut Text, With<PauseValues>>,
) {
    for action in actions.read() {
        match action {
            UiAction::Resume => paused.0 = false,
            UiAction::SensDown => config.sensitivity = (config.sensitivity - 0.1).max(0.1),
            UiAction::SensUp => config.sensitivity = (config.sensitivity + 0.1).min(5.0),
            UiAction::VolDown => config.volume = (config.volume - 0.1).max(0.0),
            UiAction::VolUp => config.volume = (config.volume + 0.1).min(1.5),
            UiAction::FovDown => config.fov = (config.fov - 5.0).max(70.0),
            UiAction::FovUp => config.fov = (config.fov + 5.0).min(120.0),
            UiAction::Leave => {
                net.write(NetCommand::Leave);
            }
            _ => continue,
        };
        config.save();
    }
    if let Ok(mut t) = values.single_mut() {
        let s = format!(
            "Sensibilité {:.1}   ·   Volume {:.0}%   ·   FOV {:.0}°",
            config.sensitivity,
            config.volume * 100.0,
            config.fov
        );
        if t.0 != s {
            t.0 = s;
        }
    }
}
