//! Procedurally synthesized sound effects (no audio files).
//!
//! Every sound is generated once at startup into a buffer of samples:
//! oscillators (sine, square, saw), white noise, pitch sweeps and simple
//! envelopes. A custom Bevy audio source (`Synth`) plays these buffers.

use std::sync::Arc;
use std::time::Duration;

use bevy::audio::{AddAudioSource, ChannelCount, SampleRate, Source, Volume};
use bevy::prelude::*;

use super::MainCamera;
use crate::config::AppConfig;
use crate::shared::rng::Rng;
use crate::shared::build::MainWeapon;

const RATE: u32 = 44_100;

/// A sound buffer (mono, 44.1 kHz). `Arc` = shared without copying.
#[derive(Asset, TypePath, Clone)]
pub struct Synth {
    samples: Arc<[f32]>,
}

pub struct SynthDecoder {
    samples: Arc<[f32]>,
    pos: usize,
}

impl Iterator for SynthDecoder {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let s = self.samples.get(self.pos).copied();
        self.pos += 1;
        s
    }
}

impl Source for SynthDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::new(1).expect("1 channel")
    }
    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(RATE).expect("rate")
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(self.samples.len() as f32 / RATE as f32))
    }
}

impl Decodable for Synth {
    type Decoder = SynthDecoder;
    fn decoder(&self) -> SynthDecoder {
        SynthDecoder { samples: self.samples.clone(), pos: 0 }
    }
}

/// All sound effects of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sfx {
    Shot(MainWeapon),
    Hit,
    Crit,
    Kill,
    Jump,
    Land,
    Dash,
    Pad,
    Reload,
    Empty,
    Switch,
    ChestOpen,
    Coin,
    Hurt,
    Explosion,
    Telegraph,
    BossRoar,
    Downed,
    Revived,
    Click,
}

/// Request to play a sound, optionally at a world position (distance attenuation).
#[derive(Message, Clone, Copy, Debug)]
pub struct PlaySfx {
    pub sfx: Sfx,
    pub pos: Option<Vec3>,
    pub volume: f32,
}

impl PlaySfx {
    pub fn ui(sfx: Sfx) -> Self {
        Self { sfx, pos: None, volume: 1.0 }
    }
    pub fn at(sfx: Sfx, pos: Vec3) -> Self {
        Self { sfx, pos: Some(pos), volume: 1.0 }
    }
}

#[derive(Resource, Default)]
struct SoundBank(std::collections::HashMap<Sfx, Handle<Synth>>);

pub struct SfxPlugin;

impl Plugin for SfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<Synth>()
            .add_message::<PlaySfx>()
            .init_resource::<SoundBank>()
            .add_systems(Startup, build_sounds)
            .add_systems(PostUpdate, (play_sounds, apply_volume));
    }
}

// --- Tiny synthesizer -------------------------------------------------------

fn len(seconds: f32) -> usize {
    (seconds * RATE as f32) as usize
}

/// Exponential decay envelope with a short attack (no clicks).
fn env(i: usize, n: usize, decay: f32) -> f32 {
    let t = i as f32 / RATE as f32;
    let attack = (t / 0.004).min(1.0);
    let release = ((n - i) as f32 / (RATE as f32 * 0.01)).min(1.0);
    attack * release * (-t * decay).exp()
}

/// A pitch-swept oscillator. `shape`: 0 sine, 1 square, 2 saw, 3 triangle.
fn osc(seconds: f32, f0: f32, f1: f32, shape: u8, decay: f32) -> Vec<f32> {
    let n = len(seconds);
    let mut phase = 0.0f32;
    (0..n)
        .map(|i| {
            let k = i as f32 / n as f32;
            let f = f0 * (f1 / f0).powf(k); // exponential sweep sounds natural
            phase = (phase + f / RATE as f32).fract();
            let v = match shape {
                0 => (phase * std::f32::consts::TAU).sin(),
                1 => if phase < 0.5 { 1.0 } else { -1.0 },
                2 => phase * 2.0 - 1.0,
                _ => 1.0 - 4.0 * (phase - 0.5).abs(),
            };
            v * env(i, n, decay)
        })
        .collect()
}

/// Filtered white noise: `smooth` in (0,1], lower = darker sound.
fn noise(seconds: f32, smooth: f32, decay: f32, seed: u64) -> Vec<f32> {
    let n = len(seconds);
    let mut rng = Rng::new(seed);
    let mut last = 0.0;
    (0..n)
        .map(|i| {
            let white = rng.next_f32() * 2.0 - 1.0;
            last += (white - last) * smooth;
            last * env(i, n, decay)
        })
        .collect()
}

/// Sums several layers with gains, then normalizes to `peak`.
fn mix(layers: &[(&[f32], f32)], peak: f32) -> Vec<f32> {
    let n = layers.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
    let mut out = vec![0.0; n];
    for (layer, gain) in layers {
        for (o, s) in out.iter_mut().zip(layer.iter()) {
            *o += s * gain;
        }
    }
    let max = out.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(1e-6);
    out.iter_mut().for_each(|s| *s *= peak / max);
    out
}

/// Notes one after the other (for jingles).
fn sequence(notes: &[(f32, f32)], shape: u8) -> Vec<f32> {
    let mut out = Vec::new();
    for (i, (freq, dur)) in notes.iter().enumerate() {
        let note = osc(*dur * 2.0, *freq, *freq, shape, 7.0);
        let start = out.len().min(len(notes[..i].iter().map(|n| n.1).sum()));
        out.resize(out.len().max(start + note.len()), 0.0);
        for (j, s) in note.iter().enumerate() {
            out[start + j] += s * 0.6;
        }
    }
    out
}

fn synthesize(sfx: Sfx) -> Vec<f32> {
    match sfx {
        Sfx::Shot(w) => match w {
            // Whoosh + crackle.
            MainWeapon::FireStaff => mix(&[(&noise(0.3, 0.15, 9.0, 30), 1.0), (&osc(0.25, 180.0, 90.0, 2, 10.0), 0.5)], 0.6),
            MainWeapon::Bow => mix(&[(&osc(0.12, 140.0, 90.0, 0, 25.0), 1.0), (&noise(0.15, 0.5, 30.0, 31), 0.6)], 0.55),
            MainWeapon::Revolver => mix(&[(&noise(0.35, 0.4, 12.0, 20), 1.0), (&osc(0.25, 220.0, 55.0, 0, 14.0), 0.9)], 0.8),
            MainWeapon::Shotgun => mix(&[(&noise(0.45, 0.35, 9.0, 3), 1.0), (&osc(0.3, 120.0, 40.0, 0, 12.0), 0.9)], 0.9),
            MainWeapon::Sniper => mix(&[(&osc(0.6, 2400.0, 120.0, 2, 5.0), 0.6), (&noise(0.5, 0.25, 7.0, 24), 0.8), (&osc(0.5, 90.0, 40.0, 0, 6.0), 0.8)], 1.0),
            MainWeapon::LightningStaff => mix(&[(&noise(0.25, 0.9, 14.0, 32), 1.0), (&osc(0.2, 1200.0, 300.0, 3, 15.0), 0.4)], 0.6),
            MainWeapon::Bananarang => mix(&[(&osc(0.3, 300.0, 600.0, 0, 8.0), 0.8), (&noise(0.25, 0.1, 10.0, 33), 0.6)], 0.5),
            MainWeapon::HeroSword => mix(&[(&noise(0.25, 0.2, 12.0, 34), 1.0), (&osc(0.2, 900.0, 1400.0, 0, 14.0), 0.35)], 0.6),
        },
        Sfx::Hit => mix(&[(&osc(0.06, 1400.0, 1100.0, 3, 50.0), 1.0)], 0.35),
        Sfx::Crit => mix(&[(&osc(0.12, 1800.0, 2400.0, 3, 25.0), 1.0), (&osc(0.12, 900.0, 1200.0, 0, 25.0), 0.6)], 0.45),
        Sfx::Kill => mix(&[(&osc(0.2, 600.0, 1500.0, 3, 14.0), 1.0), (&noise(0.12, 0.7, 30.0, 7), 0.3)], 0.5),
        Sfx::Jump => mix(&[(&osc(0.12, 220.0, 440.0, 3, 20.0), 1.0), (&noise(0.08, 0.4, 40.0, 8), 0.4)], 0.3),
        Sfx::Land => mix(&[(&noise(0.12, 0.2, 30.0, 9), 1.0), (&osc(0.1, 90.0, 50.0, 0, 30.0), 0.8)], 0.45),
        Sfx::Dash => mix(&[(&noise(0.3, 0.25, 9.0, 10), 1.0), (&osc(0.25, 200.0, 900.0, 0, 10.0), 0.25)], 0.5),
        Sfx::Pad => mix(&[(&osc(0.5, 150.0, 1200.0, 3, 5.0), 1.0), (&noise(0.3, 0.3, 10.0, 11), 0.4)], 0.55),
        Sfx::Reload => mix(&[(&noise(0.05, 0.9, 80.0, 12), 1.0), (&osc(0.06, 1800.0, 1600.0, 1, 70.0), 0.4)], 0.35),
        Sfx::Empty => mix(&[(&osc(0.04, 2500.0, 2200.0, 1, 90.0), 1.0)], 0.25),
        Sfx::Switch => mix(&[(&noise(0.08, 0.8, 50.0, 13), 1.0), (&osc(0.05, 1200.0, 900.0, 3, 60.0), 0.5)], 0.3),
        Sfx::ChestOpen => {
            let jingle = sequence(&[(523.0, 0.08), (659.0, 0.08), (784.0, 0.08), (1047.0, 0.25)], 3);
            mix(&[(&jingle, 1.0), (&noise(0.15, 0.3, 20.0, 14), 0.3)], 0.55)
        }
        Sfx::Coin => mix(&[(&sequence(&[(1320.0, 0.05), (1760.0, 0.12)], 1), 1.0)], 0.2),
        Sfx::Hurt => mix(&[(&osc(0.2, 300.0, 120.0, 2, 14.0), 1.0), (&noise(0.15, 0.4, 20.0, 15), 0.6)], 0.55),
        Sfx::Explosion => mix(&[(&noise(1.2, 0.08, 3.5, 16), 1.0), (&osc(0.8, 70.0, 25.0, 0, 4.0), 1.0)], 1.0),
        Sfx::Telegraph => mix(&[(&osc(0.35, 140.0, 90.0, 2, 6.0), 1.0), (&osc(0.35, 147.0, 95.0, 2, 6.0), 0.7)], 0.35),
        Sfx::BossRoar => mix(&[(&osc(1.6, 80.0, 45.0, 2, 1.5), 1.0), (&osc(1.6, 83.0, 47.0, 1, 1.5), 0.6), (&noise(1.6, 0.1, 1.5, 17), 0.7)], 0.9),
        Sfx::Downed => mix(&[(&sequence(&[(440.0, 0.15), (330.0, 0.15), (220.0, 0.4)], 2), 1.0)], 0.5),
        Sfx::Revived => mix(&[(&sequence(&[(392.0, 0.1), (523.0, 0.1), (784.0, 0.3)], 0), 1.0)], 0.5),
        Sfx::Click => mix(&[(&osc(0.03, 900.0, 700.0, 3, 80.0), 1.0)], 0.25),
    }
}

fn build_sounds(mut bank: ResMut<SoundBank>, mut assets: ResMut<Assets<Synth>>) {
    let mut all = vec![
        Sfx::Hit,
        Sfx::Crit,
        Sfx::Kill,
        Sfx::Jump,
        Sfx::Land,
        Sfx::Dash,
        Sfx::Pad,
        Sfx::Reload,
        Sfx::Empty,
        Sfx::Switch,
        Sfx::ChestOpen,
        Sfx::Coin,
        Sfx::Hurt,
        Sfx::Explosion,
        Sfx::Telegraph,
        Sfx::BossRoar,
        Sfx::Downed,
        Sfx::Revived,
        Sfx::Click,
    ];
    all.extend(MainWeapon::ALL.iter().map(|w| Sfx::Shot(*w)));
    for sfx in all {
        let samples: Arc<[f32]> = synthesize(sfx).into();
        bank.0.insert(sfx, assets.add(Synth { samples }));
    }
}

fn play_sounds(
    mut commands: Commands,
    mut requests: MessageReader<PlaySfx>,
    bank: Res<SoundBank>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut rng: Local<Option<Rng>>,
) {
    let rng = rng.get_or_insert_with(|| Rng::new(99));
    let cam = camera.single().map(|c| c.translation()).unwrap_or_default();
    let mut played = 0;
    for req in requests.read() {
        // Never stack too many sounds in one frame (shotgun hits...).
        if played > 12 {
            break;
        }
        let Some(handle) = bank.0.get(&req.sfx) else { continue };
        let attenuation = match req.pos {
            Some(p) => {
                let d = p.distance(cam);
                (1.0 / (1.0 + d * d / 300.0)).max(0.0)
            }
            None => 1.0,
        };
        let volume = req.volume * attenuation;
        if volume < 0.02 {
            continue;
        }
        // Small random pitch variation avoids the "machine gun" repetition effect.
        let speed = rng.range(0.93, 1.07);
        commands.spawn((
            AudioPlayer(handle.clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)).with_speed(speed),
        ));
        played += 1;
    }
}

fn apply_volume(config: Res<AppConfig>, mut global: ResMut<GlobalVolume>) {
    if config.is_changed() {
        global.volume = Volume::Linear(config.volume);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_sounds_are_valid() {
        let mut all = vec![Sfx::Hit, Sfx::ChestOpen, Sfx::Explosion, Sfx::BossRoar, Sfx::Downed];
        all.extend(MainWeapon::ALL.iter().map(|w| Sfx::Shot(*w)));
        for sfx in all {
            let s = synthesize(sfx);
            assert!(!s.is_empty(), "{sfx:?}");
            assert!(s.iter().all(|v| v.is_finite() && v.abs() <= 1.0 + 1e-4), "{sfx:?} out of range");
        }
    }
}
