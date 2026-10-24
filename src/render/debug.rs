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
    mut players: Query<(Entity, &crate::net::protocol::Player, &crate::net::protocol::PlayerState, &mut crate::net::protocol::Items)>,
) {
    use crate::net::protocol::{PendingChoice, WeaponPickup};
    use crate::shared::economy::ChestKind;
    use crate::shared::items::ItemKind;
    if *done || time.elapsed_secs() < 5.0 || !role.is_authority() {
        return;
    }
    let Some((entity, _, state, mut items)) = players.iter_mut().find(|(_, p, ..)| p.slot == 0) else { return };
    *done = true;
    for item in [ItemKind::FrogLegs, ItemKind::FrogLegs, ItemKind::Jetpack, ItemKind::VampireFangs, ItemKind::Crown, ItemKind::TeslaCoil] {
        items.0.add(item);
    }
    let forward = Vec3::new(-state.yaw.sin(), 0.0, -state.yaw.cos());
    let right = Vec3::new(forward.z, 0.0, -forward.x);
    // Gallery of the new V2 monsters (one elite) + a statue + a shockwave.
    use crate::shared::mobs::MobKind;
    let gallery = [MobKind::Charger, MobKind::Jelly, MobKind::Shaman, MobKind::HiveQueen, MobKind::StormTitan];
    for (i, kind) in gallery.iter().enumerate() {
        let p = state.sim.mv.pos + forward * 22.0 + right * ((i as f32 - 2.0) * 7.0) + Vec3::Y * if kind.def().flying { 3.0 } else { 0.3 };
        crate::game::mobs::spawn_mob(&mut commands, *kind, p, 1, 0, 0.0, i == 0);
    }
    commands.spawn((
        bevy_replicon::prelude::Replicated,
        crate::net::protocol::Statue { spot_id: 42, pos: state.sim.mv.pos + forward * 8.0 - right * 6.0, boss: MobKind::HiveQueen },
        crate::net::protocol::StatueUsed(false),
    ));
    commands.spawn((
        bevy_replicon::prelude::Replicated,
        WeaponPickup { kind: crate::shared::weapons::WeaponKind::Railgun, pos: state.sim.mv.pos + forward * 3.0 + Vec3::Y * 0.9 },
    ));
    commands.entity(entity).insert(DemoCards(8.0));
    let _ = PendingChoice {
        chest: ChestKind::Large,
        options: vec![],
    };
}

/// Opens the demo card choice a few seconds after the gallery.
#[derive(Component)]
struct DemoCards(f32);

fn demo_cards(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut DemoCards)>) {
    use crate::net::protocol::PendingChoice;
    use crate::shared::economy::ChestKind;
    use crate::shared::items::ItemKind;
    for (e, mut d) in &mut q {
        d.0 -= time.delta_secs();
        if d.0 <= 0.0 {
            commands.entity(e).remove::<DemoCards>().insert(PendingChoice {
                chest: ChestKind::Large,
                options: vec![ItemKind::DragonHeart, ItemKind::FrogLegs, ItemKind::EnergyShield, ItemKind::GatlingSoul],
            });
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
    if let Some(&at) = SHOT_TIMES.get(*next) {
        if t >= at {
            let _ = std::fs::create_dir_all(dir);
            let path = format!("{dir}/shot_{:02}.png", *next);
            info!("screenshot -> {path}");
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
            *next += 1;
        }
    } else if t > SHOT_TIMES[SHOT_TIMES.len() - 1] + 2.0 {
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
