//! Debug helpers to check the game without playing: automatic screenshots.
//!
//! `prism_rush solo --autopilot --shots shots` plays a run with a bot driving
//! the local player and saves screenshots to `shots/`, then quits.

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

use crate::config::AppConfig;

/// Seconds (since launch) at which screenshots are taken.
const SHOT_TIMES: [f32; 6] = [4.0, 9.0, 15.0, 24.0, 36.0, 50.0];

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, auto_screenshots.run_if(|c: Res<AppConfig>| c.shots.is_some()))
            .add_systems(Update, (demo_scene, demo_cards).run_if(|c: Res<AppConfig>| c.shots.is_some()))
            .add_systems(Update, diag_mobs.run_if(|c: Res<AppConfig>| c.autopilot));
    }
}

/// For screenshots: gives the host player a few items, drops a weapon in
/// front of them and opens a card choice, so every UI element is visible.
fn demo_scene(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut done: Local<bool>,
    role: Res<crate::net::NetRole>,
    input: Res<crate::net::client::InputState>,
    mut players: Query<(
        Entity,
        &crate::net::protocol::Player,
        &crate::net::protocol::PlayerState,
        &mut crate::net::protocol::Items,
        &mut crate::net::protocol::Gear,
    )>,
) {
    use crate::shared::build::{AutoWeapon, WeaponLevel};
    use crate::shared::items::ItemKind;
    if *done || time.elapsed_secs() < 5.0 || !role.is_authority() {
        return;
    }
    let Some((entity, _, state, mut items, mut gear)) = players.iter_mut().find(|(_, p, ..)| p.slot == 0) else { return };
    *done = true;
    // Show the automatic weapons.
    let gallery_mode = std::env::var("PRISM_GALLERY").is_ok();
    for a in [AutoWeapon::Aura, AutoWeapon::Orbs, AutoWeapon::Lightning].into_iter().filter(|_| !gallery_mode) {
        if !gear.0.has_auto(a) {
            gear.0.autos.push((a, WeaponLevel { level: 1, ..Default::default() }));
        }
    }
    for item in [ItemKind::FrogLegs, ItemKind::FrogLegs, ItemKind::Jetpack, ItemKind::VampireFangs, ItemKind::Crown, ItemKind::TeslaCoil] {
        items.0.add(item);
    }
    // In front of the camera (the local view direction).
    let forward = Vec3::new(-input.yaw.sin(), 0.0, -input.yaw.cos());
    let right = Vec3::new(forward.z, 0.0, -forward.x);
    // Gallery of the new V2 monsters (one elite) + a statue + a shockwave.
    use crate::shared::mobs::MobKind;
    // PRISM_GALLERY=1..4 shows the V4 monsters instead.
    use MobKind::*;
    let gallery: Vec<MobKind> = match std::env::var("PRISM_GALLERY").as_deref() {
        Ok("1") => vec![Flea, Scarab, ToxicJelly, MagmaJelly, MegaJelly, Mortar],
        Ok("2") => vec![Eye, Thornback, SkyJelly, Sporeling, FireBat, StoneGolem],
        Ok("3") => vec![CrystalGolem, ShieldKnight, Summoner, LavaBoar, Burrower],
        Ok("4") => vec![Broodmother, AncientGolem],
        Ok("5") => vec![Sandworm, Colossus],
        Ok("6") => vec![Broodmother],
        _ => vec![Charger, Jelly, Shaman, HiveQueen, StormTitan],
    };
    let (spacing, dist) = match (gallery.len(), gallery_mode) {
        (..=2, true) => (16.0, 22.0),
        (_, true) => (3.6, 10.0),
        _ => (7.0, 22.0),
    };
    let half = (gallery.len() as f32 - 1.0) * 0.5;
    for (i, kind) in gallery.iter().enumerate() {
        let p = state.sim.mv.pos + forward * dist + right * ((i as f32 - half) * spacing) + Vec3::Y * if kind.def().flying { 3.0 } else { 0.3 };
        let e = crate::game::mobs::spawn_mob(&mut commands, *kind, p, 1, 0, 0.0, i == 0 && !gallery_mode);
        if gallery_mode {
            commands.entity(e).insert(crate::game::mobs::Frozen);
            if *kind == Broodmother {
                // Two legs already broken, to check they disappear.
                commands.entity(e).insert(crate::net::protocol::MobParts { broken: 0b0101 });
            }
        }
    }
    if gallery_mode {
        return;
    }
    commands.spawn((
        bevy_replicon::prelude::Replicated,
        crate::net::protocol::Statue { spot_id: 42, pos: state.sim.mv.pos + forward * 8.0 - right * 6.0, boss: MobKind::HiveQueen },
        crate::net::protocol::StatueUsed(false),
    ));
    commands.entity(entity).insert(DemoCards(8.0));
}

/// Opens the demo card choice a few seconds after the gallery.
#[derive(Component)]
struct DemoCards(f32);

fn demo_cards(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut DemoCards, &crate::net::protocol::Gear, &crate::net::protocol::Items)>,
) {
    use crate::net::protocol::{CardOption, PendingChoice};
    use crate::shared::build::{roll_offers, AutoWeapon};
    for (e, mut d, gear, items) in &mut q {
        d.0 -= time.delta_secs();
        if d.0 <= 0.0 {
            let mut rng = crate::shared::rng::Rng::new(7);
            let options = roll_offers(&gear.0, &items.0, &AutoWeapon::ALL, 4.0, 3, &mut rng).into_iter().map(CardOption::Offer).collect();
            commands.entity(e).remove::<DemoCards>().insert(PendingChoice { chest: None, options });
        }
    }
}

fn auto_screenshots(
    mut commands: Commands,
    config: Res<AppConfig>,
    time: Res<Time<Real>>,
    mut next: Local<usize>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(dir) = &config.shots else { return };
    let t = time.elapsed_secs();
    // The monster gallery only needs the first shots.
    let count = if std::env::var("PRISM_GALLERY").is_ok() && !config.autopilot { 2 } else { SHOT_TIMES.len() };
    if let Some(&at) = SHOT_TIMES[..count].get(*next) {
        if t >= at {
            let _ = std::fs::create_dir_all(dir);
            let path = format!("{dir}/shot_{:02}.png", *next);
            info!("screenshot -> {path}");
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
            *next += 1;
        }
    } else if t > SHOT_TIMES[count - 1] + 2.0 {
        exit.write(AppExit::Success);
    }
}

/// Temporary diagnostics: visual vs server positions of monsters, orphan parts.
pub fn diag_mobs(
    mobs: Query<(Entity, &crate::net::protocol::MobState, &Transform)>,
    parts: Query<&super::model::AnimPart>,
    all: Query<Entity>,
    time: Res<Time<Real>>,
    mut last: Local<f32>,
    meshes: Res<Assets<Mesh>>,
    materials: Res<Assets<StandardMaterial>>,
    images: Res<Assets<Image>>,
    texts: Query<(), With<Text>>,
    nodes: Query<(), With<Node>>,
    lights: Query<(), With<PointLight>>,
) {
    if time.elapsed_secs() - *last < 3.0 {
        return;
    }
    *last = time.elapsed_secs();
    let max = mobs.iter().map(|(_, s, t)| s.pos.distance(t.translation)).fold(0.0f32, f32::max);
    let orphans = parts.iter().filter(|p| all.get(p.root).is_err()).count();
    info!(
        "[diag] entities {} meshes {} materials {} images {} texts {} nodes {} lights {} | mobs {} offset {:.2} orphans {}",
        all.iter().count(),
        meshes.len(),
        materials.len(),
        images.len(),
        texts.iter().count(),
        nodes.iter().count(),
        lights.iter().count(),
        mobs.iter().count(),
        max,
        orphans
    );
}
