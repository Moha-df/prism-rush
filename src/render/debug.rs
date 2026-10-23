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
            .add_systems(Update, diag_mobs.run_if(|c: Res<AppConfig>| c.autopilot));
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
) {
    if time.elapsed_secs() - *last < 3.0 {
        return;
    }
    *last = time.elapsed_secs();
    let max = mobs.iter().map(|(_, s, t)| s.pos.distance(t.translation)).fold(0.0f32, f32::max);
    let orphans = parts.iter().filter(|p| all.get(p.root).is_err()).count();
    info!("[diag] mobs {} max visual offset {:.2} m, orphan anim parts {}", mobs.iter().count(), max, orphans);
}
