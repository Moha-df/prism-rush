//! Item and weapon icons, *drawn in code* (no image files).
//!
//! A tiny 2D painter: shapes are described by an "inside?" test, each pixel
//! is sampled 4x4 times for smooth (anti-aliased) edges, then a dark outline
//! pass gives every icon a sticker look. Coordinates are in a 0..64 square,
//! y pointing down.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::shared::items::ItemKind;
use crate::shared::build::{AutoWeapon, MainWeapon, Tome};

const SIZE: usize = 64;
const SS: usize = 4; // samples per axis

type Rgba = [f32; 4];

pub const fn rgb(r: f32, g: f32, b: f32) -> Rgba {
    [r, g, b, 1.0]
}

/// RGBA float canvas.
pub struct Canvas {
    px: Vec<Rgba>,
}

impl Default for Canvas {
    fn default() -> Self {
        Self { px: vec![[0.0; 4]; SIZE * SIZE] }
    }
}

fn dist_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

impl Canvas {
    /// Paints the pixels covered by `inside` with `color` (alpha blended).
    pub fn fill(&mut self, color: Rgba, inside: impl Fn(Vec2) -> bool) -> &mut Self {
        for y in 0..SIZE {
            for x in 0..SIZE {
                let mut hits = 0;
                for sy in 0..SS {
                    for sx in 0..SS {
                        let p = Vec2::new(x as f32 + (sx as f32 + 0.5) / SS as f32, y as f32 + (sy as f32 + 0.5) / SS as f32);
                        if inside(p) {
                            hits += 1;
                        }
                    }
                }
                if hits == 0 {
                    continue;
                }
                let a = color[3] * hits as f32 / (SS * SS) as f32;
                let dst = &mut self.px[y * SIZE + x];
                for c in 0..3 {
                    dst[c] = color[c] * a + dst[c] * (1.0 - a);
                }
                dst[3] = a + dst[3] * (1.0 - a);
            }
        }
        self
    }

    pub fn circle(&mut self, c: (f32, f32), r: f32, color: Rgba) -> &mut Self {
        let c = Vec2::new(c.0, c.1);
        self.fill(color, move |p| p.distance(c) <= r)
    }

    pub fn ellipse(&mut self, c: (f32, f32), rx: f32, ry: f32, color: Rgba) -> &mut Self {
        self.fill(color, move |p| {
            let d = Vec2::new((p.x - c.0) / rx, (p.y - c.1) / ry);
            d.length_squared() <= 1.0
        })
    }

    pub fn ring(&mut self, c: (f32, f32), r: f32, thickness: f32, color: Rgba) -> &mut Self {
        let c = Vec2::new(c.0, c.1);
        self.fill(color, move |p| (p.distance(c) - r).abs() <= thickness * 0.5)
    }

    pub fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: Rgba) -> &mut Self {
        self.fill(color, move |p| p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1)
    }

    /// Rectangle with rounded corners.
    pub fn rounded(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32, color: Rgba) -> &mut Self {
        self.fill(color, move |p| {
            let q = Vec2::new(p.x.clamp(x0 + r, x1 - r), p.y.clamp(y0 + r, y1 - r));
            p.distance(q) <= r
        })
    }

    pub fn line(&mut self, a: (f32, f32), b: (f32, f32), width: f32, color: Rgba) -> &mut Self {
        let (a, b) = (Vec2::new(a.0, a.1), Vec2::new(b.0, b.1));
        self.fill(color, move |p| dist_to_segment(p, a, b) <= width * 0.5)
    }

    /// Polyline (thick, round joints).
    pub fn path(&mut self, pts: &[(f32, f32)], width: f32, color: Rgba) -> &mut Self {
        for w in pts.windows(2) {
            self.line(w[0], w[1], width, color);
        }
        self
    }

    /// Convex or concave polygon (even-odd rule).
    pub fn poly(&mut self, pts: &[(f32, f32)], color: Rgba) -> &mut Self {
        let pts: Vec<Vec2> = pts.iter().map(|p| Vec2::new(p.0, p.1)).collect();
        self.fill(color, move |p| {
            let mut inside = false;
            let mut j = pts.len() - 1;
            for i in 0..pts.len() {
                let (a, b) = (pts[i], pts[j]);
                if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
                    inside = !inside;
                }
                j = i;
            }
            inside
        })
    }

    /// Heart shape centered at `c`, size ~ `s`.
    pub fn heart(&mut self, c: (f32, f32), s: f32, color: Rgba) -> &mut Self {
        self.circle((c.0 - s * 0.25, c.1 - s * 0.12), s * 0.27, color)
            .circle((c.0 + s * 0.25, c.1 - s * 0.12), s * 0.27, color)
            .poly(&[(c.0 - s * 0.5, c.1 - s * 0.05), (c.0 + s * 0.5, c.1 - s * 0.05), (c.0, c.1 + s * 0.45)], color)
    }

    /// Zigzag lightning bolt.
    pub fn bolt(&mut self, x: f32, y: f32, s: f32, color: Rgba) -> &mut Self {
        self.poly(
            &[
                (x + 0.1 * s, y),
                (x + 0.6 * s, y),
                (x + 0.4 * s, y + 0.4 * s),
                (x + 0.75 * s, y + 0.4 * s),
                (x + 0.2 * s, y + s),
                (x + 0.35 * s, y + 0.55 * s),
                (x, y + 0.55 * s),
            ],
            color,
        )
    }

    /// Dark outline around everything drawn so far (sticker look).
    pub fn outline(&mut self, color: Rgba, radius: i32) -> &mut Self {
        let src = self.px.clone();
        let alpha = |x: i32, y: i32| -> f32 {
            if x < 0 || y < 0 || x >= SIZE as i32 || y >= SIZE as i32 { 0.0 } else { src[y as usize * SIZE + x as usize][3] }
        };
        for y in 0..SIZE as i32 {
            for x in 0..SIZE as i32 {
                let own = alpha(x, y);
                let mut near: f32 = 0.0;
                for dy in -radius..=radius {
                    for dx in -radius..=radius {
                        if dx * dx + dy * dy <= radius * radius {
                            near = near.max(alpha(x + dx, y + dy));
                        }
                    }
                }
                let a = (near - own).max(0.0) * color[3];
                if a > 0.0 {
                    let dst = &mut self.px[y as usize * SIZE + x as usize];
                    for c in 0..3 {
                        dst[c] = color[c] * a + dst[c] * (1.0 - a);
                    }
                    dst[3] = a + dst[3] * (1.0 - a);
                }
            }
        }
        self
    }

    pub fn to_image(&self) -> Image {
        let mut data = Vec::with_capacity(SIZE * SIZE * 4);
        for p in &self.px {
            // Un-premultiply is not needed: we store straight alpha.
            for c in p.iter().take(3) {
                data.push((c.clamp(0.0, 1.0) * 255.0) as u8);
            }
            data.push((p[3].clamp(0.0, 1.0) * 255.0) as u8);
        }
        Image::new(
            Extent3d { width: SIZE as u32, height: SIZE as u32, depth_or_array_layers: 1 },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        )
    }
}

// Palette for icons (linear-ish values, they are converted when saved).
const INK: Rgba = rgb(0.03, 0.02, 0.05);
const WHITE: Rgba = rgb(1.0, 1.0, 1.0);
const RED: Rgba = rgb(0.85, 0.08, 0.08);
const DARK_RED: Rgba = rgb(0.35, 0.02, 0.03);
const GREEN: Rgba = rgb(0.2, 0.7, 0.12);
const DARK_GREEN: Rgba = rgb(0.06, 0.32, 0.05);
const BLUE: Rgba = rgb(0.15, 0.5, 1.0);
const CYAN: Rgba = rgb(0.3, 0.9, 1.0);
const YELLOW: Rgba = rgb(1.0, 0.8, 0.1);
const GOLD: Rgba = rgb(0.95, 0.6, 0.08);
const ORANGE: Rgba = rgb(1.0, 0.35, 0.03);
const GREY: Rgba = rgb(0.45, 0.47, 0.52);
const DARK_GREY: Rgba = rgb(0.15, 0.16, 0.2);
const BROWN: Rgba = rgb(0.35, 0.18, 0.06);
const BEIGE: Rgba = rgb(0.9, 0.75, 0.5);
const PURPLE: Rgba = rgb(0.5, 0.15, 0.85);
const PINK: Rgba = rgb(1.0, 0.45, 0.65);

/// Draws one item icon.
pub fn paint_item(item: ItemKind) -> Canvas {
    use ItemKind::*;
    let mut c = Canvas::default();
    match item {
        FrogLegs => {
            // A happy frog: body, two big eyes, smile.
            c.ellipse((32.0, 40.0), 22.0, 16.0, GREEN)
                .circle((20.0, 22.0), 9.0, GREEN)
                .circle((44.0, 22.0), 9.0, GREEN)
                .circle((20.0, 21.0), 6.0, WHITE)
                .circle((44.0, 21.0), 6.0, WHITE)
                .circle((21.0, 22.0), 3.0, INK)
                .circle((45.0, 22.0), 3.0, INK)
                .path(&[(20.0, 42.0), (26.0, 47.0), (38.0, 47.0), (44.0, 42.0)], 3.0, DARK_GREEN)
                .circle((24.0, 36.0), 2.5, PINK)
                .circle((40.0, 36.0), 2.5, PINK);
        }
        Jetpack => {
            c.rounded(16.0, 10.0, 30.0, 40.0, 5.0, GREY)
                .rounded(34.0, 10.0, 48.0, 40.0, 5.0, GREY)
                .rect(28.0, 16.0, 36.0, 32.0, DARK_GREY)
                .poly(&[(17.0, 40.0), (29.0, 40.0), (23.0, 60.0)], ORANGE)
                .poly(&[(35.0, 40.0), (47.0, 40.0), (41.0, 60.0)], ORANGE)
                .poly(&[(20.0, 40.0), (26.0, 40.0), (23.0, 52.0)], YELLOW)
                .poly(&[(38.0, 40.0), (44.0, 40.0), (41.0, 52.0)], YELLOW)
                .rect(16.0, 14.0, 30.0, 17.0, RED)
                .rect(34.0, 14.0, 48.0, 17.0, RED);
        }
        SwiftBoots => {
            c.poly(&[(22.0, 10.0), (36.0, 10.0), (36.0, 38.0), (54.0, 44.0), (54.0, 54.0), (22.0, 54.0)], RED)
                .rect(22.0, 50.0, 54.0, 56.0, WHITE)
                .line((6.0, 28.0), (16.0, 28.0), 3.0, CYAN)
                .line((4.0, 38.0), (16.0, 38.0), 3.0, CYAN)
                .line((8.0, 48.0), (16.0, 48.0), 3.0, CYAN)
                .poly(&[(40.0, 14.0), (52.0, 8.0), (46.0, 20.0)], YELLOW);
        }
        Thruster => {
            c.poly(&[(20.0, 8.0), (44.0, 8.0), (50.0, 34.0), (14.0, 34.0)], GREY)
                .rect(14.0, 30.0, 50.0, 36.0, DARK_GREY)
                .poly(&[(16.0, 36.0), (48.0, 36.0), (32.0, 62.0)], BLUE)
                .poly(&[(24.0, 36.0), (40.0, 36.0), (32.0, 52.0)], CYAN)
                .rect(26.0, 14.0, 38.0, 20.0, ORANGE);
        }
        DashCoil => {
            c.ring((32.0, 32.0), 22.0, 5.0, CYAN).ring((32.0, 32.0), 14.0, 3.0, BLUE).bolt(20.0, 12.0, 40.0, YELLOW);
        }
        SlideWax => {
            c.circle((32.0, 38.0), 16.0, CYAN)
                .poly(&[(17.0, 34.0), (47.0, 34.0), (32.0, 8.0)], CYAN)
                .circle((26.0, 38.0), 5.0, WHITE)
                .line((8.0, 58.0), (56.0, 58.0), 3.0, BLUE);
        }
        FeatherCape => {
            c.fill(WHITE, |p| {
                let q = Vec2::new(p.x - 32.0, p.y - 32.0);
                let r = Vec2::from_angle(0.7).rotate(q);
                (r.x / 10.0).powi(2) + (r.y / 26.0).powi(2) <= 1.0
            })
            .line((48.0, 52.0), (18.0, 12.0), 2.5, GREY)
            .line((28.0, 26.0), (20.0, 30.0), 2.0, GREY)
            .line((34.0, 34.0), (26.0, 39.0), 2.0, GREY);
        }
        PhaseStep => {
            // A ghost.
            c.circle((32.0, 26.0), 16.0, [0.9, 0.95, 1.0, 0.85])
                .rect(16.0, 26.0, 48.0, 50.0, [0.9, 0.95, 1.0, 0.85])
                .poly(&[(16.0, 50.0), (22.0, 58.0), (27.0, 50.0), (32.0, 58.0), (37.0, 50.0), (42.0, 58.0), (48.0, 50.0)], [0.9, 0.95, 1.0, 0.85])
                .ellipse((26.0, 26.0), 3.0, 5.0, INK)
                .ellipse((38.0, 26.0), 3.0, 5.0, INK);
        }
        PowerRounds => {
            for (i, x) in [16.0, 32.0, 48.0].iter().enumerate() {
                let y = 14.0 + i as f32 * 2.0;
                c.rect(x - 6.0, y + 12.0, x + 6.0, y + 40.0, GOLD).ellipse((*x, y + 12.0), 6.0, 12.0, ORANGE).rect(x - 6.0, y + 36.0, x + 6.0, y + 40.0, BROWN);
            }
        }
        HairTrigger => {
            c.ring((32.0, 32.0), 20.0, 4.0, RED)
                .line((32.0, 4.0), (32.0, 22.0), 4.0, RED)
                .line((32.0, 42.0), (32.0, 60.0), 4.0, RED)
                .line((4.0, 32.0), (22.0, 32.0), 4.0, RED)
                .line((42.0, 32.0), (60.0, 32.0), 4.0, RED)
                .circle((32.0, 32.0), 4.0, YELLOW);
        }
        CritLens => {
            c.circle((28.0, 28.0), 18.0, CYAN)
                .ring((28.0, 28.0), 18.0, 5.0, GREY)
                .line((41.0, 41.0), (56.0, 56.0), 7.0, BROWN)
                .poly(&[(28.0, 16.0), (31.0, 25.0), (40.0, 28.0), (31.0, 31.0), (28.0, 40.0), (25.0, 31.0), (16.0, 28.0), (25.0, 25.0)], YELLOW);
        }
        Executioner => {
            c.line((14.0, 56.0), (44.0, 14.0), 6.0, BROWN).fill(GREY, |p| {
                let d = p.distance(Vec2::new(44.0, 22.0));
                d < 18.0 && p.x > 38.0 && p.y < 38.0 && (p - Vec2::new(30.0, 8.0)).dot(Vec2::new(1.0, 1.0)) > 0.0
            });
            c.ring((44.0, 22.0), 17.0, 3.0, WHITE);
        }
        UnstableRounds => {
            c.circle((30.0, 38.0), 18.0, DARK_GREY)
                .circle((24.0, 32.0), 5.0, GREY)
                .path(&[(40.0, 24.0), (46.0, 14.0), (52.0, 12.0)], 3.0, BEIGE)
                .poly(&[(52.0, 2.0), (55.0, 9.0), (62.0, 12.0), (55.0, 15.0), (52.0, 22.0), (49.0, 15.0), (42.0, 12.0), (49.0, 9.0)], ORANGE)
                .circle((52.0, 12.0), 3.0, YELLOW);
        }
        TeslaCoil => {
            c.rect(26.0, 30.0, 38.0, 60.0, GREY)
                .ring((32.0, 36.0), 9.0, 3.0, ORANGE)
                .ring((32.0, 46.0), 9.0, 3.0, ORANGE)
                .circle((32.0, 20.0), 9.0, GREY)
                .bolt(4.0, 4.0, 22.0, CYAN)
                .bolt(40.0, 6.0, 22.0, CYAN);
        }
        HollowPoint => {
            c.rect(22.0, 26.0, 42.0, 60.0, GOLD).ellipse((32.0, 26.0), 10.0, 18.0, ORANGE).circle((32.0, 16.0), 4.0, BROWN).rect(22.0, 54.0, 42.0, 60.0, BROWN);
        }
        Berserker => {
            c.ellipse((32.0, 36.0), 20.0, 22.0, RED)
                .poly(&[(14.0, 22.0), (6.0, 2.0), (22.0, 16.0)], BEIGE)
                .poly(&[(50.0, 22.0), (58.0, 2.0), (42.0, 16.0)], BEIGE)
                .poly(&[(18.0, 28.0), (30.0, 34.0), (18.0, 36.0)], YELLOW)
                .poly(&[(46.0, 28.0), (34.0, 34.0), (46.0, 36.0)], YELLOW)
                .path(&[(20.0, 48.0), (26.0, 44.0), (32.0, 48.0), (38.0, 44.0), (44.0, 48.0)], 3.0, INK);
        }
        Bloodlust => {
            c.circle((36.0, 40.0), 14.0, RED)
                .poly(&[(23.0, 36.0), (49.0, 36.0), (36.0, 8.0)], RED)
                .circle((32.0, 40.0), 4.0, PINK)
                .line((4.0, 30.0), (16.0, 30.0), 3.0, ORANGE)
                .line((2.0, 40.0), (16.0, 40.0), 3.0, ORANGE)
                .line((6.0, 50.0), (18.0, 50.0), 3.0, ORANGE);
        }
        VitalHeart => {
            c.heart((32.0, 34.0), 50.0, RED).circle((22.0, 24.0), 4.0, PINK);
        }
        RegenMoss => {
            c.circle((32.0, 32.0), 26.0, DARK_GREEN)
                .rect(26.0, 12.0, 38.0, 52.0, GREEN)
                .rect(12.0, 26.0, 52.0, 38.0, GREEN)
                .ellipse((44.0, 14.0), 8.0, 5.0, GREEN);
        }
        ArmorPlate => {
            c.poly(&[(12.0, 10.0), (52.0, 10.0), (52.0, 32.0), (32.0, 58.0), (12.0, 32.0)], GREY)
                .poly(&[(18.0, 16.0), (46.0, 16.0), (46.0, 31.0), (32.0, 50.0), (18.0, 31.0)], [0.65, 0.68, 0.74, 1.0])
                .rect(30.0, 16.0, 34.0, 48.0, GREY);
        }
        VampireFangs => {
            c.ellipse((32.0, 32.0), 26.0, 18.0, DARK_RED)
                .ellipse((32.0, 30.0), 22.0, 10.0, RED)
                .poly(&[(18.0, 26.0), (26.0, 26.0), (22.0, 46.0)], WHITE)
                .poly(&[(38.0, 26.0), (46.0, 26.0), (42.0, 46.0)], WHITE)
                .circle((22.0, 52.0), 3.0, RED);
        }
        EnergyShield => {
            c.fill([0.3, 0.7, 1.0, 0.4], |p| p.distance(Vec2::splat(32.0)) < 26.0)
                .ring((32.0, 32.0), 25.0, 4.0, CYAN)
                .poly(&[(32.0, 14.0), (47.0, 23.0), (47.0, 41.0), (32.0, 50.0), (17.0, 41.0), (17.0, 23.0)], [0.6, 0.95, 1.0, 0.8]);
        }
        Thorns => {
            c.ring((32.0, 32.0), 16.0, 6.0, DARK_GREEN);
            for i in 0..8 {
                let a = i as f32 / 8.0 * std::f32::consts::TAU;
                let (s, co) = a.sin_cos();
                c.poly(&[(32.0 + co * 18.0 - s * 4.0, 32.0 + s * 18.0 + co * 4.0), (32.0 + co * 18.0 + s * 4.0, 32.0 + s * 18.0 - co * 4.0), (32.0 + co * 30.0, 32.0 + s * 30.0)], GREEN);
            }
        }
        Ankh => {
            c.ring((32.0, 18.0), 10.0, 6.0, GOLD).rect(14.0, 28.0, 50.0, 34.0, GOLD).rect(29.0, 28.0, 35.0, 60.0, GOLD);
        }
        LuckyCoin => {
            c.circle((32.0, 32.0), 24.0, GOLD)
                .ring((32.0, 32.0), 18.0, 3.0, YELLOW)
                .poly(&[(32.0, 18.0), (36.0, 28.0), (46.0, 29.0), (38.0, 36.0), (41.0, 46.0), (32.0, 40.0), (23.0, 46.0), (26.0, 36.0), (18.0, 29.0), (28.0, 28.0)], YELLOW);
        }
        ExtendedMag => {
            // A huge bullet.
            c.rounded(20.0, 24.0, 44.0, 58.0, 3.0, GOLD)
                .ellipse((32.0, 24.0), 12.0, 18.0, ORANGE)
                .rect(20.0, 50.0, 44.0, 54.0, BROWN);
        }
        QuickHands => {
            // A feather in the wind.
            c.poly(&[(14.0, 54.0), (24.0, 30.0), (44.0, 10.0), (52.0, 12.0), (40.0, 34.0), (18.0, 56.0)], rgb(0.85, 0.95, 1.0))
                .line((12.0, 58.0), (46.0, 14.0), 2.0, GREY)
                .line((40.0, 44.0), (58.0, 44.0), 2.5, CYAN)
                .line((44.0, 52.0), (60.0, 52.0), 2.5, CYAN);
        }
        TreasureMap => {
            c.poly(&[(8.0, 12.0), (56.0, 8.0), (52.0, 54.0), (12.0, 58.0)], BEIGE)
                .path(&[(16.0, 46.0), (24.0, 36.0), (32.0, 40.0), (40.0, 28.0)], 2.0, BROWN)
                .line((38.0, 18.0), (48.0, 28.0), 4.0, RED)
                .line((48.0, 18.0), (38.0, 28.0), 4.0, RED);
        }
        Clover => {
            for (dx, dy) in [(-9.0, -9.0), (9.0, -9.0), (-9.0, 9.0), (9.0, 9.0)] {
                c.circle((30.0 + dx, 28.0 + dy), 10.0, GREEN);
            }
            c.path(&[(30.0, 30.0), (36.0, 48.0), (44.0, 58.0)], 4.0, DARK_GREEN);
        }
        Medkit => {
            c.rounded(8.0, 16.0, 56.0, 54.0, 6.0, WHITE).rect(24.0, 10.0, 40.0, 16.0, GREY).rect(28.0, 22.0, 36.0, 48.0, RED).rect(19.0, 31.0, 45.0, 39.0, RED);
        }
        Crown => {
            c.poly(&[(8.0, 48.0), (8.0, 18.0), (20.0, 32.0), (32.0, 12.0), (44.0, 32.0), (56.0, 18.0), (56.0, 48.0)], GOLD)
                .rect(8.0, 46.0, 56.0, 54.0, YELLOW)
                .circle((32.0, 38.0), 5.0, RED)
                .circle((18.0, 41.0), 3.5, BLUE)
                .circle((46.0, 41.0), 3.5, GREEN);
        }
        DragonHeart => {
            c.poly(&[(20.0, 20.0), (26.0, 4.0), (32.0, 16.0), (38.0, 2.0), (44.0, 20.0)], ORANGE)
                .heart((32.0, 38.0), 48.0, DARK_RED)
                .heart((32.0, 38.0), 30.0, RED)
                .circle((32.0, 36.0), 5.0, YELLOW);
        }
        GatlingSoul => {
            c.circle((32.0, 32.0), 24.0, DARK_GREY);
            for i in 0..6 {
                let a = i as f32 / 6.0 * std::f32::consts::TAU;
                c.circle((32.0 + a.cos() * 13.0, 32.0 + a.sin() * 13.0), 6.0, GREY).circle((32.0 + a.cos() * 13.0, 32.0 + a.sin() * 13.0), 3.0, INK);
            }
            c.circle((32.0, 32.0), 5.0, ORANGE);
        }
    }
    c.outline(INK, 2);
    c
}

/// Draws a main weapon (pointing to the upper right or to the right).
pub fn paint_main(kind: MainWeapon) -> Canvas {
    let mut c = Canvas::default();
    let m = DARK_GREY;
    let l = GREY;
    match kind {
        MainWeapon::FireStaff => {
            c.line((10.0, 56.0), (44.0, 22.0), 5.0, BROWN).circle((48.0, 16.0), 11.0, ORANGE).circle((48.0, 16.0), 6.0, YELLOW);
        }
        MainWeapon::LightningStaff => {
            c.line((10.0, 56.0), (42.0, 24.0), 5.0, l).poly(&[(48.0, 4.0), (58.0, 16.0), (48.0, 30.0), (38.0, 16.0)], CYAN).bolt(43.0, 8.0, 14.0, WHITE);
        }
        MainWeapon::Bow => {
            c.path(&[(18.0, 6.0), (34.0, 14.0), (42.0, 32.0), (34.0, 50.0), (18.0, 58.0)], 5.0, BROWN)
                .line((18.0, 6.0), (18.0, 58.0), 1.5, BEIGE)
                .line((8.0, 32.0), (58.0, 32.0), 2.5, BEIGE)
                .poly(&[(58.0, 32.0), (50.0, 27.0), (50.0, 37.0)], l);
        }
        MainWeapon::Revolver => {
            c.rect(20.0, 22.0, 54.0, 28.0, l).circle((24.0, 30.0), 7.0, m).poly(&[(16.0, 30.0), (24.0, 32.0), (20.0, 52.0), (10.0, 50.0)], BROWN);
        }
        MainWeapon::Shotgun => {
            c.rect(6.0, 26.0, 58.0, 31.0, m).rect(6.0, 32.0, 58.0, 36.0, m).rect(26.0, 36.0, 40.0, 42.0, BROWN).poly(&[(4.0, 30.0), (14.0, 30.0), (10.0, 46.0), (0.0, 44.0)], BROWN);
        }
        MainWeapon::Sniper => {
            c.rect(4.0, 28.0, 62.0, 32.0, m).rect(18.0, 18.0, 40.0, 24.0, l).circle((40.0, 21.0), 3.0, CYAN).rect(0.0, 28.0, 14.0, 38.0, m).rect(26.0, 32.0, 30.0, 44.0, l);
        }
        MainWeapon::Bananarang => {
            c.path(&[(10.0, 20.0), (22.0, 40.0), (40.0, 48.0), (56.0, 42.0)], 10.0, YELLOW).circle((10.0, 20.0), 4.0, BROWN).circle((56.0, 42.0), 4.0, BROWN);
        }
        MainWeapon::HeroSword => {
            c.poly(&[(52.0, 6.0), (58.0, 12.0), (24.0, 46.0), (18.0, 40.0)], rgb(0.85, 0.9, 1.0))
                .line((14.0, 36.0), (28.0, 50.0), 5.0, GOLD)
                .line((20.0, 44.0), (8.0, 56.0), 5.0, BROWN)
                .line((50.0, 10.0), (26.0, 34.0), 1.5, CYAN);
        }
    }
    c.outline(INK, 2);
    c
}

/// Draws an automatic weapon.
pub fn paint_auto(kind: AutoWeapon) -> Canvas {
    let mut c = Canvas::default();
    match kind {
        AutoWeapon::Aura => {
            c.circle((32.0, 32.0), 26.0, rgb(0.5, 0.2, 0.9)).circle((32.0, 32.0), 19.0, PURPLE).ring((32.0, 32.0), 26.0, 3.0, PINK).circle((32.0, 32.0), 7.0, WHITE);
        }
        AutoWeapon::Lightning => {
            c.ellipse((32.0, 14.0), 24.0, 10.0, GREY).bolt(18.0, 18.0, 42.0, YELLOW);
        }
        AutoWeapon::Orbs => {
            c.ring((32.0, 32.0), 20.0, 2.0, CYAN).circle((32.0, 12.0), 8.0, CYAN).circle((14.0, 42.0), 8.0, CYAN).circle((50.0, 42.0), 8.0, CYAN).circle((32.0, 32.0), 5.0, WHITE);
        }
        AutoWeapon::Axes => {
            c.line((16.0, 54.0), (44.0, 14.0), 5.0, BROWN).poly(&[(36.0, 10.0), (58.0, 14.0), (52.0, 34.0), (40.0, 22.0)], GREY);
        }
        AutoWeapon::Mines => {
            c.ellipse((32.0, 40.0), 24.0, 12.0, DARK_GREY).ellipse((32.0, 36.0), 18.0, 8.0, GREY).circle((32.0, 32.0), 5.0, RED);
        }
        AutoWeapon::Tornado => {
            c.poly(&[(10.0, 10.0), (54.0, 10.0), (36.0, 56.0), (28.0, 56.0)], rgb(0.75, 0.8, 0.85))
                .line((14.0, 20.0), (50.0, 20.0), 3.0, WHITE)
                .line((20.0, 34.0), (44.0, 34.0), 3.0, WHITE)
                .line((26.0, 46.0), (38.0, 46.0), 3.0, WHITE);
        }
        AutoWeapon::Poison => {
            c.ellipse((32.0, 48.0), 26.0, 10.0, GREEN).rounded(24.0, 6.0, 40.0, 16.0, 2.0, BROWN).circle((32.0, 30.0), 14.0, rgb(0.4, 0.9, 0.2)).circle((28.0, 26.0), 4.0, WHITE);
        }
        AutoWeapon::FireTrail => {
            c.poly(&[(14.0, 56.0), (20.0, 24.0), (28.0, 40.0), (34.0, 8.0), (42.0, 34.0), (48.0, 22.0), (52.0, 56.0)], ORANGE)
                .poly(&[(24.0, 56.0), (30.0, 36.0), (36.0, 48.0), (42.0, 56.0)], YELLOW);
        }
    }
    c.outline(INK, 2);
    c
}

/// Draws a tome: a book whose cover shows what it improves.
pub fn paint_tome(t: Tome) -> Canvas {
    let mut c = Canvas::default();
    let cover = match t {
        Tome::Damage | Tome::Precision => RED,
        Tome::Cooldown | Tome::ProjectileSpeed | Tome::Agility => BLUE,
        Tome::Quantity | Tome::Size | Tome::Duration => PURPLE,
        Tome::Health | Tome::Regen | Tome::Armor => GREEN,
        Tome::Luck | Tome::Gold => GOLD,
        Tome::Experience | Tome::Attraction => CYAN,
    };
    c.rounded(10.0, 6.0, 54.0, 58.0, 4.0, cover).rect(10.0, 6.0, 16.0, 58.0, DARK_GREY).rect(48.0, 10.0, 52.0, 56.0, BEIGE);
    let s = (32.0, 32.0);
    match t {
        Tome::Damage => {
            c.line((22.0, 44.0), (42.0, 18.0), 4.0, WHITE).line((20.0, 36.0), (30.0, 46.0), 3.0, GOLD);
        }
        Tome::Cooldown => {
            c.ring(s, 12.0, 3.0, WHITE).line(s, (32.0, 24.0), 2.5, WHITE).line(s, (38.0, 34.0), 2.5, WHITE);
        }
        Tome::Quantity => {
            c.circle((24.0, 38.0), 5.0, WHITE).circle((40.0, 38.0), 5.0, WHITE).circle((32.0, 24.0), 5.0, WHITE);
        }
        Tome::Size => {
            c.circle(s, 13.0, WHITE).circle(s, 6.0, cover);
        }
        Tome::ProjectileSpeed => {
            c.line((20.0, 24.0), (44.0, 24.0), 3.0, WHITE).line((20.0, 32.0), (44.0, 32.0), 3.0, WHITE).line((20.0, 40.0), (44.0, 40.0), 3.0, WHITE).poly(&[(44.0, 18.0), (50.0, 32.0), (44.0, 46.0)], WHITE);
        }
        Tome::Duration => {
            c.poly(&[(22.0, 18.0), (42.0, 18.0), (32.0, 32.0)], WHITE).poly(&[(32.0, 32.0), (42.0, 46.0), (22.0, 46.0)], WHITE);
        }
        Tome::Health => {
            c.heart(s, 1.0, RED);
        }
        Tome::Regen => {
            c.heart(s, 1.0, RED).rect(29.0, 22.0, 35.0, 40.0, WHITE).rect(23.0, 28.0, 41.0, 34.0, WHITE);
        }
        Tome::Armor => {
            c.poly(&[(20.0, 18.0), (44.0, 18.0), (44.0, 32.0), (32.0, 48.0), (20.0, 32.0)], GREY);
        }
        Tome::Agility => {
            c.bolt(20.0, 14.0, 34.0, YELLOW);
        }
        Tome::Luck => {
            c.circle((26.0, 28.0), 7.0, GREEN).circle((38.0, 28.0), 7.0, GREEN).circle((32.0, 38.0), 7.0, GREEN).line((32.0, 40.0), (34.0, 50.0), 2.0, DARK_GREEN);
        }
        Tome::Experience => {
            c.poly(&[(32.0, 16.0), (44.0, 30.0), (32.0, 48.0), (20.0, 30.0)], WHITE).poly(&[(32.0, 20.0), (40.0, 30.0), (32.0, 42.0), (24.0, 30.0)], CYAN);
        }
        Tome::Gold => {
            c.circle(s, 12.0, YELLOW).circle(s, 8.0, GOLD);
        }
        Tome::Attraction => {
            c.ring((32.0, 30.0), 11.0, 6.0, RED).rect(18.0, 30.0, 26.0, 44.0, RED).rect(38.0, 30.0, 46.0, 44.0, RED).rect(18.0, 40.0, 26.0, 46.0, WHITE).rect(38.0, 40.0, 46.0, 46.0, WHITE).rect(20.0, 26.0, 44.0, 32.0, cover);
        }
        Tome::Precision => {
            c.ring(s, 12.0, 2.5, WHITE).line((32.0, 14.0), (32.0, 50.0), 2.0, WHITE).line((14.0, 32.0), (50.0, 32.0), 2.0, WHITE).circle(s, 3.0, WHITE);
        }
    }
    c.outline(INK, 2);
    c
}

/// All icons as GPU images, built once at startup.
#[derive(Resource, Default)]
pub struct Icons {
    pub items: HashMap<ItemKind, Handle<Image>>,
    pub mains: HashMap<MainWeapon, Handle<Image>>,
    pub autos: HashMap<AutoWeapon, Handle<Image>>,
    pub tomes: HashMap<Tome, Handle<Image>>,
}

pub fn build_icons(mut icons: ResMut<Icons>, mut images: ResMut<Assets<Image>>) {
    for item in ItemKind::ALL {
        icons.items.insert(item, images.add(paint_item(item).to_image()));
    }
    for w in MainWeapon::ALL {
        icons.mains.insert(w, images.add(paint_main(w).to_image()));
    }
    for a in AutoWeapon::ALL {
        icons.autos.insert(a, images.add(paint_auto(a).to_image()));
    }
    for t in Tome::ALL {
        icons.tomes.insert(t, images.add(paint_tome(t).to_image()));
    }
}

pub struct IconsPlugin;

impl Plugin for IconsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Icons>().add_systems(Startup, build_icons);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_draws_something() {
        for item in ItemKind::ALL {
            let c = paint_item(item);
            let covered = c.px.iter().filter(|p| p[3] > 0.5).count();
            assert!(covered > 200, "{item:?}: only {covered} pixels");
        }
        let canvases = MainWeapon::ALL
            .into_iter()
            .map(paint_main)
            .chain(AutoWeapon::ALL.into_iter().map(paint_auto))
            .chain(Tome::ALL.into_iter().map(paint_tome));
        for (i, c) in canvases.enumerate() {
            let covered = c.px.iter().filter(|p| p[3] > 0.5).count();
            assert!(covered > 200, "icon {i}: only {covered} pixels");
        }
    }
}
