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
use crate::render::MainCamera;
use crate::shared::economy;
use crate::shared::items::{ItemKind, PlayerStats};
use crate::shared::mobs::MobKind;
use crate::shared::weapons::magazine_size;

#[derive(Component)]
struct HpFill;
#[derive(Component)]
struct HpText;
#[derive(Component)]
struct GoldText;
#[derive(Component)]
struct RunText;
#[derive(Component)]
struct WeaponText;
#[derive(Component)]
struct AmmoText;
#[derive(Component)]
struct ReloadFill;
#[derive(Component)]
struct SlotsText;
#[derive(Component)]
struct DashText;
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
#[derive(Component)]
struct Toast(f32);
#[derive(Component)]
struct BossBar;
#[derive(Component)]
struct BossFill;
#[derive(Component)]
struct TeamText;
#[derive(Component)]
struct Hitmarker;
#[derive(Component)]
struct HurtVignette;
#[derive(Component)]
struct PauseMenu;
#[derive(Component)]
struct PauseValues;

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
                    age_toasts,
                    spawn_world_labels,
                    place_world_labels,
                    toggle_pause,
                    pause_actions,
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
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
        children![(
            marker,
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(fill_color),
        )],
    )
}

fn spawn_hud(mut commands: Commands) {
    let root = commands
        .spawn((
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

    // Crosshair: a dot + 4 ticks.
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
    for (x, y, w, h) in [(-1.0, -1.0, 2.0, 2.0), (-12.0, -1.0, 6.0, 2.0), (6.0, -1.0, 6.0, 2.0), (-1.0, -12.0, 2.0, 6.0), (-1.0, 6.0, 2.0, 6.0)] {
        commands.spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(x), top: Val::Px(y), width: Val::Px(w), height: Val::Px(h), ..default() },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.9)),
            ChildOf(center),
        ));
    }
    commands.spawn((
        Hitmarker,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(-11.0),
            top: Val::Px(-11.0),
            width: Val::Px(22.0),
            height: Val::Px(22.0),
            border: UiRect::all(Val::Px(3.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
        BorderColor::all(Color::NONE),
        ChildOf(center),
    ));

    // Bottom-left: health, gold, items.
    let bl = child(&mut commands, (abs_node(Some(24.0), None, None, Some(24.0)),));
    commands.spawn((label("", 18.0, TEXT), HpText, ChildOf(bl)));
    commands.spawn((bar(320.0, 18.0, Color::srgb(0.35, 0.85, 0.45), HpFill), ChildOf(bl)));
    commands.spawn((label("", 26.0, crate::render::palette::GOLD), GoldText, ChildOf(bl)));
    commands.spawn((
        ItemsRow,
        Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(4.0), row_gap: Val::Px(4.0), max_width: Val::Px(420.0), ..default() },
        ChildOf(bl),
    ));

    // Bottom-right: weapon.
    let br = child(&mut commands, (Node { align_items: AlignItems::FlexEnd, ..abs_node(None, Some(24.0), None, Some(24.0)) },));
    commands.spawn((label("", 16.0, TEXT_DIM), SlotsText, ChildOf(br)));
    commands.spawn((label("", 22.0, TEXT), WeaponText, ChildOf(br)));
    commands.spawn((Text::new(""), bold(44.0), TextColor(TEXT), TextShadow::default(), AmmoText, ChildOf(br)));
    commands.spawn((bar(200.0, 6.0, ACCENT, ReloadFill), ChildOf(br)));
    commands.spawn((label("", 16.0, TEXT_DIM), DashText, ChildOf(br)));

    // Top-left: run info.
    let tl = child(&mut commands, (abs_node(Some(24.0), None, Some(20.0), None),));
    commands.spawn((label("", 22.0, TEXT), RunText, ChildOf(tl)));

    // Top-right: team.
    let tr = child(&mut commands, (Node { align_items: AlignItems::FlexEnd, ..abs_node(None, Some(24.0), Some(20.0), None) },));
    commands.spawn((label("", 18.0, TEXT), TeamText, ChildOf(tr)));

    // Top-center: boss bar.
    let top = commands
        .spawn((
            BossBar,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(24.0),
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
    commands.spawn((Text::new("COLOSSE"), bold(24.0), TextColor(crate::render::palette::GLOW_ORANGE), TextShadow::default(), ChildOf(top)));
    commands.spawn((bar(520.0, 14.0, crate::render::palette::GLOW_MAGENTA, BossFill), ChildOf(top)));

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

    // Right: toasts.
    commands.spawn((
        ToastList,
        Node { align_items: AlignItems::FlexEnd, ..abs_node(None, Some(24.0), Some(220.0), None) },
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

type LocalQuery<'a> = (&'a Player, &'a Health, &'a Wallet, &'a Items);

fn local_player<'a>(local: &LocalPlayer, q: &'a Query<LocalQuery>) -> Option<(&'a Player, &'a Health, &'a Wallet, &'a Items)> {
    q.iter().find(|(p, ..)| Some(p.slot) == local.slot)
}

#[allow(clippy::too_many_arguments)]
fn update_vitals(
    local: Res<LocalPlayer>,
    players: Query<LocalQuery>,
    info: Query<&MatchInfo>,
    mut hp_text: Query<&mut Text, (With<HpText>, Without<GoldText>, Without<RunText>)>,
    mut hp_fill: Query<(&mut Node, &mut BackgroundColor), With<HpFill>>,
    mut gold: Query<&mut Text, (With<GoldText>, Without<HpText>, Without<RunText>)>,
    mut run: Query<&mut Text, (With<RunText>, Without<HpText>, Without<GoldText>)>,
) {
    if let Some((_, health, wallet, _)) = local_player(&local, &players) {
        if let Ok(mut t) = hp_text.single_mut() {
            t.0 = format!("PV {:.0} / {:.0}", health.hp.max(0.0), health.max);
        }
        if let Ok((mut node, mut color)) = hp_fill.single_mut() {
            let frac = (health.hp / health.max).clamp(0.0, 1.0);
            node.width = Val::Percent(frac * 100.0);
            color.0 = if frac < 0.3 { Color::srgb(0.95, 0.3, 0.25) } else { Color::srgb(0.35, 0.85, 0.45) };
        }
        if let Ok(mut t) = gold.single_mut() {
            t.0 = format!("{} or", wallet.gold);
        }
    }
    if let (Ok(m), Ok(mut t)) = (info.single(), run.single_mut()) {
        let secs = m.elapsed as u32;
        t.0 = format!(
            "{:02}:{:02}   Niveau {}   ·   Coffre {} or   ·   Arme {} or",
            secs / 60,
            secs % 60,
            m.level,
            economy::chest_price(economy::ChestKind::Item, m.difficulty),
            economy::chest_price(economy::ChestKind::Weapon, m.difficulty)
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn update_weapon(
    pred: Res<Prediction>,
    local: Res<LocalPlayer>,
    players: Query<LocalQuery>,
    mut weapon: Query<&mut Text, (With<WeaponText>, Without<AmmoText>, Without<SlotsText>, Without<DashText>)>,
    mut ammo: Query<&mut Text, (With<AmmoText>, Without<WeaponText>, Without<SlotsText>, Without<DashText>)>,
    mut slots: Query<&mut Text, (With<SlotsText>, Without<WeaponText>, Without<AmmoText>, Without<DashText>)>,
    mut dash: Query<&mut Text, (With<DashText>, Without<WeaponText>, Without<AmmoText>, Without<SlotsText>)>,
    mut reload: Query<&mut Node, With<ReloadFill>>,
) {
    if !pred.active {
        return;
    }
    let a = &pred.sim.arsenal;
    let kind = a.current_kind();
    let stats = local_player(&local, &players).map(|(_, _, _, i)| PlayerStats::from_inventory(&i.0)).unwrap_or_else(|| PlayerStats::from_inventory(&Default::default()));
    if let Ok(mut t) = weapon.single_mut() {
        t.0 = kind.def().name.to_string();
    }
    if let Ok(mut t) = ammo.single_mut() {
        t.0 = if a.is_reloading() {
            "Rechargement".into()
        } else {
            format!("{} / {}", a.weapons[a.current].ammo, magazine_size(kind, &stats))
        };
    }
    if let Ok(mut t) = slots.single_mut() {
        t.0 = a
            .weapons
            .iter()
            .enumerate()
            .map(|(i, w)| if i == a.current { format!("[{} {}]", i + 1, w.kind.def().name) } else { format!("{}", i + 1) })
            .collect::<Vec<_>>()
            .join("  ");
    }
    if let Ok(mut node) = reload.single_mut() {
        let frac = if a.is_reloading() { 1.0 - a.reload_left / kind.def().reload_time } else { 0.0 };
        node.width = Val::Percent(frac.clamp(0.0, 1.0) * 100.0);
    }
    if let Ok(mut t) = dash.single_mut() {
        let full = pred.sim.mv.dash_energy.floor() as usize;
        let max = stats.movement.dash_charges as usize;
        t.0 = format!("Dash {}{}   Sauts {}", "●".repeat(full), "○".repeat(max.saturating_sub(full)), stats.movement.air_jumps + 1);
    }
}

fn update_items(
    mut commands: Commands,
    local: Res<LocalPlayer>,
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
        let [r, g, b] = item.color();
        commands.spawn((
            Node {
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(r * 0.5, g * 0.5, b * 0.5, 0.85)),
            ChildOf(row),
            children![label(format!("{} x{}", item.name(), n), 14.0, TEXT)],
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn update_prompt(
    local: Res<LocalPlayer>,
    pred: Res<Prediction>,
    players: Query<LocalQuery>,
    states: Query<(&Player, &PlayerState, &Health)>,
    chests: Query<(&Chest, &ChestOpen)>,
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
        } else if let Some((chest, _)) = chests
            .iter()
            .filter(|(c, o)| !o.opened && c.pos.distance(pos + Vec3::Y * 0.5) < INTERACT_RANGE)
            .min_by(|a, b| a.0.pos.distance(pos).total_cmp(&b.0.pos.distance(pos)))
        {
            let difficulty = info.single().map(|m| m.difficulty).unwrap_or(1.0);
            let price = economy::chest_price(chest.kind, difficulty);
            let what = match chest.kind {
                economy::ChestKind::Item => "coffre d'objet",
                economy::ChestKind::Weapon => "coffre d'arme",
            };
            text = if wallet.gold >= price {
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
) {
    if let Ok(mut t) = team.single_mut() {
        let mut sorted: Vec<_> = players.iter().filter(|(p, _)| Some(p.slot) != local.slot).collect();
        sorted.sort_by_key(|(p, _)| p.slot);
        t.0 = sorted
            .iter()
            .map(|(p, h)| match h {
                Some(h) if h.downed => format!("{} — À TERRE", p.name),
                Some(h) => format!("{} — {:.0} PV", p.name, h.hp),
                None => p.name.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    let boss = bosses.iter().find(|(m, _)| m.kind == MobKind::Colossus);
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
    mut hitmarker: Query<&mut BorderColor, (With<Hitmarker>, Without<HurtVignette>)>,
    mut vignette: Query<&mut BorderColor, (With<HurtVignette>, Without<Hitmarker>)>,
) {
    if let Ok(mut b) = hitmarker.single_mut() {
        let a = (feedback.hitmarker / 0.18).clamp(0.0, 1.0);
        let c = if feedback.hitmarker_kill {
            Color::srgba(1.0, 0.3, 0.3, a)
        } else if feedback.hitmarker_crit {
            Color::srgba(1.0, 0.85, 0.2, a)
        } else {
            Color::srgba(1.0, 1.0, 1.0, a)
        };
        *b = BorderColor::all(c);
    }
    let low_hp = local_player(&local, &players).map(|(_, h, ..)| 1.0 - (h.hp / h.max).clamp(0.0, 1.0)).unwrap_or(0.0);
    if let Ok(mut b) = vignette.single_mut() {
        let a = (feedback.hurt_flash * 0.9).max(if low_hp > 0.7 { (low_hp - 0.7) * 1.2 } else { 0.0 });
        *b = BorderColor::all(Color::srgba(0.8, 0.0, 0.05, a.min(0.6)));
    }
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
            GameEvent::ChestOpened { slot, reward, .. } => {
                let what = match reward {
                    Reward::Item(i) => format!("{} ({})", i.name(), i.description()),
                    Reward::Weapon(w) => format!("arme : {}", w.def().name),
                };
                (format!("{} obtient {}", name(*slot), what), player_color(*slot))
            }
            GameEvent::Downed { slot } => (format!("{} est à terre !", name(*slot)), Color::srgb(1.0, 0.35, 0.3)),
            GameEvent::Revived { slot } => (format!("{} est de retour !", name(*slot)), Color::srgb(0.4, 1.0, 0.6)),
            GameEvent::BossSpawned => ("Un COLOSSE approche !".to_string(), crate::render::palette::GLOW_ORANGE),
            GameEvent::BossKilled => ("Le Colosse est tombé !".to_string(), crate::render::palette::GOLD),
            GameEvent::GameOver(_) => continue,
        };
        commands.spawn((
            Toast(6.0),
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.03, 0.08, 0.7)),
            ChildOf(list),
            children![label(text, 18.0, color)],
        ));
    }
}

fn age_toasts(mut commands: Commands, time: Res<Time>, mut toasts: Query<(Entity, &mut Toast, &mut BackgroundColor)>) {
    for (e, mut t, mut bg) in &mut toasts {
        t.0 -= time.delta_secs();
        if t.0 <= 0.0 {
            commands.entity(e).try_despawn();
        } else {
            bg.0.set_alpha((t.0).min(0.7));
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
    let difficulty = info.single().map(|m| m.difficulty).unwrap_or(1.0);
    for (e, c) in &chests {
        let color = match c.kind {
            economy::ChestKind::Item => crate::render::palette::GOLD,
            economy::ChestKind::Weapon => crate::render::palette::TEAL_GLOW,
        };
        commands.spawn((
            DespawnOnExit(AppState::InGame),
            WorldLabel { target: e, height: 1.4, max_distance: 25.0 },
            Text::new(format!("{}", economy::chest_price(c.kind, difficulty))),
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
    mut labels: Query<(Entity, &WorldLabel, &mut Node, &mut Visibility, &mut Text, &ComputedNode)>,
) {
    let Ok((camera, cam_t)) = camera.single() else { return };
    let difficulty = info.single().map(|m| m.difficulty).unwrap_or(1.0);
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
            let s = format!("{} or", economy::chest_price(c.kind, difficulty));
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

fn toggle_pause(keys: Res<ButtonInput<KeyCode>>, mut paused: ResMut<Paused>, mut menu: Query<&mut Visibility, With<PauseMenu>>) {
    if keys.just_pressed(KeyCode::Escape) {
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
