//! Bots: simple AI players used to test the game without humans.
//!
//! The same "brain" (`think`) drives:
//! - **server bots** (`--bots N`): they push inputs directly into their queue;
//! - the **headless bot client** (`prism_rush bot`): it fills `InputState`
//!   exactly like the keyboard would, so the full netcode path is exercised
//!   (prediction, sending inputs, reconciliation).

use bevy::prelude::*;

use crate::config::{AppConfig, LaunchMode};
use crate::game::players::{InputQueue, Owner};
use crate::game::{match_playing, ServerSet};
use crate::net::client::{InputState, Prediction};
use crate::net::protocol::*;
use crate::net::{LocalPlayer, NetRole};
use crate::shared::economy::{self, ChestKind};
use crate::shared::movement::{buttons, PlayerInput};
use crate::shared::rng::Rng;
use crate::shared::sim::PlayerSim;

#[derive(Component, Debug)]
pub struct BotBrain {
    rng: Rng,
    ticks: u32,
    wander_goal: Option<Vec3>,
    strafe: f32,
    /// Chest currently targeted, and ticks spent trying to reach it.
    chest_goal: Option<Vec3>,
    chest_ticks: u32,
    /// Chests we gave up on (unreachable): never try them again.
    ignored_chests: Vec<Vec3>,
}

impl Default for BotBrain {
    fn default() -> Self {
        Self {
            rng: Rng::new(std::process::id() as u64 ^ 0xB07),
            ticks: 0,
            wander_goal: None,
            strafe: 1.0,
            chest_goal: None,
            chest_ticks: 0,
            ignored_chests: Vec::new(),
        }
    }
}

/// What a bot knows about the world.
pub struct BotView<'a> {
    pub me: &'a PlayerSim,
    pub downed: bool,
    pub gold: u32,
    pub elapsed: f32,
    pub chests_opened: u32,
    /// (center position) of monsters.
    pub mobs: &'a [Vec3],
    /// Unopened chests.
    pub chests: &'a [(Vec3, ChestKind)],
    pub downed_allies: &'a [Vec3],
    /// Power-ups to take and charge shrines to stand in.
    pub goodies: &'a [Vec3],
}

pub struct BotOutput {
    pub move_axis: Vec2,
    pub yaw: f32,
    pub pitch: f32,
    pub buttons: u16,
}

/// Yaw/pitch looking from `from` to `to`.
fn look_at(from: Vec3, to: Vec3) -> (f32, f32) {
    let d = to - from;
    let flat = Vec2::new(d.x, d.z).length();
    (f32::atan2(-d.x, -d.z), f32::atan2(d.y, flat.max(0.01)))
}

/// Converts a world direction to the (strafe, forward) axis for a given yaw.
fn world_to_axis(dir: Vec3, yaw: f32) -> Vec2 {
    let forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    Vec2::new(dir.dot(right), dir.dot(forward)).clamp_length_max(1.0)
}

/// Chests higher than this above the bot are ignored (they need parkour).
const MAX_CHEST_CLIMB: f32 = 1.5;
/// A monster closer than this: fight instead of going to a chest.
const THREAT_DISTANCE: f32 = 12.0;
/// Seconds (in ticks) before giving up on a chest it can't reach.
const CHEST_GIVE_UP_TICKS: u32 = 8 * 60;

pub fn think(brain: &mut BotBrain, view: &BotView) -> BotOutput {
    brain.ticks += 1;
    let me = view.me.mv.pos;
    let eye = view.me.eye();
    let mut buttons_out = 0u16;
    let toggle = brain.ticks.is_multiple_of(2);

    if view.downed {
        return BotOutput { move_axis: Vec2::ZERO, yaw: 0.0, pitch: 0.0, buttons: 0 };
    }

    let nearest = |points: &mut dyn Iterator<Item = Vec3>| -> Option<Vec3> {
        points.min_by(|a, b| a.distance_squared(me).total_cmp(&b.distance_squared(me)))
    };

    // 1. Revive teammates.
    let ally = nearest(&mut view.downed_allies.iter().copied().filter(|p| p.distance(me) < 60.0));
    // 2. Buy chests when rich enough.
    //    The bot can't do parkour: skip chests perched high above it, and
    //    give up on a chest it couldn't reach in time.
    let reachable = |p: &Vec3| p.y - me.y < MAX_CHEST_CLIMB && !brain.ignored_chests.iter().any(|i| i.distance(*p) < 0.5);
    let chest = nearest(
        &mut view
            .chests
            .iter()
            .filter(|(p, k)| p.distance(me) < 50.0 && reachable(p) && view.gold >= economy::chest_price_v3(*k, view.chests_opened, view.elapsed))
            .map(|(p, _)| *p),
    );
    // 3. Fight.
    let enemy = nearest(&mut view.mobs.iter().copied().filter(|p| p.distance(me) < 45.0));
    // Monsters close by take priority over shopping (survive first).
    let threatened = enemy.is_some_and(|e| e.distance(me) < THREAT_DISTANCE);
    // Power-ups and shrines: worth a detour, even under pressure when close.
    let goodie = nearest(&mut view.goodies.iter().copied().filter(|p| p.distance(me) < 35.0))
        .filter(|g| !threatened || g.distance(me) < 10.0);
    let chest = chest.filter(|_| !threatened);

    // Give-up timer for the current chest.
    match chest {
        Some(c) if brain.chest_goal.is_some_and(|g| g.distance(c) < 0.5) => {
            brain.chest_ticks += 1;
            if brain.chest_ticks > CHEST_GIVE_UP_TICKS {
                brain.ignored_chests.push(c);
                brain.chest_goal = None;
                brain.chest_ticks = 0;
            }
        }
        Some(c) => {
            brain.chest_goal = Some(c);
            brain.chest_ticks = 0;
        }
        None => brain.chest_goal = None,
    }
    let chest = chest.filter(|c| !brain.ignored_chests.iter().any(|i| i.distance(*c) < 0.5));

    let (goal, look) = if let Some(a) = ally {
        if a.distance(me) < 2.0 {
            buttons_out |= buttons::INTERACT;
        }
        (Some(a), a + Vec3::Y * 0.5)
    } else if let Some(g) = goodie {
        // Keep shooting on the way (and while charging a shrine).
        match enemy {
            Some(e) => {
                buttons_out |= buttons::FIRE;
                (Some(g), e)
            }
            None => (Some(g), g + Vec3::Y),
        }
    } else if let Some(c) = chest {
        if c.distance(me) < 2.0 && toggle {
            buttons_out |= buttons::INTERACT;
        }
        (Some(c), c + Vec3::Y * 0.5)
    } else if let Some(e) = enemy {
        // Hold the trigger: the weapon fires at its own rate.
        buttons_out |= buttons::FIRE;
        // Circle-strafe at a comfortable distance.
        if brain.ticks.is_multiple_of(180) {
            brain.strafe = -brain.strafe;
        }
        let to = Vec3::new(e.x - me.x, 0.0, e.z - me.z);
        let side = Vec3::new(-to.z, 0.0, to.x).normalize_or_zero() * brain.strafe;
        let radial = if to.length() < 9.0 { -to.normalize_or_zero() } else { to.normalize_or_zero() * 0.3 };
        (Some(me + (side + radial) * 5.0), e)
    } else {
        let goal = match brain.wander_goal {
            Some(g) if g.distance(me) > 3.0 => g,
            _ => {
                let angle = brain.rng.range(0.0, std::f32::consts::TAU);
                let g = me + Vec3::new(angle.cos(), 0.0, angle.sin()) * 30.0;
                brain.wander_goal = Some(g);
                g
            }
        };
        buttons_out |= buttons::SPRINT;
        (Some(goal), goal + Vec3::Y * 1.5)
    };

    let (yaw, pitch) = look_at(eye, look);
    let mut axis = Vec2::ZERO;
    if let Some(g) = goal {
        let d = Vec3::new(g.x - me.x, 0.0, g.z - me.z);
        if d.length() > 1.2 {
            axis = world_to_axis(d.normalize(), yaw);
        }
    }

    // Movement tech, to exercise the controller: jumps, dashes, slides.
    let hv = Vec2::new(view.me.mv.vel.x, view.me.mv.vel.z).length();
    if (brain.rng.chance(0.012) || (axis.length() > 0.5 && hv < 1.0 && brain.ticks > 30)) && toggle {
        buttons_out |= buttons::JUMP;
    }
    if brain.rng.chance(0.004) {
        buttons_out |= buttons::DASH;
    }

    BotOutput { move_axis: axis, yaw, pitch, buttons: buttons_out }
}

/// How much a bot likes a card: rarity first, then new weapons and
/// main weapon upgrades.
fn card_score(option: &CardOption) -> u32 {
    use crate::shared::build::Offer;
    match option {
        CardOption::Item(item) => item.rarity() as u32 * 10,
        CardOption::Offer(card) => {
            let bonus = match card.offer {
                Offer::NewAuto(_) => 15,
                Offer::UpgradeMain { .. } => 5,
                Offer::UpgradeAuto { .. } => 3,
                Offer::Tome { .. } => 0,
            };
            card.rarity as u32 * 10 + bonus
        }
    }
}

/// Bots pick the card they like most (ties: the first one).
pub fn bot_pick(choice: &PendingChoice) -> usize {
    choice
        .options
        .iter()
        .enumerate()
        .max_by_key(|(i, option)| (card_score(option), std::cmp::Reverse(*i)))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn server_bot_choices(
    mut commands: Commands,
    mut bots: Query<(Entity, &Player, &PendingChoice, &mut Items, &mut Gear), With<BotBrain>>,
    mut events: MessageWriter<bevy_replicon::prelude::ToClients<GameEvent>>,
) {
    for (entity, player, choice, mut items, mut gear) in &mut bots {
        let index = bot_pick(choice);
        crate::game::chests::apply_choice(&mut commands, entity, player, choice, index, &mut items, &mut gear, &mut events);
    }
}

fn bot_client_choices(
    local: Res<LocalPlayer>,
    me: Query<(&Player, &PendingChoice)>,
    mut choose: MessageWriter<ChooseCard>,
    mut sent: Local<Vec<CardOption>>,
) {
    let Some((_, choice)) = me.iter().find(|(p, _)| Some(p.slot) == local.slot) else {
        sent.clear();
        return;
    };
    if *sent != choice.options {
        *sent = choice.options.clone();
        choose.write(ChooseCard { index: bot_pick(choice) as u8 });
    }
}

pub struct BotPlugin;

impl Plugin for BotPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, (server_bots, server_bot_choices).in_set(ServerSet::Input))
            .add_systems(
                Update,
                bot_client_choices.run_if(|c: Res<AppConfig>| c.launch == LaunchMode::BotClient || (c.autopilot && c.shots.is_none())),
            )
            .add_systems(
                FixedPreUpdate,
                bot_client_input
                    .run_if(|c: Res<AppConfig>| c.launch == LaunchMode::BotClient || c.autopilot)
                    .run_if(match_playing),
            )
            .add_systems(
                Update,
                (
                    log_server_summary.run_if(|r: Res<NetRole>| r.is_authority()).run_if(match_playing),
                    log_bot_client.run_if(|c: Res<AppConfig>| c.launch == LaunchMode::BotClient),
                )
                    .run_if(bevy::time::common_conditions::on_timer(std::time::Duration::from_secs(10))),
            )
            .add_systems(Update, bot_client_auto_restart.run_if(|c: Res<AppConfig>| c.launch == LaunchMode::BotClient));
    }
}

struct WorldSnapshot {
    mobs: Vec<Vec3>,
    chests: Vec<(Vec3, ChestKind)>,
    downed: Vec<Vec3>,
    goodies: Vec<Vec3>,
}

type GoodiesQuery<'w, 's> = Query<'w, 's, (Option<&'static Powerup>, Option<&'static Shrine>, Option<&'static ShrineCharge>)>;

fn snapshot(
    mobs: &Query<(&Mob, &MobState)>,
    chests: &Query<(&Chest, &ChestOpen)>,
    players: &Query<(&PlayerState, &Health)>,
    goodies: &GoodiesQuery,
) -> WorldSnapshot {
    WorldSnapshot {
        goodies: goodies
            .iter()
            .filter_map(|(p, s, c)| match (p, s, c) {
                (Some(p), ..) => Some(p.pos),
                (_, Some(s), Some(c)) if !c.used => Some(s.pos),
                _ => None,
            })
            .collect(),
        mobs: mobs.iter().map(|(m, s)| s.pos + Vec3::Y * m.kind.def().height * 0.5).collect(),
        chests: chests.iter().filter(|(_, o)| !o.opened).map(|(c, _)| (c.pos, c.kind)).collect(),
        downed: players.iter().filter(|(_, h)| h.downed).map(|(s, _)| s.sim.mv.pos).collect(),
    }
}

type BotQuery<'a> = (&'a mut BotBrain, &'a PlayerState, &'a Health, &'a Wallet, &'a mut InputQueue);

#[allow(clippy::type_complexity)]
fn server_bots(
    mut bots: Query<BotQuery, With<Owner>>,
    mobs: Query<(&Mob, &MobState)>,
    chests: Query<(&Chest, &ChestOpen)>,
    players: Query<(&PlayerState, &Health)>,
    goodies: GoodiesQuery,
    info: Single<&MatchInfo>,
    tick: Res<crate::game::SimTick>,
) {
    let world = snapshot(&mobs, &chests, &players, &goodies);
    for (mut brain, state, health, wallet, mut queue) in &mut bots {
        // Server bots see the world in the present: no lag compensation.
        queue.view_tick = tick.0;
        let view = BotView {
            me: &state.sim,
            downed: health.downed,
            gold: wallet.gold,
            elapsed: info.elapsed,
            chests_opened: wallet.chests_opened,
            mobs: &world.mobs,
            chests: &world.chests,
            downed_allies: &world.downed,
            goodies: &world.goodies,
        };
        let out = think(&mut brain, &view);
        let seq = queue.last_received + 1;
        queue.push_new(&[PlayerInput {
            seq,
            move_axis: out.move_axis,
            yaw: out.yaw,
            pitch: out.pitch,
            buttons: out.buttons,
            select_slot: 255,
        }]);
    }
}

#[allow(clippy::too_many_arguments)]
fn bot_client_input(
    mut brain: Local<BotBrain>,
    pred: Res<Prediction>,
    mut input: ResMut<InputState>,
    local: Res<LocalPlayer>,
    me: Query<(&Player, &Health, &Wallet)>,
    mobs: Query<(&Mob, &MobState)>,
    chests: Query<(&Chest, &ChestOpen)>,
    players: Query<(&PlayerState, &Health)>,
    goodies: GoodiesQuery,
    info: Single<&MatchInfo>,
) {
    if !pred.active {
        return;
    }
    let Some((_, health, wallet)) = me.iter().find(|(p, ..)| Some(p.slot) == local.slot) else { return };
    let world = snapshot(&mobs, &chests, &players, &goodies);
    let view = BotView {
        me: &pred.sim,
        downed: health.downed,
        gold: wallet.gold,
        elapsed: info.elapsed,
        chests_opened: wallet.chests_opened,
        mobs: &world.mobs,
        chests: &world.chests,
        downed_allies: &world.downed,
        goodies: &world.goodies,
    };
    let out = think(&mut brain, &view);
    input.move_axis = out.move_axis;
    input.yaw = out.yaw;
    input.pitch = out.pitch;
    input.held = out.buttons;
}

/// Periodic, human-readable summary of the run (server log).
fn log_server_summary(
    info: Single<&MatchInfo>,
    players: Query<(&Player, &Health, &Wallet, &Items, &PlayerState, &crate::game::players::PlayerScore, &Gear)>,
    mobs: Query<&Mob>,
    chests: Res<crate::game::chests::ChestRegistry>,
) {
    let line: Vec<String> = players
        .iter()
        .map(|(p, h, w, i, s, sc, g)| {
            format!(
                "{}[hp {:.0}/{:.0}{} gold {} items {} {:?} Nv{} autos {:?} kills {} pos ({:.0},{:.0},{:.0})]",
                p.name,
                h.hp,
                h.max,
                if h.downed { " DOWN" } else { "" },
                w.gold,
                i.0.total(),
                g.0.main,
                g.0.main_up.level + 1,
                g.0.autos.iter().map(|(a, l)| (a, l.level)).collect::<Vec<_>>(),
                sc.kills,
                s.sim.mv.pos.x,
                s.sim.mv.pos.y,
                s.sim.mv.pos.z
            )
        })
        .collect();
    info!(
        "[run] t={:.0}s lvl {} xp {:.0}/{:.0} swarm {} diff {:.2} mobs {} chests opened {} spawned {} | {}",
        info.elapsed,
        info.team_level,
        info.xp,
        info.xp_next,
        info.swarm,
        info.difficulty,
        mobs.iter().count(),
        chests.opened.len(),
        chests.spawned.len(),
        line.join(" ")
    );
}

fn log_bot_client(
    pred: Res<Prediction>,
    local: Res<LocalPlayer>,
    stats: Res<bevy_replicon::prelude::ClientStats>,
    me: Query<(&Player, &Health, &Wallet, &Items)>,
    mobs: Query<&Mob>,
) {
    let Some((p, h, w, i)) = me.iter().find(|(p, ..)| Some(p.slot) == local.slot) else {
        info!("[bot] waiting (slot {:?})", local.slot);
        return;
    };
    info!(
        "[bot] {} hp {:.0} gold {} items {} | pos ({:.1},{:.1},{:.1}) | rtt {:.0} ms, loss {:.1}% | predicted inputs pending {} corrections {} | mobs seen {}",
        p.name,
        h.hp,
        w.gold,
        i.0.total(),
        pred.sim.mv.pos.x,
        pred.sim.mv.pos.y,
        pred.sim.mv.pos.z,
        stats.rtt * 1000.0,
        stats.packet_loss * 100.0,
        pred.history.len(),
        pred.corrections,
        mobs.iter().count()
    );
}

/// A bot client on a dedicated server asks to start / restart the run.
fn bot_client_auto_restart(
    info: Query<&MatchInfo>,
    local: Res<LocalPlayer>,
    mut cmd: MessageWriter<LobbyCommand>,
    mut cooldown: Local<f32>,
    time: Res<Time>,
) {
    *cooldown -= time.delta_secs();
    let Ok(info) = info.single() else { return };
    if local.slot.is_some() && *cooldown <= 0.0 {
        match info.phase {
            Phase::Lobby => {
                cmd.write(LobbyCommand::Start);
                *cooldown = 3.0;
            }
            Phase::GameOver => {
                cmd.write(LobbyCommand::BackToLobby);
                *cooldown = 3.0;
            }
            Phase::Playing => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view<'a>(me: &'a PlayerSim, mobs: &'a [Vec3], chests: &'a [(Vec3, ChestKind)]) -> BotView<'a> {
        BotView { me, downed: false, gold: 1000, elapsed: 0.0, chests_opened: 0, mobs, chests, downed_allies: &[], goodies: &[] }
    }

    #[test]
    fn bots_pick_the_rarest_card() {
        use crate::shared::items::ItemKind;
        let choice = PendingChoice {
            chest: Some(ChestKind::Small),
            options: [ItemKind::PowerRounds, ItemKind::Crown, ItemKind::FrogLegs].into_iter().map(CardOption::Item).collect(),
        };
        assert_eq!(bot_pick(&choice), 1);
    }

    #[test]
    fn ignores_chests_too_high() {
        let me = PlayerSim::spawn_at(Vec3::ZERO);
        let chests = [(Vec3::new(10.0, 8.0, 0.0), ChestKind::Small)];
        let mut brain = BotBrain::default();
        think(&mut brain, &view(&me, &[], &chests));
        assert!(brain.chest_goal.is_none());
    }

    #[test]
    fn gives_up_unreachable_chest() {
        let me = PlayerSim::spawn_at(Vec3::ZERO);
        let chests = [(Vec3::new(10.0, 1.0, 0.0), ChestKind::Small)];
        let mut brain = BotBrain::default();
        for _ in 0..=CHEST_GIVE_UP_TICKS + 1 {
            think(&mut brain, &view(&me, &[], &chests));
        }
        assert_eq!(brain.ignored_chests.len(), 1);
        assert!(brain.chest_goal.is_none());
    }

    #[test]
    fn fights_before_shopping_when_threatened() {
        let me = PlayerSim::spawn_at(Vec3::ZERO);
        let chests = [(Vec3::new(10.0, 0.0, 0.0), ChestKind::Small)];
        let mobs = [Vec3::new(0.0, 0.5, -5.0)];
        let mut brain = BotBrain::default();
        think(&mut brain, &view(&me, &mobs, &chests));
        assert!(brain.chest_goal.is_none());
    }
}
