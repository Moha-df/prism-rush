//! Player movement: a *pure* simulation step.
//!
//! `simulate_movement(state, input, stats, terrain, dt)` is the single source
//! of truth for how a player moves. It runs:
//! - on the **server** (authoritative),
//! - on the **client** for its own player (prediction), and again when a
//!   correction arrives from the server (re-simulation of unacknowledged inputs).
//!
//! Because both sides run the very same code with the same inputs and the same
//! world, the prediction is almost always right and corrections are tiny.

use bevy::math::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

use super::worldgen::Terrain;

/// Fixed simulation rate (Hz) shared by server and clients.
pub const TICK_HZ: f64 = 60.0;
pub const DT: f32 = 1.0 / TICK_HZ as f32;

pub const GRAVITY: f32 = 30.0;
/// Extra gravity when falling: snappier, less floaty jumps.
pub const FALL_GRAVITY_MULT: f32 = 1.35;
pub const PLAYER_RADIUS: f32 = 0.45;
pub const PLAYER_HEIGHT: f32 = 1.8;
pub const EYE_HEIGHT: f32 = 1.6;
pub const SLIDE_EYE_HEIGHT: f32 = 0.9;
/// Obstacles lower than this are climbed automatically (stairs, small rocks, ledges).
pub const STEP_HEIGHT: f32 = 0.65;

pub const WALK_SPEED: f32 = 7.5;
pub const SPRINT_SPEED: f32 = 11.0;
pub const CRAWL_SPEED: f32 = 1.4;
pub const GROUND_ACCEL: f32 = 70.0;
pub const AIR_ACCEL: f32 = 22.0;
pub const JUMP_SPEED: f32 = 10.5;
pub const COYOTE_TIME: f32 = 0.1;
pub const DASH_SPEED: f32 = 28.0;
pub const DASH_TIME: f32 = 0.16;
pub const DASH_RECHARGE: f32 = 2.2;
pub const SLIDE_TIME: f32 = 0.85;
pub const SLIDE_BOOST: f32 = 5.0;
pub const SLIDE_FRICTION: f32 = 4.5;
/// Below this horizontal speed a slide ends early.
pub const SLIDE_MIN_SPEED: f32 = 5.0;
pub const KILL_PLANE: f32 = -200.0;

/// Button bit flags packed in a `u16` (compact on the network).
pub mod buttons {
    pub const JUMP: u16 = 1 << 0;
    pub const SPRINT: u16 = 1 << 1;
    pub const SLIDE: u16 = 1 << 2;
    pub const DASH: u16 = 1 << 3;
    pub const FIRE: u16 = 1 << 4;
    pub const RELOAD: u16 = 1 << 5;
    pub const INTERACT: u16 = 1 << 6;
    pub const NEXT_WEAPON: u16 = 1 << 7;
    pub const PREV_WEAPON: u16 = 1 << 8;
}

/// What the player wants to do during one simulation tick.
/// This is the *only* thing clients send for gameplay: the server decides the rest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlayerInput {
    /// Sequence number, increases by one every tick. Used for acknowledgment.
    pub seq: u32,
    /// x = strafe right, y = forward. Length <= 1.
    pub move_axis: Vec2,
    /// Look direction, radians.
    pub yaw: f32,
    pub pitch: f32,
    /// Bit set of `buttons::*`.
    pub buttons: u16,
    /// Weapon slot to select (0-based), or 255 for "no change".
    pub select_slot: u8,
}

impl PlayerInput {
    pub fn held(&self, b: u16) -> bool {
        self.buttons & b != 0
    }

    /// Forward vector (horizontal) from yaw. Yaw 0 looks toward -Z (Bevy convention).
    pub fn forward_flat(&self) -> Vec3 {
        Vec3::new(-self.yaw.sin(), 0.0, -self.yaw.cos())
    }

    pub fn right_flat(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin())
    }

    /// Full 3D aim direction.
    pub fn aim_dir(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(-sy * cp, sp, -cy * cp)
    }
}

/// Movement-related stats coming from items.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoveStats {
    pub speed_mult: f32,
    pub air_jumps: u8,
    pub dash_charges: u8,
    pub jump_mult: f32,
}

impl Default for MoveStats {
    fn default() -> Self {
        Self { speed_mult: 1.0, air_jumps: 1, dash_charges: 1, jump_mult: 1.0 }
    }
}

/// Complete movement state of a player. Replicated to clients for reconciliation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MoveState {
    /// Feet position.
    pub pos: Vec3,
    pub vel: Vec3,
    pub grounded: bool,
    pub air_jumps_used: u8,
    /// Time since we left the ground (for coyote time).
    pub air_time: f32,
    /// Fractional dash charges available (recharges over time).
    pub dash_energy: f32,
    pub dash_timer: f32,
    pub dash_dir: Vec3,
    pub slide_timer: f32,
    /// Buttons of the previous tick, to detect "just pressed" edges.
    pub prev_buttons: u16,
}

impl MoveState {
    pub fn spawn_at(pos: Vec3) -> Self {
        Self { pos, dash_energy: 1.0, ..Default::default() }
    }

    pub fn is_sliding(&self) -> bool {
        self.slide_timer > 0.0
    }

    pub fn eye_height(&self) -> f32 {
        if self.is_sliding() { SLIDE_EYE_HEIGHT } else { EYE_HEIGHT }
    }
}

/// Things that happened during a tick (for sounds / effects).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MoveEvents {
    pub jumped: bool,
    pub air_jumped: bool,
    pub landed: bool,
    pub dashed: bool,
    pub slide_started: bool,
    pub pad_launched: bool,
}

/// Accelerates `vel` (horizontal part) toward `target` by at most `accel * dt`.
fn accelerate(vel: Vec3, target: Vec2, accel: f32, dt: f32) -> Vec3 {
    let cur = Vec2::new(vel.x, vel.z);
    let diff = target - cur;
    let max = accel * dt;
    let delta = if diff.length() > max { diff.normalize() * max } else { diff };
    let new = cur + delta;
    Vec3::new(new.x, vel.y, new.y)
}

/// One movement tick. `downed` players can only crawl.
pub fn simulate_movement(
    s: &mut MoveState,
    input: &PlayerInput,
    stats: &MoveStats,
    terrain: &Terrain,
    downed: bool,
    dt: f32,
) -> MoveEvents {
    let mut ev = MoveEvents::default();
    let pressed = |b: u16| input.held(b) && s.prev_buttons & b == 0;

    // --- Wish direction from input.
    let axis = input.move_axis.clamp_length_max(1.0);
    let wish = input.forward_flat() * axis.y + input.right_flat() * axis.x;
    let wish2 = Vec2::new(wish.x, wish.z);

    // --- Dash recharge.
    let max_dash = f32::from(stats.dash_charges);
    s.dash_energy = (s.dash_energy + dt / DASH_RECHARGE).min(max_dash);

    if downed {
        s.slide_timer = 0.0;
        s.dash_timer = 0.0;
        let target = wish2 * CRAWL_SPEED;
        s.vel = accelerate(s.vel, target, GROUND_ACCEL, dt);
    } else if s.dash_timer > 0.0 {
        // --- Dashing: fixed velocity, no gravity. Short and punchy.
        s.dash_timer -= dt;
        s.vel = s.dash_dir * DASH_SPEED;
        if s.dash_timer <= 0.0 {
            // Keep some momentum after the dash.
            s.vel = s.dash_dir * (SPRINT_SPEED * stats.speed_mult);
        }
    } else {
        let sprinting = input.held(buttons::SPRINT) && axis.y > 0.1;
        let speed = if sprinting { SPRINT_SPEED } else { WALK_SPEED } * stats.speed_mult;

        // --- Start a dash.
        if pressed(buttons::DASH) && s.dash_energy >= 1.0 {
            s.dash_energy -= 1.0;
            s.dash_timer = DASH_TIME;
            s.slide_timer = 0.0;
            s.dash_dir = if wish.length_squared() > 0.01 { wish.normalize() } else { input.forward_flat() };
            s.vel = s.dash_dir * DASH_SPEED;
            ev.dashed = true;
        }
        // --- Start a slide (on the ground, with some speed).
        else if pressed(buttons::SLIDE) && s.grounded && !s.is_sliding() {
            let hv = Vec2::new(s.vel.x, s.vel.z);
            let dir = if hv.length() > 1.0 { hv.normalize() } else { Vec2::new(wish.x, wish.z).normalize_or_zero() };
            if dir != Vec2::ZERO {
                let boosted = (hv.length() + SLIDE_BOOST).max(SPRINT_SPEED * stats.speed_mult + 2.0);
                s.vel.x = dir.x * boosted;
                s.vel.z = dir.y * boosted;
                s.slide_timer = SLIDE_TIME;
                ev.slide_started = true;
            }
        }

        if s.is_sliding() {
            s.slide_timer -= dt;
            // Low friction, slight steering.
            let hv = Vec2::new(s.vel.x, s.vel.z);
            let sp = (hv.length() - SLIDE_FRICTION * dt).max(0.0);
            let steer = (hv.normalize_or_zero() + wish2 * 0.03).normalize_or_zero();
            s.vel.x = steer.x * sp;
            s.vel.z = steer.y * sp;
            // Sliding downhill accelerates you: follow the slope.
            let g = terrain.wgen.ground_height(s.pos.x, s.pos.z);
            let gx = terrain.wgen.ground_height(s.pos.x + 0.5, s.pos.z) - g;
            let gz = terrain.wgen.ground_height(s.pos.x, s.pos.z + 0.5) - g;
            s.vel.x -= gx * 2.0 * GRAVITY * dt;
            s.vel.z -= gz * 2.0 * GRAVITY * dt;
            if sp < SLIDE_MIN_SPEED || !input.held(buttons::SLIDE) || !s.grounded {
                s.slide_timer = 0.0;
            }
        } else if s.grounded {
            s.vel = accelerate(s.vel, wish2 * speed, GROUND_ACCEL, dt);
        } else {
            // Air control: accelerate toward the wish direction but never
            // *brake* below the current speed (keeps dash/pad/slide momentum).
            let cur = Vec2::new(s.vel.x, s.vel.z);
            let target = wish2 * speed.max(cur.length());
            let accel = if wish2 == Vec2::ZERO { 2.0 } else { AIR_ACCEL };
            s.vel = accelerate(s.vel, target, accel, dt);
        }

        // --- Jumps (ground jump with coyote time, then air jumps).
        if pressed(buttons::JUMP) {
            let jump_v = JUMP_SPEED * stats.jump_mult;
            if s.grounded || s.air_time < COYOTE_TIME {
                s.vel.y = jump_v;
                s.grounded = false;
                s.air_time = COYOTE_TIME; // consume coyote time
                ev.jumped = true;
                // Slide-jump keeps the slide speed: a core "tech" of the game.
                s.slide_timer = 0.0;
            } else if s.air_jumps_used < stats.air_jumps {
                s.air_jumps_used += 1;
                s.vel.y = jump_v * 0.92;
                // Air jumps let you redirect horizontally a bit.
                if wish2 != Vec2::ZERO {
                    let cur = Vec2::new(s.vel.x, s.vel.z);
                    let redirected = wish2 * cur.length().max(WALK_SPEED * stats.speed_mult);
                    let mixed = cur.lerp(redirected, 0.6);
                    s.vel.x = mixed.x;
                    s.vel.z = mixed.y;
                }
                ev.air_jumped = true;
            }
        }
    }

    // --- Gravity (not while dashing).
    if s.dash_timer <= 0.0 && !s.grounded {
        let g = if s.vel.y < 0.0 { GRAVITY * FALL_GRAVITY_MULT } else { GRAVITY };
        s.vel.y -= g * dt;
        s.vel.y = s.vel.y.max(-60.0);
    }

    // --- Integrate + collide.
    let was_grounded = s.grounded;
    move_body(&mut s.pos, &mut s.vel, &mut s.grounded, PLAYER_RADIUS, PLAYER_HEIGHT, terrain, dt);

    if s.grounded {
        if !was_grounded && s.vel.y <= 0.0 {
            ev.landed = true;
        }
        s.air_time = 0.0;
        s.air_jumps_used = 0;
    } else {
        s.air_time += dt;
    }

    // --- Jump pads.
    if s.grounded && !downed {
        let mut launch = None;
        terrain.cache.for_each_pad_near(s.pos, |pad| {
            let d = Vec2::new(s.pos.x - pad.pos.x, s.pos.z - pad.pos.z).length();
            if d < pad.radius && (s.pos.y - pad.pos.y).abs() < 0.6 {
                launch = Some(pad.strength);
            }
        });
        if let Some(v) = launch {
            s.vel.y = v;
            s.grounded = false;
            s.air_time = COYOTE_TIME;
            s.slide_timer = 0.0;
            ev.pad_launched = true;
        }
    }

    // Fell out of the world (should not happen, but never trust physics).
    if s.pos.y < KILL_PLANE {
        s.pos.y = terrain.wgen.ground_height(s.pos.x, s.pos.z) + 2.0;
        s.vel = Vec3::ZERO;
    }

    s.prev_buttons = input.buttons;
    ev
}

/// Moves a vertical cylinder body through the world: horizontal collision
/// against solids (with auto step-up), terrain slope limit, then vertical
/// collision (floor / ceiling). Shared by players and ground monsters.
pub fn move_body(
    pos: &mut Vec3,
    vel: &mut Vec3,
    grounded: &mut bool,
    radius: f32,
    height: f32,
    terrain: &Terrain,
    dt: f32,
) {
    // Horizontal move.
    let old = *pos;
    pos.x += vel.x * dt;
    pos.z += vel.z * dt;

    // Terrain too steep upward -> act as a wall (prevents climbing cliffs).
    let h_old = terrain.wgen.ground_height(old.x, old.z);
    let h_new = terrain.wgen.ground_height(pos.x, pos.z);
    let horiz = Vec2::new(pos.x - old.x, pos.z - old.z).length();
    if horiz > 1e-4 && h_new > old.y + STEP_HEIGHT * 0.5 && (h_new - h_old) / horiz > 1.8 {
        pos.x = old.x;
        pos.z = old.z;
        vel.x *= 0.2;
        vel.z *= 0.2;
    }

    // Push out of solids (2 iterations handle corners between two objects).
    for _ in 0..2 {
        let p = *pos;
        let mut correction = Vec2::ZERO;
        terrain.cache.for_each_solid_near(p, |s| {
            let (bottom, top) = s.y_range();
            // Not overlapping vertically, or low enough to step on.
            if p.y + height <= bottom || p.y >= top - STEP_HEIGHT {
                return;
            }
            if let Some(push) = s.push_out_xz(Vec2::new(p.x + correction.x, p.z + correction.y), radius) {
                correction += push;
            }
        });
        if correction == Vec2::ZERO {
            break;
        }
        pos.x += correction.x;
        pos.z += correction.y;
        // Remove the velocity component going into the obstacle.
        let n = correction.normalize();
        let v = Vec2::new(vel.x, vel.z);
        let into = v.dot(n);
        if into < 0.0 {
            let v = v - n * into;
            vel.x = v.x;
            vel.z = v.y;
        }
    }

    // Vertical move.
    pos.y += vel.y * dt;

    // Ceiling: bump our head under solids while going up.
    if vel.y > 0.0 {
        let p = *pos;
        let mut ceiling = f32::INFINITY;
        terrain.cache.for_each_solid_near(p, |s| {
            let (bottom, top) = s.y_range();
            if p.y < bottom && p.y + height > bottom && top > p.y + STEP_HEIGHT && s.covers_xz(Vec2::new(p.x, p.z), radius * 0.6) {
                ceiling = ceiling.min(bottom);
            }
        });
        if ceiling.is_finite() {
            pos.y = ceiling - height;
            vel.y = 0.0;
        }
    }

    // Floor.
    let floor = terrain.floor_below(*pos + Vec3::Y * 0.01, radius, STEP_HEIGHT);
    let snap = if *grounded { 0.45 } else { 0.0 }; // stick to the ground walking downhill
    if vel.y <= 0.0 && pos.y <= floor + snap {
        pos.y = floor;
        vel.y = 0.0;
        *grounded = true;
    } else if pos.y < floor {
        // Moving up but below the floor (e.g. step-up): lift.
        pos.y = floor;
        *grounded = vel.y <= 0.0;
    } else {
        *grounded = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_world() -> Terrain {
        // Seed whose spawn area is flat (all are, by construction).
        let mut t = Terrain::new(7);
        t.cache.ensure_around(&t.wgen, Vec3::ZERO, 1);
        t
    }

    fn run(state: &mut MoveState, input: PlayerInput, terrain: &Terrain, ticks: usize) {
        for _ in 0..ticks {
            simulate_movement(state, &input, &MoveStats::default(), terrain, false, DT);
        }
    }

    fn spawn(t: &Terrain) -> MoveState {
        MoveState::spawn_at(Vec3::new(0.0, t.wgen.ground_height(0.0, 0.0), 0.0))
    }

    #[test]
    fn falls_and_lands() {
        let t = flat_world();
        let mut s = spawn(&t);
        s.pos.y += 5.0;
        run(&mut s, PlayerInput::default(), &t, 120);
        assert!(s.grounded);
        assert!((s.pos.y - t.wgen.ground_height(0.0, 0.0)).abs() < 0.05);
    }

    #[test]
    fn walks_forward_at_walk_speed() {
        let t = flat_world();
        let mut s = spawn(&t);
        run(&mut s, PlayerInput::default(), &t, 10); // settle
        let input = PlayerInput { move_axis: Vec2::Y, ..Default::default() };
        run(&mut s, input, &t, 60);
        let hv = Vec2::new(s.vel.x, s.vel.z).length();
        assert!((hv - WALK_SPEED).abs() < 0.1, "speed {hv}");
        // Yaw 0 looks toward -Z.
        assert!(s.pos.z < -3.0);
    }

    #[test]
    fn jump_reaches_expected_height() {
        let t = flat_world();
        let mut s = spawn(&t);
        run(&mut s, PlayerInput::default(), &t, 5);
        let ground = s.pos.y;
        let jump = PlayerInput { buttons: buttons::JUMP, ..Default::default() };
        let mut max_y = ground;
        for _ in 0..60 {
            simulate_movement(&mut s, &jump, &MoveStats::default(), &t, false, DT);
            max_y = max_y.max(s.pos.y);
        }
        let expected = JUMP_SPEED * JUMP_SPEED / (2.0 * GRAVITY);
        assert!((max_y - ground - expected).abs() < 0.25, "{}", max_y - ground);
    }

    #[test]
    fn air_jumps_are_limited_by_stats() {
        let t = flat_world();
        let mut s = spawn(&t);
        let stats = MoveStats { air_jumps: 2, ..Default::default() };
        let jump = PlayerInput { buttons: buttons::JUMP, ..Default::default() };
        let none = PlayerInput::default();
        let mut jumps = 0;
        // Spam jump every other tick.
        for i in 0..40 {
            let ev = simulate_movement(&mut s, if i % 2 == 0 { &jump } else { &none }, &stats, &t, false, DT);
            if ev.jumped || ev.air_jumped {
                jumps += 1;
            }
        }
        assert_eq!(jumps, 3, "1 ground jump + 2 air jumps");
    }

    #[test]
    fn dash_moves_fast_and_uses_charge() {
        let t = flat_world();
        let mut s = spawn(&t);
        let dash = PlayerInput { buttons: buttons::DASH, ..Default::default() };
        let ev = simulate_movement(&mut s, &dash, &MoveStats::default(), &t, false, DT);
        assert!(ev.dashed);
        assert!(s.dash_energy < 1.0);
        let start = s.pos;
        run(&mut s, PlayerInput::default(), &t, 9);
        assert!(start.distance(s.pos) > 3.0);
        // No charge left: second dash does nothing.
        let ev = simulate_movement(&mut s, &dash, &MoveStats::default(), &t, false, DT);
        assert!(!ev.dashed);
    }

    #[test]
    fn simulation_is_deterministic() {
        let t = flat_world();
        let inputs: Vec<PlayerInput> = (0..200)
            .map(|i| PlayerInput {
                seq: i,
                move_axis: Vec2::new((i as f32 * 0.1).sin(), 1.0),
                yaw: i as f32 * 0.02,
                buttons: if i % 37 == 0 { buttons::JUMP } else if i % 53 == 0 { buttons::DASH } else { 0 },
                ..Default::default()
            })
            .collect();
        let mut a = spawn(&t);
        let mut b = spawn(&t);
        for i in &inputs {
            simulate_movement(&mut a, i, &MoveStats::default(), &t, false, DT);
        }
        for i in &inputs {
            simulate_movement(&mut b, i, &MoveStats::default(), &t, false, DT);
        }
        assert_eq!(a, b);
    }
}
