//! Procedural animation: every tagged part is posed from its rest transform
//! and the `AnimState` of its model. No keyframes, just sines and state.

use std::f32::consts::TAU;

use bevy::prelude::*;

use super::avatars::AnimState;
use super::model::{AnimPart, AnimTag};
use crate::net::protocol::MobAnim;

pub struct AnimPlugin;

impl Plugin for AnimPlugin {
    fn build(&self, app: &mut App) {
        // Pose parts after the anim states were updated this frame.
        app.add_systems(Update, animate_parts.after(super::avatars::update_anim_states));
    }
}

/// Pose of one part for the given state. Pure function: easy to tweak.
pub fn pose(tag: AnimTag, rest: &Transform, st: &AnimState, t: f32) -> Transform {
    let mut out = *rest;
    let move_amount = (st.speed / 5.0).min(1.0);
    let ph = st.phase;
    let windup = st.action == MobAnim::Windup;
    let attack = st.action == MobAnim::Attack;
    let toppled = st.action == MobAnim::Toppled;
    // 0 -> 1 over the first 0.2 s of an action: snappy but not instant.
    let ease = (st.action_time / 0.2).min(1.0);

    match tag {
        AnimTag::LegL | AnimTag::LegR => {
            let sign = if tag == AnimTag::LegL { 1.0 } else { -1.0 };
            out.rotation = rest.rotation * Quat::from_rotation_x(ph.sin() * 0.75 * move_amount * sign);
        }
        AnimTag::ArmL | AnimTag::ArmR => {
            let sign = if tag == AnimTag::ArmL { -1.0 } else { 1.0 };
            let angle = if windup {
                // Raise both arms (big telegraph).
                -2.3 * ease
            } else if attack {
                // Slam down.
                0.7 - 0.7 * ease
            } else {
                ph.sin() * 0.55 * move_amount * sign + (t * 1.5).sin() * 0.04
            };
            out.rotation = rest.rotation * Quat::from_rotation_x(angle);
        }
        AnimTag::Leg(offset) => {
            let p = ph * 1.4 + offset * TAU;
            let lift = p.cos().max(0.0) * 0.35 * move_amount;
            out.rotation = rest.rotation * Quat::from_rotation_y(p.sin() * 0.4 * move_amount) * Quat::from_rotation_z(lift);
        }
        AnimTag::Breakable(i) => {
            if st.broken & (1 << i) != 0 {
                out.scale = Vec3::ZERO;
            } else {
                let p = ph * 1.4 + f32::from(i) * 0.25 * TAU;
                let lift = p.cos().max(0.0) * 0.3 * move_amount;
                out.rotation = rest.rotation * Quat::from_rotation_y(p.sin() * 0.3 * move_amount) * Quat::from_rotation_z(lift);
            }
        }
        AnimTag::Wing(offset) => {
            out.rotation = rest.rotation * Quat::from_rotation_z((t * 9.0 + offset * TAU).sin() * 0.45);
        }
        AnimTag::Body if toppled => {
            // Collapsed / kneeling: the body drops to the ground and shakes.
            let k = (st.action_time / 0.35).min(1.0);
            out.translation.y = rest.translation.y * (1.0 - 0.5 * k);
            out.rotation = rest.rotation * Quat::from_rotation_x(0.25 * k) * Quat::from_rotation_z((t * 7.0).sin() * 0.03);
        }
        AnimTag::Body => {
            let bounce = (ph.sin().abs() * 0.07 * move_amount) + (t * 2.0).sin() * 0.015;
            out.translation.y += bounce;
            let lean = if windup { -0.25 * ease } else if attack { 0.35 * (1.0 - ease) } else { 0.06 * move_amount };
            out.rotation = rest.rotation * Quat::from_rotation_x(lean);
            if st.downed {
                out.rotation *= Quat::from_rotation_z((t * 3.0).sin() * 0.05);
            }
        }
        AnimTag::Head => {
            out.rotation = rest.rotation * Quat::from_rotation_y((t * 0.8).sin() * 0.18) * Quat::from_rotation_x(if windup { -0.3 * ease } else { 0.0 });
        }
        AnimTag::Jaw => {
            let open = if windup { 0.6 * ease } else { (t * 6.0).sin().abs() * 0.15 };
            out.rotation = rest.rotation * Quat::from_rotation_x(-open);
        }
        AnimTag::Lid => {
            out.rotation = rest.rotation * Quat::from_rotation_x(-1.9 * st.open);
        }
        AnimTag::Spin(speed) => {
            out.rotation = Quat::from_rotation_y(t * speed) * rest.rotation;
        }
        AnimTag::Bob => {
            out.translation.y += (t * 2.4).sin() * 0.12;
        }
        AnimTag::Pulse => {
            let s = if windup {
                // Swelling + trembling before exploding / spitting.
                1.0 + 0.3 * ease + (t * 40.0).sin() * 0.04
            } else {
                1.0 + (t * 3.0).sin() * 0.03
            };
            out.scale = Vec3::splat(s);
        }
        AnimTag::Recoil => {
            out.translation.z += st.recoil * 0.07;
        }
        AnimTag::Beam => {
            let k = (1.0 - st.open * 1.5).max(0.0);
            let pulse = 1.0 + (t * 3.0).sin() * 0.15;
            out.scale = Vec3::new(k * pulse, k, k * pulse);
        }
    }
    out
}

fn animate_parts(time: Res<Time>, states: Query<&AnimState>, mut parts: Query<(&AnimPart, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (part, mut transform) in &mut parts {
        let Ok(st) = states.get(part.root) else { continue };
        *transform = pose(part.tag, &part.rest, st, t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legs_swing_only_when_moving() {
        let rest = Transform::IDENTITY;
        let mut st = AnimState { phase: 1.0, ..Default::default() };
        let still = pose(AnimTag::LegL, &rest, &st, 0.0);
        assert!(still.rotation.angle_between(Quat::IDENTITY) < 1e-4);
        st.speed = 6.0;
        let walking = pose(AnimTag::LegL, &rest, &st, 0.0);
        assert!(walking.rotation.angle_between(Quat::IDENTITY) > 0.1);
    }

    #[test]
    fn lid_opens() {
        let st = AnimState { open: 1.0, ..Default::default() };
        let p = pose(AnimTag::Lid, &Transform::IDENTITY, &st, 0.0);
        assert!(p.rotation.angle_between(Quat::IDENTITY) > 1.5);
    }
}
