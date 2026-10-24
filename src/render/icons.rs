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

// --- Shaded painting (3 tones + gloss + drop shadow) -------------------------
//
// Shapes below are plain "inside?" closures, so they can be rotated, combined
// and shaded the same way. Light always comes from the upper left.

/// Darker tone of a color (shaded side).
fn shade_of(c: Rgba) -> Rgba {
    [c[0] * 0.55, c[1] * 0.5, c[2] * 0.6 + 0.03, c[3]]
}

/// Lighter tone of a color (lit rim).
fn light_of(c: Rgba) -> Rgba {
    [c[0] + (1.0 - c[0]) * 0.5, c[1] + (1.0 - c[1]) * 0.5, c[2] + (1.0 - c[2]) * 0.5, c[3]]
}

fn poly_inside(pts: &[Vec2], p: Vec2) -> bool {
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
}

fn sh_circle(cx: f32, cy: f32, r: f32) -> impl Fn(Vec2) -> bool + Copy {
    move |p| p.distance(Vec2::new(cx, cy)) <= r
}

fn sh_ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> impl Fn(Vec2) -> bool + Copy {
    move |p| ((p.x - cx) / rx).powi(2) + ((p.y - cy) / ry).powi(2) <= 1.0
}

fn sh_rect(x0: f32, y0: f32, x1: f32, y1: f32) -> impl Fn(Vec2) -> bool + Copy {
    move |p| p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1
}

fn sh_rounded(x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> impl Fn(Vec2) -> bool + Copy {
    move |p| p.distance(Vec2::new(p.x.clamp(x0 + r, x1 - r), p.y.clamp(y0 + r, y1 - r))) <= r
}

fn sh_line(a: (f32, f32), b: (f32, f32), w: f32) -> impl Fn(Vec2) -> bool + Copy {
    let (a, b) = (Vec2::new(a.0, a.1), Vec2::new(b.0, b.1));
    move |p| dist_to_segment(p, a, b) <= w * 0.5
}

/// Thick polyline.
fn sh_path(pts: &[(f32, f32)], w: f32) -> impl Fn(Vec2) -> bool + use<> {
    let pts: Vec<Vec2> = pts.iter().map(|p| Vec2::new(p.0, p.1)).collect();
    move |p| pts.windows(2).any(|s| dist_to_segment(p, s[0], s[1]) <= w * 0.5)
}

fn sh_poly(pts: &[(f32, f32)]) -> impl Fn(Vec2) -> bool + use<> {
    let pts: Vec<Vec2> = pts.iter().map(|p| Vec2::new(p.0, p.1)).collect();
    move |p| poly_inside(&pts, p)
}

/// Heart centered at (cx, cy), size ~ `s` (same shape as `Canvas::heart`).
fn sh_heart(cx: f32, cy: f32, s: f32) -> impl Fn(Vec2) -> bool {
    let tri = sh_poly(&[(cx - s * 0.5, cy - s * 0.05), (cx + s * 0.5, cy - s * 0.05), (cx, cy + s * 0.45)]);
    let (l, r) = (sh_circle(cx - s * 0.25, cy - s * 0.12, s * 0.27), sh_circle(cx + s * 0.25, cy - s * 0.12, s * 0.27));
    move |p| l(p) || r(p) || tri(p)
}

/// Drop (round bottom of radius `r` at (cx, cy), point upward).
fn sh_drop(cx: f32, cy: f32, r: f32) -> impl Fn(Vec2) -> bool {
    let tip = sh_poly(&[(cx - r * 0.92, cy - r * 0.4), (cx + r * 0.92, cy - r * 0.4), (cx, cy - r * 2.3)]);
    move |p| p.distance(Vec2::new(cx, cy)) <= r || tip(p)
}

/// Plus sign: arms of half-length `arm` and half-thickness `half`.
fn sh_cross(cx: f32, cy: f32, arm: f32, half: f32) -> impl Fn(Vec2) -> bool + Copy {
    let (h, v) = (sh_rounded(cx - arm, cy - half, cx + arm, cy + half, 1.5), sh_rounded(cx - half, cy - arm, cx + half, cy + arm, 1.5));
    move |p| h(p) || v(p)
}

/// Star with `n` points.
fn sh_star(cx: f32, cy: f32, r_out: f32, r_in: f32, n: usize) -> impl Fn(Vec2) -> bool {
    let pts: Vec<(f32, f32)> = (0..n * 2)
        .map(|i| {
            let a = i as f32 / (n * 2) as f32 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
            let r = if i % 2 == 0 { r_out } else { r_in };
            (cx + a.cos() * r, cy + a.sin() * r)
        })
        .collect();
    sh_poly(&pts)
}

/// `f` rotated by `angle` (radians, clockwise on screen) around `c`.
fn sh_rot(c: (f32, f32), angle: f32, f: impl Fn(Vec2) -> bool) -> impl Fn(Vec2) -> bool {
    let (c, r) = (Vec2::new(c.0, c.1), Vec2::from_angle(-angle));
    move |p| f(c + r.rotate(p - c))
}

impl Canvas {
    /// Fills a shape with 3 tones: base color, darker rim on the lower right,
    /// lighter rim on the upper left (light comes from the upper left).
    pub fn lit(&mut self, base: Rgba, inside: impl Fn(Vec2) -> bool) -> &mut Self {
        let f = &inside;
        self.fill(base, f)
            .fill(shade_of(base), move |p| f(p) && !f(p + Vec2::new(2.2, 3.0)))
            .fill(light_of(base), move |p| f(p) && !f(p - Vec2::new(1.6, 1.6)))
    }

    /// Soft white reflection (a tilted ellipse).
    pub fn gloss(&mut self, c: (f32, f32), rx: f32, ry: f32, angle: f32) -> &mut Self {
        self.fill([1.0, 1.0, 1.0, 0.6], sh_rot(c, angle, sh_ellipse(c.0, c.1, rx, ry)))
    }

    /// 4-pointed twinkle.
    pub fn sparkle(&mut self, c: (f32, f32), r: f32, color: Rgba) -> &mut Self {
        self.fill(color, sh_star(c.0, c.1, r, r * 0.28, 4))
    }

    /// Soft drop shadow under everything drawn so far, offset by (dx, dy).
    pub fn drop_shadow(&mut self, dx: i32, dy: i32, opacity: f32) -> &mut Self {
        let n = SIZE as i32;
        let alpha = |x: i32, y: i32| -> f32 { if x < 0 || y < 0 || x >= n || y >= n { 0.0 } else { self.px[y as usize * SIZE + x as usize][3] } };
        let mut shadow = vec![0.0f32; SIZE * SIZE];
        for y in 0..n {
            for x in 0..n {
                let mut sum = 0.0;
                for by in -2..=2 {
                    for bx in -2..=2 {
                        sum += alpha(x - dx + bx, y - dy + by);
                    }
                }
                shadow[y as usize * SIZE + x as usize] = sum / 25.0 * opacity;
            }
        }
        let tint = [0.02, 0.0, 0.05];
        for (dst, s) in self.px.iter_mut().zip(shadow) {
            let sa = dst[3];
            let out = sa + s * (1.0 - sa);
            if out > 0.0 {
                for c in 0..3 {
                    dst[c] = (dst[c] * sa + tint[c] * s * (1.0 - sa)) / out;
                }
                dst[3] = out;
            }
        }
        self
    }

    /// A bullet: casing + `tip` colored ogive, center `c`, pointing up and
    /// then rotated by `angle`. `w` is the half width, `len` the full length.
    fn bullet(&mut self, c: (f32, f32), w: f32, len: f32, angle: f32, tip: Rgba) -> &mut Self {
        let (top, bottom) = (c.1 - len * 0.5, c.1 + len * 0.5);
        let neck = top + len * 0.42;
        let casing = sh_rot(c, angle, sh_rounded(c.0 - w, neck - 1.0, c.0 + w, bottom, 1.5));
        let ogive = sh_rot(c, angle, move |p: Vec2| p.y <= neck && ((p.x - c.0) / w).powi(2) + ((p.y - neck) / (neck - top)).powi(2) <= 1.0);
        let rim = sh_rot(c, angle, sh_rect(c.0 - w - 0.8, bottom - len * 0.12, c.0 + w + 0.8, bottom - len * 0.06));
        let shine = sh_rot(c, angle, sh_rounded(c.0 - w * 0.6, neck + 1.5, c.0 - w * 0.25, bottom - len * 0.16, 1.0));
        self.lit(GOLD, casing).lit(tip, ogive).lit(rgb(0.6, 0.32, 0.04), rim).fill([1.0, 0.95, 0.7, 0.55], shine)
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

/// Draws one item icon (shaded shapes, dark outline, soft drop shadow).
pub fn paint_item(item: ItemKind) -> Canvas {
    use ItemKind::*;
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, TAU};
    const STEEL: Rgba = rgb(0.66, 0.7, 0.78);
    const COPPER: Rgba = rgb(0.85, 0.42, 0.14);
    const WOOD: Rgba = rgb(0.58, 0.33, 0.13);
    const LEAF: Rgba = rgb(0.45, 0.85, 0.2);
    const PAPER: Rgba = rgb(0.96, 0.88, 0.95);
    const GLASS: Rgba = [0.35, 0.8, 1.0, 0.45];
    let mut c = Canvas::default();
    match item {
        FrogLegs => {
            // A happy frog sitting on its big webbed feet.
            for x in [12.0, 52.0] {
                c.lit(GREEN, sh_ellipse(x, 53.0, 9.0, 4.5));
                for dx in [-6.0, 0.0, 6.0] {
                    c.lit(GREEN, sh_circle(x + dx, 55.5, 2.8));
                }
            }
            c.lit(GREEN, sh_ellipse(32.0, 38.0, 23.0, 16.0))
                .fill(LEAF, sh_ellipse(32.0, 44.0, 14.0, 8.0))
                .lit(GREEN, sh_circle(19.0, 21.0, 10.0))
                .lit(GREEN, sh_circle(45.0, 21.0, 10.0))
                .lit(WHITE, sh_circle(19.0, 20.0, 6.5))
                .lit(WHITE, sh_circle(45.0, 20.0, 6.5))
                .circle((20.0, 21.0), 3.5, INK)
                .circle((46.0, 21.0), 3.5, INK)
                .circle((18.8, 19.6), 1.3, WHITE)
                .circle((44.8, 19.6), 1.3, WHITE)
                .path(&[(18.0, 36.0), (25.0, 41.0), (39.0, 41.0), (46.0, 36.0)], 2.5, DARK_GREEN)
                .ellipse((13.0, 33.0), 3.5, 2.2, [1.0, 0.45, 0.65, 0.8])
                .ellipse((51.0, 33.0), 3.5, 2.2, [1.0, 0.45, 0.65, 0.8]);
        }
        Jetpack => {
            // Twin tanks spitting flames.
            for x in [22.0, 42.0] {
                c.lit(ORANGE, sh_poly(&[(x - 7.0, 44.0), (x + 7.0, 44.0), (x + 4.0, 52.0), (x, 61.0), (x - 4.0, 52.0)]))
                    .fill(YELLOW, sh_poly(&[(x - 4.0, 44.0), (x + 4.0, 44.0), (x, 55.0)]))
                    .fill(WHITE, sh_ellipse(x, 46.0, 2.0, 3.0))
                    .lit(DARK_GREY, sh_rounded(x - 5.5, 40.0, x + 5.5, 46.0, 1.0));
            }
            c.lit(rgb(0.3, 0.32, 0.4), sh_rounded(26.0, 12.0, 38.0, 38.0, 3.0));
            for x in [22.0, 42.0] {
                c.lit(STEEL, sh_rounded(x - 8.0, 6.0, x + 8.0, 42.0, 7.5))
                    .lit(RED, sh_rect(x - 8.0, 14.0, x + 8.0, 19.0))
                    .lit(RED, sh_rect(x - 8.0, 31.0, x + 8.0, 35.0))
                    .gloss((x - 3.5, 25.0), 1.6, 4.5, 0.0);
            }
        }
        SwiftBoots => {
            // A red sneaker with a golden wing on the heel.
            c.lit(GOLD, sh_poly(&[(20.0, 14.0), (5.0, 8.0), (9.0, 15.0), (3.0, 18.0), (9.0, 23.0), (4.0, 27.0), (11.0, 31.0), (20.0, 30.0)]))
                .line((7.0, 16.0), (19.0, 19.0), 1.4, rgb(0.6, 0.32, 0.03))
                .line((7.0, 25.0), (19.0, 26.0), 1.4, rgb(0.6, 0.32, 0.03))
                .lit(RED, sh_poly(&[(18.0, 8.0), (36.0, 8.0), (36.0, 30.0), (50.0, 36.0), (58.0, 44.0), (58.0, 50.0), (16.0, 50.0), (16.0, 22.0)]))
                .lit(WHITE, sh_rounded(16.0, 6.0, 38.0, 12.0, 2.5))
                .line((24.0, 44.0), (34.0, 26.0), 3.0, WHITE)
                .line((30.0, 44.0), (40.0, 32.0), 3.0, WHITE)
                .lit(WHITE, sh_rounded(14.0, 48.0, 60.0, 56.0, 3.0))
                .line((18.0, 53.0), (56.0, 53.0), 1.5, GREY)
                .gloss((22.0, 22.0), 1.6, 6.0, 0.0);
        }
        Thruster => {
            // A rocket nozzle firing downward.
            c.fill(BLUE, sh_poly(&[(14.0, 42.0), (50.0, 42.0), (42.0, 52.0), (32.0, 62.0), (22.0, 52.0)]))
                .fill(CYAN, sh_poly(&[(20.0, 42.0), (44.0, 42.0), (32.0, 57.0)]))
                .fill(WHITE, sh_ellipse(32.0, 45.0, 6.0, 3.5))
                .lit(STEEL, sh_poly(&[(22.0, 20.0), (42.0, 20.0), (51.0, 41.0), (13.0, 41.0)]))
                .lit(DARK_GREY, sh_rounded(11.0, 38.0, 53.0, 44.0, 2.0))
                .lit(rgb(0.4, 0.43, 0.5), sh_rounded(16.0, 4.0, 48.0, 22.0, 4.0))
                .lit(ORANGE, sh_rect(16.0, 11.0, 48.0, 15.0))
                .gloss((24.0, 30.0), 1.5, 6.0, 0.4);
        }
        DashCoil => {
            // A copper spring charged with a lightning bolt.
            c.lit(DARK_GREY, sh_rounded(4.0, 22.0, 10.0, 46.0, 2.0)).lit(DARK_GREY, sh_rounded(54.0, 22.0, 60.0, 46.0, 2.0));
            let pts: Vec<(f32, f32)> = (0..9).map(|i| (9.0 + i as f32 * 5.75, if i % 2 == 0 { 42.0 } else { 26.0 })).collect();
            c.lit(COPPER, sh_path(&pts, 5.0));
            c.lit(YELLOW, sh_poly(&[(30.0, 4.0), (44.0, 4.0), (36.0, 24.0), (46.0, 24.0), (24.0, 60.0), (30.0, 34.0), (20.0, 34.0)]))
                .fill(WHITE, sh_poly(&[(32.0, 7.0), (38.0, 7.0), (33.0, 22.0)]));
        }
        SlideWax => {
            // A tin of wax, a slick drop and a glide streak.
            c.line((4.0, 52.0), (18.0, 52.0), 3.0, CYAN)
                .line((8.0, 44.0), (18.0, 44.0), 3.0, CYAN)
                .lit(BLUE, sh_ellipse(38.0, 50.0, 18.0, 6.0))
                .lit(BLUE, sh_rect(20.0, 34.0, 56.0, 50.0))
                .lit(YELLOW, sh_rect(20.0, 39.0, 56.0, 44.0))
                .lit(rgb(0.55, 0.85, 1.0), sh_ellipse(38.0, 34.0, 18.0, 6.0))
                .fill(rgb(0.85, 0.97, 1.0), sh_ellipse(38.0, 34.0, 13.0, 3.8))
                .lit(CYAN, sh_drop(38.0, 19.0, 7.0))
                .gloss((35.5, 18.0), 1.6, 3.0, 0.3);
        }
        FeatherCape => {
            // A flowing purple cape with a feather emblem.
            c.lit(PURPLE, sh_poly(&[(22.0, 9.0), (42.0, 9.0), (54.0, 52.0), (46.0, 48.0), (39.0, 56.0), (32.0, 50.0), (25.0, 56.0), (18.0, 48.0), (10.0, 52.0)]))
                .fill(rgb(0.35, 0.08, 0.6), sh_poly(&[(30.0, 14.0), (34.0, 14.0), (36.0, 50.0), (32.0, 50.0)]))
                .lit(WHITE, sh_rot((32.0, 33.0), 0.5, sh_ellipse(32.0, 33.0, 6.0, 15.0)))
                .line((26.0, 46.0), (39.0, 20.0), 1.5, GREY)
                .lit(GOLD, sh_rounded(18.0, 5.0, 46.0, 12.0, 3.5))
                .lit(YELLOW, sh_circle(32.0, 9.0, 4.0));
        }
        PhaseStep => {
            // A ghost dashing, with fading after-images.
            let ghost = |ox: f32| {
                let head = sh_circle(34.0 + ox, 24.0, 15.0);
                let body = sh_rect(19.0 + ox, 24.0, 49.0, 48.0);
                let tail = sh_poly(&[(19.0 + ox, 47.0), (19.0 + ox, 56.0), (24.0 + ox, 51.0), (29.0 + ox, 57.0), (34.0 + ox, 51.0), (39.0 + ox, 57.0), (44.0 + ox, 51.0), (49.0 + ox, 56.0), (49.0 + ox, 47.0)]);
                move |p: Vec2| head(p) || (body(p) && p.x <= 49.0 + ox) || tail(p)
            };
            c.fill([0.5, 0.8, 1.0, 0.25], ghost(-15.0))
                .fill([0.6, 0.85, 1.0, 0.45], ghost(-8.0))
                .lit(rgb(0.86, 0.93, 1.0), ghost(0.0))
                .ellipse((29.0, 26.0), 3.0, 4.5, INK)
                .ellipse((40.0, 26.0), 3.0, 4.5, INK)
                .circle((28.2, 24.5), 1.1, WHITE)
                .circle((39.2, 24.5), 1.1, WHITE)
                .ellipse((34.5, 36.0), 3.0, 3.5, INK);
        }
        PowerRounds => {
            // Three heavy cartridges.
            c.bullet((14.0, 34.0), 6.5, 42.0, 0.0, COPPER).bullet((50.0, 34.0), 6.5, 42.0, 0.0, COPPER).bullet((32.0, 30.0), 7.5, 48.0, 0.0, RED);
        }
        HairTrigger => {
            // A trigger and its guard, with "quick" lines.
            c.lit(DARK_GREY, sh_rounded(4.0, 8.0, 60.0, 20.0, 3.0))
                .lit(GREY, sh_rounded(4.0, 8.0, 60.0, 12.0, 2.0))
                .lit(GREY, move |p: Vec2| {
                    let d = p.distance(Vec2::new(34.0, 22.0));
                    (14.0..=19.0).contains(&d) && p.y >= 18.0
                })
                .lit(RED, sh_poly(&[(30.0, 19.0), (37.0, 19.0), (37.0, 27.0), (34.0, 36.0), (28.0, 40.0), (26.0, 38.0), (30.0, 30.0)]))
                .path(&[(6.0, 28.0), (14.0, 30.0)], 2.5, YELLOW)
                .path(&[(4.0, 36.0), (13.0, 36.0)], 2.5, YELLOW)
                .path(&[(6.0, 44.0), (14.0, 42.0)], 2.5, YELLOW)
                .sparkle((24.0, 47.0), 6.0, YELLOW);
        }
        CritLens => {
            // A scope reticle with a critical twinkle.
            c.lit(rgb(0.25, 0.27, 0.34), sh_circle(29.0, 29.0, 24.0))
                .lit(rgb(0.2, 0.55, 0.8), sh_circle(29.0, 29.0, 18.0))
                .fill(rgb(0.4, 0.8, 1.0), sh_circle(26.0, 26.0, 12.0))
                .rect(28.0, 11.0, 30.0, 23.0, INK)
                .rect(28.0, 35.0, 30.0, 47.0, INK)
                .rect(11.0, 28.0, 23.0, 30.0, INK)
                .rect(35.0, 28.0, 47.0, 30.0, INK)
                .ring((29.0, 29.0), 6.0, 1.5, RED)
                .circle((29.0, 29.0), 2.0, RED)
                .gloss((21.0, 19.0), 3.0, 6.0, 0.8)
                .lit(YELLOW, sh_star(50.0, 50.0, 10.0, 3.0, 4))
                .circle((50.0, 50.0), 2.0, WHITE);
        }
        Executioner => {
            // A big executioner's axe.
            c.lit(WOOD, sh_line((12.0, 58.0), (40.0, 8.0), 6.0));
            for t in [0.62, 0.72, 0.82] {
                let p = (12.0 + 28.0 * t, 58.0 - 50.0 * t);
                c.line((p.0 - 3.0, p.1 - 1.5), (p.0 + 3.0, p.1 + 1.5), 1.5, BROWN);
            }
            c.lit(STEEL, sh_poly(&[(30.0, 14.0), (44.0, 4.0), (56.0, 8.0), (60.0, 24.0), (54.0, 40.0), (46.0, 38.0), (36.0, 28.0)]))
                .path(&[(46.0, 5.0), (56.0, 9.0), (59.5, 24.0), (54.0, 39.0)], 2.2, WHITE)
                .lit(DARK_GREY, sh_poly(&[(31.0, 14.0), (24.0, 10.0), (28.0, 21.0), (34.0, 22.0)]))
                .circle((37.0, 18.0), 2.0, DARK_GREY)
                .lit(RED, sh_drop(53.0, 46.0, 3.0));
        }
        UnstableRounds => {
            // A cracked bomb with a lit fuse.
            c.lit(rgb(0.24, 0.24, 0.32), sh_circle(28.0, 38.0, 20.0))
                .lit(GREY, sh_rot((41.0, 22.0), FRAC_PI_4, sh_rounded(35.0, 18.0, 47.0, 26.0, 2.0)))
                .path(&[(24.0, 54.0), (28.0, 46.0), (24.0, 40.0), (30.0, 34.0)], 2.0, ORANGE)
                .path(&[(28.0, 46.0), (36.0, 44.0)], 1.6, ORANGE)
                .gloss((19.0, 30.0), 3.5, 6.0, 0.7)
                .path(&[(44.0, 19.0), (47.0, 12.0), (52.0, 10.0)], 3.0, BEIGE)
                .fill(ORANGE, sh_star(52.0, 10.0, 9.0, 4.0, 8))
                .fill(YELLOW, sh_star(52.0, 10.0, 5.5, 2.5, 8))
                .circle((52.0, 10.0), 1.8, WHITE);
        }
        TeslaCoil => {
            // A Tesla tower throwing arcs.
            let arc_l = [(20.0, 16.0), (13.0, 14.0), (15.0, 8.0), (5.0, 4.0)];
            let arc_r = [(44.0, 16.0), (51.0, 18.0), (49.0, 10.0), (59.0, 6.0)];
            c.path(&arc_l, 4.0, CYAN).path(&arc_l, 1.5, WHITE).path(&arc_r, 4.0, CYAN).path(&arc_r, 1.5, WHITE);
            c.lit(DARK_GREY, sh_rounded(16.0, 52.0, 48.0, 59.0, 2.0)).lit(GREY, sh_rect(27.0, 24.0, 37.0, 53.0));
            for y in [30.0, 35.5, 41.0, 46.5] {
                c.lit(COPPER, sh_rounded(22.0, y - 2.0, 42.0, y + 2.0, 2.0));
            }
            c.lit(STEEL, sh_ellipse(32.0, 19.0, 15.0, 9.0)).gloss((26.0, 15.5), 4.0, 2.0, 0.0);
        }
        HollowPoint => {
            // A hollow-point bullet flying into a target.
            c.lit(WHITE, sh_circle(44.0, 20.0, 15.0))
                .lit(RED, sh_circle(44.0, 20.0, 11.5))
                .fill(WHITE, sh_circle(44.0, 20.0, 7.5))
                .fill(RED, sh_circle(44.0, 20.0, 3.5));
            c.bullet((24.0, 40.0), 7.0, 34.0, FRAC_PI_4, COPPER)
                .fill(rgb(0.25, 0.08, 0.02), sh_rot((24.0, 40.0), FRAC_PI_4, sh_ellipse(24.0, 26.0, 3.2, 2.0)));
            c.line((6.0, 46.0), (12.0, 52.0), 2.0, CYAN).line((10.0, 40.0), (16.0, 46.0), 2.0, CYAN);
        }
        Berserker => {
            // A horned war mask with glowing eyes.
            c.lit(BEIGE, sh_poly(&[(17.0, 22.0), (6.0, 14.0), (3.0, 2.0), (12.0, 10.0), (25.0, 15.0)]))
                .lit(BEIGE, sh_poly(&[(47.0, 22.0), (58.0, 14.0), (61.0, 2.0), (52.0, 10.0), (39.0, 15.0)]))
                .lit(RED, sh_poly(&[(12.0, 16.0), (52.0, 16.0), (55.0, 34.0), (45.0, 52.0), (32.0, 58.0), (19.0, 52.0), (9.0, 34.0)]))
                .fill(DARK_RED, sh_poly(&[(30.0, 16.0), (34.0, 16.0), (33.0, 30.0), (31.0, 30.0)]))
                .fill(INK, sh_poly(&[(15.0, 27.0), (30.0, 33.0), (28.0, 39.0), (16.0, 36.0)]))
                .fill(INK, sh_poly(&[(49.0, 27.0), (34.0, 33.0), (36.0, 39.0), (48.0, 36.0)]))
                .fill(YELLOW, sh_poly(&[(18.0, 30.0), (28.0, 34.5), (27.0, 37.0), (19.0, 35.0)]))
                .fill(YELLOW, sh_poly(&[(46.0, 30.0), (36.0, 34.5), (37.0, 37.0), (45.0, 35.0)]))
                .fill(INK, sh_rounded(21.0, 42.0, 43.0, 50.0, 2.0));
            for i in 0..5 {
                let x = 23.0 + i as f32 * 4.5;
                c.poly(&[(x - 1.8, 42.0), (x + 1.8, 42.0), (x, 46.0)], WHITE).poly(&[(x - 1.8, 50.0), (x + 1.8, 50.0), (x, 46.5)], WHITE);
            }
        }
        Bloodlust => {
            // A blood drop rushing forward.
            c.line((4.0, 34.0), (18.0, 34.0), 3.0, ORANGE)
                .line((2.0, 43.0), (18.0, 43.0), 3.0, ORANGE)
                .line((6.0, 52.0), (20.0, 52.0), 3.0, ORANGE)
                .lit(RED, sh_rot((38.0, 40.0), 0.35, sh_drop(38.0, 40.0, 16.0)))
                .gloss((31.0, 36.0), 3.0, 6.0, 0.4)
                .circle((44.0, 48.0), 2.0, [1.0, 0.6, 0.6, 0.7]);
        }
        VitalHeart => {
            // A glossy heart with a plus.
            c.lit(RED, sh_heart(32.0, 33.0, 54.0)).gloss((19.0, 21.0), 4.0, 6.0, 0.8).lit(WHITE, sh_cross(34.0, 34.0, 9.0, 3.2));
        }
        RegenMoss => {
            // A mossy stone sprouting, with a healing plus.
            c.lit(GREY, sh_ellipse(32.0, 48.0, 25.0, 10.0));
            for (x, y, r) in [(14.0, 42.0, 8.0), (50.0, 43.0, 8.0), (24.0, 36.0, 11.0), (40.0, 37.0, 11.0), (32.0, 32.0, 9.0)] {
                c.lit(GREEN, sh_circle(x, y, r));
            }
            for (x, y) in [(20.0, 34.0), (36.0, 31.0), (44.0, 38.0), (28.0, 41.0)] {
                c.circle((x, y), 1.8, LEAF);
            }
            c.path(&[(32.0, 24.0), (32.0, 16.0)], 2.5, DARK_GREEN)
                .lit(LEAF, sh_rot((26.0, 14.0), -0.5, sh_ellipse(26.0, 14.0, 6.0, 3.5)))
                .lit(LEAF, sh_rot((38.0, 12.0), 0.5, sh_ellipse(38.0, 12.0, 6.0, 3.5)))
                .lit(rgb(0.55, 1.0, 0.45), sh_cross(52.0, 14.0, 7.0, 2.6));
        }
        ArmorPlate => {
            // A steel cuirass.
            c.lit(STEEL, sh_poly(&[(10.0, 12.0), (22.0, 6.0), (26.0, 14.0), (38.0, 14.0), (42.0, 6.0), (54.0, 12.0), (56.0, 30.0), (50.0, 52.0), (32.0, 59.0), (14.0, 52.0), (8.0, 30.0)]))
                .fill(light_of(STEEL), sh_poly(&[(30.0, 16.0), (34.0, 16.0), (34.0, 56.0), (30.0, 56.0)]))
                .path(&[(14.0, 36.0), (32.0, 40.0), (50.0, 36.0)], 2.0, rgb(0.35, 0.37, 0.45))
                .path(&[(16.0, 46.0), (32.0, 50.0), (48.0, 46.0)], 2.0, rgb(0.35, 0.37, 0.45))
                .path(&[(10.0, 13.0), (22.0, 7.0)], 2.0, GOLD)
                .path(&[(42.0, 7.0), (54.0, 13.0)], 2.0, GOLD);
            for (x, y) in [(16.0, 20.0), (48.0, 20.0), (20.0, 30.0), (44.0, 30.0)] {
                c.circle((x, y), 2.0, DARK_GREY).circle((x - 0.6, y - 0.6), 0.8, WHITE);
            }
            c.gloss((20.0, 24.0), 2.0, 7.0, 0.4);
        }
        VampireFangs => {
            // Open mouth with two long fangs and a drop of blood.
            c.lit(rgb(0.7, 0.06, 0.12), sh_ellipse(32.0, 30.0, 27.0, 18.0))
                .fill(rgb(0.18, 0.02, 0.06), sh_ellipse(32.0, 30.0, 20.0, 10.5))
                .fill(rgb(0.6, 0.1, 0.2), sh_ellipse(32.0, 37.0, 12.0, 3.5))
                .lit(WHITE, move |p: Vec2| sh_ellipse(32.0, 30.0, 20.0, 10.5)(p) && p.y <= 24.0)
                .lit(WHITE, sh_poly(&[(15.0, 21.0), (24.0, 21.0), (20.5, 46.0)]))
                .lit(WHITE, sh_poly(&[(40.0, 21.0), (49.0, 21.0), (43.5, 46.0)]))
                .lit(RED, sh_drop(20.5, 55.0, 3.5));
        }
        EnergyShield => {
            // A shield made of energy, hexagon core.
            let outer = sh_poly(&[(10.0, 8.0), (54.0, 8.0), (54.0, 30.0), (32.0, 59.0), (10.0, 30.0)]);
            let inner = sh_poly(&[(15.0, 13.0), (49.0, 13.0), (49.0, 29.0), (32.0, 52.0), (15.0, 29.0)]);
            c.fill(GLASS, &inner).lit(CYAN, move |p: Vec2| outer(p) && !inner(p));
            let hex = |r: f32| {
                let pts: Vec<(f32, f32)> = (0..6).map(|i| (32.0 + (i as f32 * TAU / 6.0).cos() * r, 28.0 + (i as f32 * TAU / 6.0).sin() * r)).collect();
                sh_poly(&pts)
            };
            let (h_out, h_in) = (hex(9.5), hex(6.0));
            c.fill(WHITE, move |p: Vec2| h_out(p) && !h_in(p)).fill([0.7, 0.95, 1.0, 0.8], hex(4.0)).gloss((21.0, 18.0), 2.0, 5.0, 0.5);
        }
        Thorns => {
            // A bramble ring with sharp thorns.
            for i in 0..8 {
                let a = i as f32 / 8.0 * TAU + 0.2;
                let (s, co) = a.sin_cos();
                let tip = 27.0;
                c.lit(LEAF, sh_poly(&[(32.0 + co * 16.0 - s * 5.0, 32.0 + s * 16.0 + co * 5.0), (32.0 + co * 16.0 + s * 5.0, 32.0 + s * 16.0 - co * 5.0), (32.0 + co * tip + s * 3.0, 32.0 + s * tip - co * 3.0)]));
            }
            c.lit(rgb(0.25, 0.5, 0.12), move |p: Vec2| (p.distance(Vec2::splat(32.0)) - 17.0).abs() <= 4.5);
            for i in 0..6 {
                let a = i as f32 / 6.0 * TAU;
                let (s, co) = a.sin_cos();
                c.line((32.0 + co * 13.5, 32.0 + s * 13.5), (32.0 + (a + 0.5).cos() * 20.5, 32.0 + (a + 0.5).sin() * 20.5), 1.5, DARK_GREEN);
            }
            c.lit(RED, sh_circle(46.0, 46.0, 3.0)).lit(RED, sh_circle(16.0, 20.0, 2.5));
        }
        Ankh => {
            // A golden ankh with a gem.
            let (outer, hole) = (sh_ellipse(32.0, 17.0, 11.0, 12.5), sh_ellipse(32.0, 17.0, 5.0, 6.5));
            c.lit(GOLD, sh_poly(&[(28.5, 33.0), (35.5, 33.0), (40.0, 59.0), (24.0, 59.0)]))
                .lit(GOLD, sh_poly(&[(9.0, 26.0), (55.0, 26.0), (52.0, 31.0), (55.0, 36.0), (9.0, 36.0), (12.0, 31.0)]))
                .lit(GOLD, move |p: Vec2| outer(p) && !hole(p))
                .lit(CYAN, sh_rot((32.0, 31.0), FRAC_PI_4, sh_rect(28.5, 27.5, 35.5, 34.5)))
                .circle((30.5, 29.5), 1.0, WHITE);
        }
        LuckyCoin => {
            // A thick gold coin with a star and a twinkle.
            c.lit(rgb(0.7, 0.38, 0.03), sh_circle(33.5, 34.5, 24.0))
                .lit(GOLD, sh_circle(31.0, 32.0, 23.0))
                .ring((31.0, 32.0), 18.0, 1.6, rgb(0.6, 0.32, 0.03))
                .lit(YELLOW, sh_star(31.0, 32.5, 13.0, 5.5, 5))
                .gloss((21.0, 21.0), 3.0, 6.0, 0.8)
                .sparkle((52.0, 10.0), 8.0, WHITE);
        }
        ExtendedMag => {
            // One huge cartridge with "bigger" arrows.
            c.bullet((32.0, 32.0), 11.0, 54.0, 0.0, COPPER);
            for side in [-1.0f32, 1.0] {
                let x = 32.0 + side * 18.0;
                c.lit(CYAN, sh_poly(&[(x, 25.0), (x + side * 10.0, 32.0), (x, 39.0), (x, 35.0), (x - side * 2.0, 35.0), (x - side * 2.0, 29.0), (x, 29.0)]));
            }
        }
        QuickHands => {
            // A feather carried by the wind.
            let vane = sh_rot((30.0, 30.0), FRAC_PI_4, move |p: Vec2| {
                let t = ((p.y - 6.0) / 44.0).clamp(0.0, 1.0);
                p.y >= 6.0 && p.y <= 46.0 && (p.x - 30.0).abs() <= 10.0 * (t * std::f32::consts::PI).sin().max(0.0).sqrt() * (1.0 - t * 0.3)
            });
            c.lit(rgb(0.85, 0.95, 1.0), vane);
            c.line((12.0, 52.0), (45.0, 15.0), 2.0, GREY)
                .path(&[(34.0, 46.0), (52.0, 46.0), (57.0, 42.0), (54.0, 38.0)], 2.8, CYAN)
                .line((40.0, 54.0), (60.0, 54.0), 2.8, CYAN)
                .path(&[(4.0, 24.0), (16.0, 24.0)], 2.8, CYAN);
        }
        TreasureMap => {
            // An old parchment with a dotted path and a red X.
            c.lit(BEIGE, sh_poly(&[(10.0, 12.0), (54.0, 8.0), (54.0, 52.0), (10.0, 56.0)]))
                .lit(rgb(0.78, 0.6, 0.35), sh_rounded(6.0, 9.0, 13.0, 59.0, 3.5))
                .lit(rgb(0.78, 0.6, 0.35), sh_rounded(51.0, 5.0, 58.0, 55.0, 3.5))
                .fill(rgb(0.45, 0.75, 0.9), sh_ellipse(22.0, 22.0, 5.0, 3.5))
                .fill(rgb(0.55, 0.7, 0.35), sh_ellipse(36.0, 44.0, 8.0, 4.0));
            for (x, y) in [(18.0, 48.0), (22.0, 43.0), (27.0, 40.0), (31.0, 36.0), (34.0, 31.0)] {
                c.circle((x, y), 1.5, BROWN);
            }
            c.line((36.0, 16.0), (46.0, 26.0), 4.0, RED).line((46.0, 16.0), (36.0, 26.0), 4.0, RED);
        }
        Clover => {
            // A four-leaf clover with heart-shaped leaves.
            c.lit(DARK_GREEN, sh_path(&[(32.0, 32.0), (36.0, 46.0), (46.0, 58.0)], 4.0));
            for k in 0..4 {
                let a = k as f32 * FRAC_PI_2;
                c.lit(GREEN, sh_rot((32.0, 30.0), a, sh_heart(32.0, 17.5, 25.0)));
            }
            for k in 0..4 {
                let a = k as f32 * FRAC_PI_2;
                c.fill([0.75, 1.0, 0.5, 0.7], sh_rot((32.0, 30.0), a, sh_line((32.0, 28.0), (32.0, 16.0), 1.4)));
            }
            c.circle((32.0, 30.0), 2.5, DARK_GREEN).sparkle((52.0, 10.0), 7.0, YELLOW);
        }
        Medkit => {
            // A first aid case.
            c.lit(DARK_GREY, move |p: Vec2| sh_rounded(21.0, 5.0, 43.0, 18.0, 4.0)(p) && !sh_rounded(26.0, 10.0, 38.0, 18.0, 1.0)(p))
                .lit(rgb(0.94, 0.94, 0.97), sh_rounded(5.0, 14.0, 59.0, 55.0, 6.0))
                .rect(5.0, 22.0, 59.0, 23.5, rgb(0.7, 0.72, 0.78))
                .lit(GREY, sh_rounded(12.0, 19.0, 18.0, 26.0, 1.0))
                .lit(GREY, sh_rounded(46.0, 19.0, 52.0, 26.0, 1.0))
                .lit(RED, sh_cross(32.0, 38.0, 13.0, 5.0));
        }
        Crown => {
            // A golden crown with gems.
            c.lit(GOLD, sh_poly(&[(8.0, 50.0), (7.0, 18.0), (20.0, 32.0), (32.0, 12.0), (44.0, 32.0), (57.0, 18.0), (56.0, 50.0)]));
            for (x, y) in [(7.0, 16.0), (32.0, 10.0), (57.0, 16.0)] {
                c.lit(YELLOW, sh_circle(x, y, 4.0));
            }
            c.lit(rgb(0.85, 0.48, 0.04), sh_rounded(6.0, 42.0, 58.0, 54.0, 2.0))
                .lit(RED, sh_rot((32.0, 48.0), FRAC_PI_4, sh_rect(28.0, 44.0, 36.0, 52.0)))
                .lit(BLUE, sh_circle(17.0, 48.0, 3.5))
                .lit(GREEN, sh_circle(47.0, 48.0, 3.5))
                .gloss((15.0, 32.0), 1.8, 6.0, 0.0)
                .circle((30.0, 46.0), 1.0, WHITE);
        }
        DragonHeart => {
            // A dark heart with a burning core and a crown of fire.
            c.lit(ORANGE, sh_poly(&[(14.0, 30.0), (10.0, 14.0), (19.0, 20.0), (22.0, 4.0), (31.0, 16.0), (38.0, 2.0), (42.0, 18.0), (52.0, 10.0), (50.0, 30.0)]))
                .fill(YELLOW, sh_poly(&[(20.0, 28.0), (24.0, 14.0), (31.0, 22.0), (38.0, 10.0), (44.0, 28.0)]))
                .lit(rgb(0.5, 0.04, 0.08), sh_heart(32.0, 37.0, 50.0))
                .lit(RED, sh_heart(32.0, 37.0, 32.0))
                .fill(ORANGE, sh_heart(32.0, 37.0, 17.0))
                .fill(YELLOW, sh_circle(32.0, 36.0, 3.5));
            for (x, y) in [(15.0, 32.0), (49.0, 32.0), (24.0, 50.0), (40.0, 50.0)] {
                c.ring((x, y), 3.0, 1.2, rgb(0.3, 0.02, 0.05));
            }
        }
        GatlingSoul => {
            // Gatling barrels seen from the front, wrapped in a purple soul flame.
            c.lit(PURPLE, sh_poly(&[(10.0, 46.0), (5.0, 22.0), (15.0, 28.0), (18.0, 8.0), (28.0, 20.0), (33.0, 2.0), (40.0, 18.0), (49.0, 7.0), (51.0, 27.0), (59.0, 21.0), (55.0, 46.0)]))
                .fill(PINK, sh_poly(&[(16.0, 40.0), (14.0, 28.0), (21.0, 32.0), (24.0, 18.0), (32.0, 28.0), (36.0, 14.0), (42.0, 28.0), (47.0, 20.0), (49.0, 40.0)]))
                .lit(rgb(0.25, 0.26, 0.33), sh_circle(32.0, 39.0, 19.0));
            for i in 0..6 {
                let a = i as f32 / 6.0 * TAU;
                let (x, y) = (32.0 + a.cos() * 11.0, 39.0 + a.sin() * 11.0);
                c.lit(STEEL, sh_circle(x, y, 5.5)).circle((x + 0.3, y + 0.5), 2.6, INK);
            }
            c.lit(PINK, sh_circle(32.0, 39.0, 4.0)).circle((31.0, 38.0), 1.3, WHITE);
        }
    }
    c.outline(INK, 2).drop_shadow(1, 2, 0.55);
    c
}

/// Draws a main weapon: a bare, chunky object pointing to the upper right
/// (automatic weapons, by contrast, sit on a round badge).
pub fn paint_main(kind: MainWeapon) -> Canvas {
    let mut c = Canvas::default();
    // Guns are drawn facing right, then tilted up.
    let tilt = |s: ShapeFn| shp_turn(s, (32.0, 32.0), -0.36);
    match kind {
        MainWeapon::FireStaff => {
            c.soft_glow((45.0, 19.0), 21.0, with_alpha(ORANGE, 0.6))
                .inked(WOOD, &shp_capsule((9.0, 57.0), (39.0, 28.0), 6.0))
                .inked(WOOD_DARK, &shp_capsule((11.0, 55.0), (18.0, 48.0), 8.0))
                .inked(BRASS, &shp_capsule((35.5, 31.5), (40.0, 27.0), 10.0))
                .inked(ORANGE, &shp_flame((45.0, 32.0), 11.0, 30.0, 2.0))
                .shaded(YELLOW, &shp_flame((45.0, 29.0), 6.0, 17.0, 1.0))
                .ellipse((41.5, 21.0), 1.8, 3.0, with_alpha(WHITE, 0.85));
        }
        MainWeapon::LightningStaff => {
            c.soft_glow((46.0, 17.0), 20.0, with_alpha(CYAN, 0.6))
                .inked(STEEL, &shp_capsule((9.0, 57.0), (38.0, 28.0), 5.5))
                .inked(GUNMETAL, &shp_capsule((11.0, 55.0), (18.0, 48.0), 8.0))
                .inked(BRASS, &shp_poly(&[(31.0, 30.0), (37.0, 22.0), (42.0, 27.0), (34.0, 34.0)]))
                .inked(CYAN, &shp_poly(&[(37.0, 26.0), (39.0, 11.0), (55.0, 5.0), (53.0, 21.0)]))
                .shaded(lighter(CYAN, 0.6), &shp_poly(&[(39.0, 11.0), (55.0, 5.0), (45.0, 16.0)]))
                .shaded(WHITE, &shp_bolt(41.0, 9.0, 14.0));
        }
        MainWeapon::Bow => {
            let bow = |s: ShapeFn| shp_turn(s, (32.0, 32.0), -0.785);
            let limb = [(22.0, 7.0), (32.0, 11.0), (40.0, 21.0), (42.0, 32.0), (40.0, 43.0), (32.0, 53.0), (22.0, 57.0)];
            c.shaded(BEIGE, &bow(shp_capsule((22.0, 7.0), (22.0, 57.0), 1.6)))
                .inked(WOOD, &bow(shp_path(&limb, 6.0)))
                .inked(BRASS, &bow(shp_circle((22.0, 7.0), 3.0)))
                .inked(BRASS, &bow(shp_circle((22.0, 57.0), 3.0)))
                .inked(WOOD_DARK, &bow(shp_capsule((42.0, 27.0), (42.0, 37.0), 8.0)))
                .inked(BEIGE, &bow(shp_capsule((14.0, 32.0), (52.0, 32.0), 2.6)))
                .inked(RED, &bow(shp_poly(&[(8.0, 25.0), (16.0, 25.0), (21.0, 32.0), (13.0, 32.0)])))
                .inked(RED, &bow(shp_poly(&[(8.0, 39.0), (16.0, 39.0), (21.0, 32.0), (13.0, 32.0)])))
                .inked(STEEL, &bow(shp_poly(&[(49.0, 26.0), (61.0, 32.0), (49.0, 38.0)])));
        }
        MainWeapon::Revolver => {
            c.inked(GUNMETAL, &tilt(shp_ring((28.0, 36.0), 5.0, 2.6)))
                .inked(STEEL, &tilt(shp_rounded(28.0, 17.0, 59.0, 25.0, 2.0)))
                .inked(GUNMETAL, &tilt(shp_rounded(30.0, 24.0, 50.0, 29.0, 1.5)))
                .inked(GUNMETAL, &tilt(shp_rect(53.0, 13.0, 57.0, 18.0)))
                .inked(GUNMETAL, &tilt(shp_poly(&[(11.0, 19.0), (16.0, 11.0), (21.0, 13.0), (20.0, 21.0)])))
                .inked(GUNMETAL, &tilt(shp_rounded(14.0, 15.0, 31.0, 32.0, 3.0)))
                .inked(STEEL, &tilt(shp_rounded(19.0, 19.0, 38.0, 32.0, 3.5)))
                .shaded(darker(STEEL, 0.45), &tilt(shp_rounded(21.0, 24.5, 36.0, 26.5, 1.0)))
                .inked(WOOD, &tilt(shp_poly(&[(13.0, 28.0), (28.0, 28.0), (23.0, 52.0), (16.0, 54.0), (8.0, 50.0)])))
                .inked(BRASS, &tilt(shp_circle((15.0, 34.0), 1.8)));
        }
        MainWeapon::Shotgun => {
            c.inked(WOOD, &tilt(shp_poly(&[(2.0, 30.0), (19.0, 26.0), (21.0, 37.0), (4.0, 46.0)])))
                .inked(GUNMETAL, &tilt(shp_ring((26.0, 39.0), 4.0, 2.4)))
                .inked(GUNMETAL, &tilt(shp_rounded(16.0, 24.0, 36.0, 37.0, 2.5)))
                .inked(STEEL, &tilt(shp_rounded(34.0, 23.0, 62.0, 30.0, 2.0)))
                .inked(GUNMETAL, &tilt(shp_rounded(34.0, 30.0, 57.0, 35.0, 2.0)))
                .inked(WOOD, &tilt(shp_rounded(38.0, 28.5, 53.0, 38.0, 3.0)))
                .shaded(WOOD_DARK, &tilt(shp_rect(42.0, 30.5, 43.2, 36.0)))
                .shaded(WOOD_DARK, &tilt(shp_rect(47.0, 30.5, 48.2, 36.0)))
                .inked(RED, &tilt(shp_circle((60.0, 22.0), 1.8)));
        }
        MainWeapon::Sniper => {
            c.soft_glow((43.0, 20.0), 10.0, with_alpha(CYAN, 0.5))
                .inked(OLIVE, &tilt(shp_poly(&[(1.0, 29.0), (18.0, 28.0), (21.0, 38.0), (12.0, 44.0), (2.0, 43.0)])))
                .inked(STEEL, &tilt(shp_rounded(38.0, 29.5, 59.0, 34.5, 1.5)))
                .inked(GUNMETAL, &tilt(shp_rounded(55.0, 28.0, 63.0, 36.0, 1.5)))
                .inked(OLIVE, &tilt(shp_poly(&[(25.0, 35.0), (32.0, 35.0), (29.0, 47.0), (22.0, 46.0)])))
                .inked(OLIVE, &tilt(shp_rounded(16.0, 27.5, 41.0, 37.5, 2.5)))
                .inked(GUNMETAL, &tilt(shp_rect(24.0, 23.0, 28.0, 29.0)))
                .inked(GUNMETAL, &tilt(shp_rect(33.0, 23.0, 37.0, 29.0)))
                .inked(GUNMETAL, &tilt(shp_rounded(17.0, 16.0, 43.0, 25.0, 4.5)))
                .inked(CYAN, &tilt(shp_ellipse((43.0, 20.5), 2.6, 4.2)))
                .ellipse((42.0, 20.0), 0.9, 1.6, with_alpha(WHITE, 0.9));
        }
        MainWeapon::Bananarang => {
            let arc = [(9.0, 12.0), (13.0, 27.0), (22.0, 40.0), (36.0, 49.0), (55.0, 46.0)];
            c.shaded(with_alpha(WHITE, 0.35), &shp_arc((33.0, 28.0), 26.0, 2.5, -2.6, -0.9))
                .shaded(with_alpha(WHITE, 0.25), &shp_arc((33.0, 28.0), 21.0, 2.0, -2.4, -1.2))
                .inked(BANANA, &shp_path(&arc, 12.0))
                .shaded(lighter(BANANA, 0.6), &shp_path(&[(13.0, 22.0), (20.0, 35.0), (33.0, 43.0)], 2.2))
                .inked(WOOD, &shp_circle((9.0, 11.0), 4.2))
                .inked(WOOD_DARK, &shp_circle((56.0, 46.0), 4.2))
                .ellipse((27.0, 46.0), 1.6, 1.1, with_alpha(WOOD, 0.8))
                .ellipse((44.0, 49.0), 1.4, 1.0, with_alpha(WOOD, 0.8));
        }
        MainWeapon::HeroSword => {
            c.soft_glow((40.0, 24.0), 22.0, with_alpha(CYAN, 0.35))
                .inked(rgb(0.86, 0.9, 1.0), &shp_poly(&[(20.5, 36.5), (49.0, 8.0), (58.0, 6.0), (56.0, 15.0), (27.5, 43.5)]))
                .shaded(with_alpha(CYAN, 0.9), &shp_capsule((26.0, 38.0), (50.0, 14.0), 2.2))
                .inked(WOOD_DARK, &shp_capsule((22.0, 42.0), (11.0, 53.0), 5.5))
                .inked(GOLD, &shp_capsule((14.0, 34.0), (30.0, 50.0), 6.0))
                .inked(CYAN, &shp_circle((22.0, 42.0), 3.4))
                .inked(GOLD, &shp_circle((9.5, 54.5), 4.2));
        }
    }
    c.soft_shadow((2, 3), 0.5);
    c
}

/// Round badge behind every automatic weapon, rimmed with its color.
fn auto_badge(c: &mut Canvas, accent: Rgba) {
    c.inked(BADGE, &shp_circle((32.0, 32.0), 27.5))
        .soft_glow((32.0, 32.0), 25.0, with_alpha(accent, 0.45))
        .shaded(accent, &shp_ring((32.0, 32.0), 25.8, 3.0));
}

/// Draws an automatic weapon: an emblem on a round badge.
pub fn paint_auto(kind: AutoWeapon) -> Canvas {
    let mut c = Canvas::default();
    match kind {
        AutoWeapon::Aura => {
            let violet = rgb(0.75, 0.35, 1.0);
            auto_badge(&mut c, violet);
            c.soft_glow((32.0, 32.0), 20.0, with_alpha(PINK, 0.7))
                .shaded(with_alpha(lighter(violet, 0.3), 0.9), &shp_ring((32.0, 32.0), 17.0, 3.5))
                .shaded(with_alpha(PINK, 0.8), &shp_ring((32.0, 32.0), 11.5, 2.0))
                .inked(WHITE, &shp_circle((32.0, 26.0), 3.6))
                .inked(WHITE, &shp_rounded(27.5, 30.0, 36.5, 40.0, 3.5));
        }
        AutoWeapon::Lightning => {
            auto_badge(&mut c, YELLOW);
            c.soft_glow((31.0, 38.0), 16.0, with_alpha(YELLOW, 0.7))
                .inked(YELLOW, &shp_bolt(22.0, 20.0, 34.0))
                .inked(
                    rgb(0.82, 0.86, 0.95),
                    &shp_union(vec![
                        shp_circle((23.0, 22.0), 6.5),
                        shp_circle((32.0, 17.0), 8.5),
                        shp_circle((41.0, 21.0), 7.0),
                        shp_rounded(16.0, 20.0, 48.0, 28.0, 4.0),
                    ]),
                );
        }
        AutoWeapon::Orbs => {
            auto_badge(&mut c, CYAN);
            c.shaded(with_alpha(CYAN, 0.55), &shp_ring((32.0, 32.0), 15.0, 2.0)).inked(WHITE, &shp_circle((32.0, 32.0), 4.0));
            for i in 0..3 {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 3.0;
                let p = (32.0 + a.cos() * 15.0, 32.0 + a.sin() * 15.0);
                c.soft_glow(p, 10.0, with_alpha(CYAN, 0.8)).inked(CYAN, &shp_circle(p, 6.0)).ellipse((p.0 - 2.0, p.1 - 2.2), 1.6, 1.2, with_alpha(WHITE, 0.9));
            }
        }
        AutoWeapon::Axes => {
            auto_badge(&mut c, ORANGE);
            c.shaded(with_alpha(WHITE, 0.45), &shp_arc((32.0, 32.0), 21.0, 2.5, 2.0, 3.6))
                .shaded(with_alpha(WHITE, 0.45), &shp_arc((32.0, 32.0), 21.0, 2.5, -1.2, 0.3))
                .inked(WOOD, &shp_capsule((19.0, 50.0), (40.0, 18.0), 5.0))
                .inked(STEEL, &shp_poly(&[(34.0, 17.0), (42.0, 8.0), (52.0, 13.0), (53.0, 27.0), (40.0, 25.0)]))
                .shaded(lighter(STEEL, 0.7), &shp_poly(&[(48.0, 12.0), (52.0, 13.0), (53.0, 27.0), (49.5, 26.5)]))
                .inked(GUNMETAL, &shp_circle((38.0, 20.0), 2.5));
        }
        AutoWeapon::Mines => {
            auto_badge(&mut c, RED);
            c.soft_glow((32.0, 21.0), 12.0, with_alpha(RED, 0.9))
                .inked(GUNMETAL, &shp_ellipse((32.0, 40.0), 18.0, 9.0))
                .inked(darker(STEEL, 0.25), &shp_ellipse((32.0, 36.0), 13.0, 6.5))
                .inked(GUNMETAL, &shp_capsule((32.0, 34.0), (32.0, 25.0), 3.5))
                .inked(RED, &shp_circle((32.0, 22.0), 4.2))
                .ellipse((30.6, 20.8), 1.3, 1.0, with_alpha(WHITE, 0.9))
                .inked(YELLOW, &shp_rect(19.0, 39.0, 22.0, 42.0))
                .inked(YELLOW, &shp_rect(42.0, 39.0, 45.0, 42.0));
        }
        AutoWeapon::Tornado => {
            let sky = rgb(0.62, 0.85, 1.0);
            auto_badge(&mut c, sky);
            for i in (0..5).rev() {
                let f = i as f32;
                let col = if i % 2 == 0 { rgb(0.86, 0.92, 0.98) } else { rgb(0.66, 0.76, 0.88) };
                c.inked(col, &shp_ellipse((32.0 + (f * 1.3).sin() * 3.0, 16.0 + f * 7.5), 18.0 - f * 3.2, 4.6 - f * 0.4));
            }
            c.inked(WOOD, &shp_circle((46.0, 26.0), 1.8)).inked(GREEN, &shp_circle((18.0, 36.0), 1.8));
        }
        AutoWeapon::Poison => {
            let toxic = rgb(0.45, 0.95, 0.2);
            auto_badge(&mut c, toxic);
            let bottle = || shp_union(vec![shp_circle((32.0, 39.0), 13.0), shp_rounded(27.5, 16.0, 36.5, 30.0, 1.5)]);
            c.soft_glow((32.0, 40.0), 16.0, with_alpha(toxic, 0.6))
                .inked(rgb(0.78, 0.92, 0.9), &bottle())
                .shaded(toxic, &shp_and(bottle(), shp_and(shp_circle((32.0, 39.0), 10.5), shp_rect(0.0, 36.0, 64.0, 64.0))))
                .shaded(with_alpha(WHITE, 0.8), &shp_circle((28.0, 43.0), 2.0))
                .shaded(with_alpha(WHITE, 0.8), &shp_circle((35.0, 46.0), 1.3))
                .ellipse((26.0, 33.0), 1.8, 3.2, with_alpha(WHITE, 0.75))
                .inked(WOOD, &shp_rounded(26.0, 10.0, 38.0, 17.0, 2.0))
                .inked(toxic, &shp_circle((44.0, 15.0), 2.6))
                .inked(toxic, &shp_circle((47.5, 9.0), 1.7));
        }
        AutoWeapon::FireTrail => {
            auto_badge(&mut c, ORANGE);
            c.soft_glow((32.0, 38.0), 20.0, with_alpha(ORANGE, 0.6))
                .inked(GUNMETAL, &shp_rounded(12.0, 45.0, 52.0, 51.0, 3.0))
                .inked(darker(ORANGE, 0.2), &shp_flame((19.0, 48.0), 5.0, 18.0, -1.5))
                .inked(darker(ORANGE, 0.1), &shp_flame((45.0, 48.0), 6.0, 24.0, 1.5))
                .inked(ORANGE, &shp_flame((32.0, 49.0), 9.0, 37.0, 0.0))
                .shaded(YELLOW, &shp_flame((32.0, 47.0), 5.0, 21.0, 0.0));
        }
    }
    c.soft_shadow((2, 3), 0.5);
    c
}

/// Each tome's color: the cover is a dark shade of it, the emblem uses it bright.
fn tome_accent(t: Tome) -> Rgba {
    match t {
        Tome::Damage => rgb(1.0, 0.42, 0.08),
        Tome::Cooldown => rgb(0.25, 0.72, 1.0),
        Tome::Quantity => rgb(0.32, 0.42, 1.0),
        Tome::Size => rgb(0.62, 0.32, 1.0),
        Tome::ProjectileSpeed => rgb(0.72, 0.95, 0.15),
        Tome::Duration => rgb(0.9, 0.72, 0.45),
        Tome::Health => rgb(1.0, 0.3, 0.45),
        Tome::Regen => rgb(0.3, 0.92, 0.55),
        Tome::Armor => rgb(0.64, 0.7, 0.8),
        Tome::Agility => rgb(1.0, 0.86, 0.1),
        Tome::Luck => rgb(0.3, 0.75, 0.15),
        Tome::Experience => rgb(0.25, 0.95, 0.95),
        Tome::Gold => rgb(1.0, 0.66, 0.08),
        Tome::Attraction => rgb(0.92, 0.15, 0.2),
        Tome::Precision => rgb(0.95, 0.25, 0.8),
    }
}

/// Draws a tome: the same leather book for all, a colored emblem per stat.
pub fn paint_tome(t: Tome) -> Canvas {
    let mut c = Canvas::default();
    let acc = tome_accent(t);
    let cover = tone_mix(acc, SHADE_TO, 0.55);
    // Page block peeking out on the right, then cover, gold frame, spine, corners.
    c.inked(PAGES, &shp_rounded(10.0, 7.0, 55.0, 59.0, 3.0));
    for i in 0..8 {
        let y = 12.0 + i as f32 * 5.5;
        c.shaded(darker(PAGES, 0.35), &shp_rect(50.5, y, 54.0, y + 0.8));
    }
    c.inked(cover, &shp_rounded(6.0, 4.0, 50.0, 57.0, 4.0))
        .soft_glow((32.0, 30.5), 17.0, with_alpha(acc, 0.35))
        .shaded(BRASS, &shp_minus(shp_rounded(17.5, 8.5, 46.5, 52.5, 3.0), shp_rounded(19.0, 10.0, 45.0, 51.0, 2.0)))
        .inked(darker(cover, 0.35), &shp_rounded(6.0, 4.0, 14.0, 57.0, 3.0))
        .inked(BRASS, &shp_rect(6.0, 11.0, 14.0, 13.5))
        .inked(BRASS, &shp_rect(6.0, 47.5, 14.0, 50.0))
        .inked(BRASS, &shp_poly(&[(42.0, 4.0), (50.0, 4.0), (50.0, 12.0)]))
        .inked(BRASS, &shp_poly(&[(42.0, 57.0), (50.0, 57.0), (50.0, 49.0)]));
    let s = (32.0, 30.5);
    match t {
        Tome::Damage => {
            c.inked(rgb(0.88, 0.92, 1.0), &shp_poly(&[(21.6, 38.2), (39.6, 14.2), (44.0, 11.0), (42.4, 17.8), (26.4, 41.8)]))
                .inked(WOOD_DARK, &shp_capsule((23.0, 41.0), (19.5, 45.5), 4.0))
                .inked(acc, &shp_capsule((18.5, 35.5), (29.0, 43.5), 5.0))
                .inked(acc, &shp_circle((18.5, 47.5), 3.0));
        }
        Tome::Cooldown => {
            c.inked(acc, &shp_rounded(29.0, 14.0, 35.0, 18.5, 1.0))
                .inked(acc, &shp_circle(s, 12.5))
                .inked(PAGES, &shp_circle(s, 8.5))
                .shaded(INK, &shp_capsule(s, (32.0, 24.5), 2.2))
                .shaded(INK, &shp_capsule(s, (36.5, 32.5), 2.2))
                .shaded(acc, &shp_circle(s, 1.6));
        }
        Tome::Quantity => {
            for p in [(32.0, 21.5), (24.0, 36.0), (40.0, 36.0)] {
                c.inked(acc, &shp_circle(p, 6.5)).ellipse((p.0 - 2.0, p.1 - 2.2), 1.7, 1.3, with_alpha(WHITE, 0.9));
            }
        }
        Tome::Size => {
            for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let tip = (32.0 + dx * 12.5, 30.5 + dy * 13.0);
                c.inked(lighter(acc, 0.35), &shp_poly(&[tip, (tip.0 - dx * 9.0, tip.1 - dy * 1.5), (tip.0 - dx * 1.5, tip.1 - dy * 9.0)]));
            }
            c.inked(acc, &shp_circle(s, 7.5)).ellipse((29.5, 28.0), 2.0, 1.5, with_alpha(WHITE, 0.9));
        }
        Tome::ProjectileSpeed => {
            c.inked(lighter(acc, 0.4), &shp_capsule((20.0, 22.5), (29.0, 22.5), 3.0))
                .inked(lighter(acc, 0.4), &shp_capsule((20.0, 38.5), (29.0, 38.5), 3.0))
                .inked(acc, &shp_capsule((20.0, 30.5), (38.0, 30.5), 5.5))
                .inked(acc, &shp_poly(&[(35.0, 21.0), (45.5, 30.5), (35.0, 40.0)]));
        }
        Tome::Duration => {
            let glass = || shp_union(vec![shp_poly(&[(24.0, 17.0), (40.0, 17.0), (33.2, 30.5), (30.8, 30.5)]), shp_poly(&[(30.8, 30.5), (33.2, 30.5), (40.0, 44.0), (24.0, 44.0)])]);
            c.inked(rgb(0.85, 0.95, 1.0), &glass())
                .shaded(acc, &shp_and(glass(), shp_rect(0.0, 36.0, 64.0, 64.0)))
                .shaded(acc, &shp_poly(&[(28.0, 22.0), (36.0, 22.0), (32.0, 28.5)]))
                .inked(WOOD, &shp_capsule((21.0, 15.5), (43.0, 15.5), 4.0))
                .inked(WOOD, &shp_capsule((21.0, 45.5), (43.0, 45.5), 4.0));
        }
        Tome::Health => {
            c.inked(acc, &shp_heart((32.0, 32.0), 28.0)).ellipse((25.0, 25.5), 2.4, 1.8, with_alpha(WHITE, 0.9));
        }
        Tome::Regen => {
            c.inked(acc, &shp_union(vec![shp_rounded(28.0, 17.0, 36.0, 44.0, 2.0), shp_rounded(18.5, 26.5, 45.5, 34.5, 2.0)]))
                .inked(WHITE, &shp_sparkle((42.5, 18.5), 5.0))
                .inked(WHITE, &shp_sparkle((21.0, 42.0), 3.5));
        }
        Tome::Armor => {
            c.inked(acc, &shp_poly(&[(19.0, 16.5), (32.0, 13.5), (45.0, 16.5), (44.0, 31.0), (32.0, 46.5), (20.0, 31.0)]))
                .shaded(darker(acc, 0.3), &shp_poly(&[(32.0, 15.5), (43.0, 18.0), (42.0, 30.5), (32.0, 43.5)]))
                .inked(BRASS, &shp_circle((32.0, 28.0), 3.5));
        }
        Tome::Agility => {
            c.inked(acc, &shp_bolt(20.0, 13.5, 34.0));
        }
        Tome::Luck => {
            c.inked(darker(acc, 0.3), &shp_capsule((32.0, 31.0), (37.5, 45.5), 3.0));
            for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                c.inked(acc, &shp_heart_turned((32.0 + dx * 5.8, 29.5 + dy * 5.8), 12.5, dx, dy));
            }
            c.shaded(lighter(acc, 0.5), &shp_circle((32.0, 29.5), 1.6));
        }
        Tome::Experience => {
            c.inked(acc, &shp_poly(&[(24.0, 18.0), (40.0, 18.0), (46.0, 26.0), (32.0, 45.0), (18.0, 26.0)]))
                .shaded(lighter(acc, 0.55), &shp_poly(&[(26.5, 19.5), (37.5, 19.5), (40.5, 26.0), (23.5, 26.0)]))
                .shaded(darker(acc, 0.3), &shp_poly(&[(32.0, 43.0), (44.0, 27.0), (36.0, 27.0)]));
        }
        Tome::Gold => {
            c.inked(darker(acc, 0.15), &shp_circle((38.5, 37.5), 8.5))
                .inked(acc, &shp_circle((29.0, 27.5), 11.5))
                .shaded(lighter(acc, 0.45), &shp_ring((29.0, 27.5), 8.0, 1.6))
                .inked(lighter(acc, 0.7), &shp_sparkle((29.0, 27.5), 5.5));
        }
        Tome::Attraction => {
            let ring = shp_and(shp_ring((32.0, 27.0), 9.0, 7.0), shp_rect(0.0, 0.0, 64.0, 27.0));
            c.inked(acc, &shp_union(vec![ring, shp_rect(19.5, 26.5, 26.5, 39.0), shp_rect(37.5, 26.5, 44.5, 39.0)]))
                .inked(STEEL, &shp_rect(19.5, 39.0, 26.5, 45.0))
                .inked(STEEL, &shp_rect(37.5, 39.0, 44.5, 45.0));
        }
        Tome::Precision => {
            c.inked(acc, &shp_ring(s, 10.0, 4.0));
            for (a, b) in [((32.0, 14.0), (32.0, 21.0)), ((32.0, 40.0), (32.0, 47.0)), ((15.5, 30.5), (22.5, 30.5)), ((41.5, 30.5), (48.5, 30.5))] {
                c.inked(acc, &shp_capsule(a, b, 3.5));
            }
            c.inked(WHITE, &shp_circle(s, 2.8));
        }
    }
    c.soft_shadow((2, 3), 0.5);
    c
}

/// Shaded toolkit used by the weapon and tome icons: shapes are tests rasterized
/// into a supersampled mask, painted in 3 tones (base, shadow rim on the lower
/// right, highlight rim on the upper left) with a bold ink stroke around them.
mod shaded {
    use super::*;

    pub(super) const WOOD: Rgba = rgb(0.62, 0.34, 0.14);
    pub(super) const WOOD_DARK: Rgba = rgb(0.36, 0.19, 0.08);
    pub(super) const STEEL: Rgba = rgb(0.72, 0.76, 0.84);
    pub(super) const GUNMETAL: Rgba = rgb(0.28, 0.3, 0.38);
    pub(super) const BRASS: Rgba = rgb(0.98, 0.74, 0.22);
    pub(super) const OLIVE: Rgba = rgb(0.38, 0.44, 0.26);
    pub(super) const BANANA: Rgba = rgb(1.0, 0.86, 0.2);
    pub(super) const PAGES: Rgba = rgb(0.97, 0.92, 0.8);
    pub(super) const BADGE: Rgba = rgb(0.16, 0.1, 0.24);
    /// What shadows tend to (a deep plum, not black: keeps colors rich).
    pub(super) const SHADE_TO: Rgba = rgb(0.07, 0.02, 0.14);

    const MS: usize = SIZE * SS; // mask resolution (samples per axis)

    pub(super) type ShapeFn = Box<dyn Fn(Vec2) -> bool>;

    pub(super) fn tone_mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3]]
    }
    pub(super) fn darker(c: Rgba, t: f32) -> Rgba {
        tone_mix(c, SHADE_TO, t)
    }
    pub(super) fn lighter(c: Rgba, t: f32) -> Rgba {
        tone_mix(c, rgb(1.0, 0.98, 0.92), t)
    }
    pub(super) fn with_alpha(c: Rgba, a: f32) -> Rgba {
        [c[0], c[1], c[2], a]
    }

    pub(super) fn shp_circle(c: (f32, f32), r: f32) -> ShapeFn {
        let c = Vec2::from(c);
        Box::new(move |p| p.distance(c) <= r)
    }
    pub(super) fn shp_ellipse(c: (f32, f32), rx: f32, ry: f32) -> ShapeFn {
        Box::new(move |p| ((p.x - c.0) / rx).powi(2) + ((p.y - c.1) / ry).powi(2) <= 1.0)
    }
    pub(super) fn shp_ring(c: (f32, f32), r: f32, thickness: f32) -> ShapeFn {
        let c = Vec2::from(c);
        Box::new(move |p| (p.distance(c) - r).abs() <= thickness * 0.5)
    }
    /// Ring section between angles `a0` and `a1` (radians, y down).
    pub(super) fn shp_arc(c: (f32, f32), r: f32, thickness: f32, a0: f32, a1: f32) -> ShapeFn {
        let c = Vec2::from(c);
        Box::new(move |p| {
            let d = p - c;
            let mut a = d.y.atan2(d.x);
            while a < a0 {
                a += std::f32::consts::TAU;
            }
            (d.length() - r).abs() <= thickness * 0.5 && a <= a1
        })
    }
    pub(super) fn shp_rect(x0: f32, y0: f32, x1: f32, y1: f32) -> ShapeFn {
        Box::new(move |p| p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1)
    }
    pub(super) fn shp_rounded(x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> ShapeFn {
        Box::new(move |p| p.distance(Vec2::new(p.x.clamp(x0 + r, x1 - r), p.y.clamp(y0 + r, y1 - r))) <= r)
    }
    /// Thick segment with round ends.
    pub(super) fn shp_capsule(a: (f32, f32), b: (f32, f32), width: f32) -> ShapeFn {
        let (a, b) = (Vec2::from(a), Vec2::from(b));
        Box::new(move |p| dist_to_segment(p, a, b) <= width * 0.5)
    }
    /// Thick polyline with round joints.
    pub(super) fn shp_path(pts: &[(f32, f32)], width: f32) -> ShapeFn {
        let pts: Vec<Vec2> = pts.iter().map(|&p| Vec2::from(p)).collect();
        Box::new(move |p| pts.windows(2).any(|w| dist_to_segment(p, w[0], w[1]) <= width * 0.5))
    }
    /// Polygon (even-odd rule).
    pub(super) fn shp_poly(pts: &[(f32, f32)]) -> ShapeFn {
        let pts: Vec<Vec2> = pts.iter().map(|&p| Vec2::from(p)).collect();
        Box::new(move |p| {
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
    /// Heart of width ~`s` centered at `c`, tip down.
    pub(super) fn shp_heart(c: (f32, f32), s: f32) -> ShapeFn {
        shp_heart_turned(c, s, 0.0, -1.0)
    }
    /// Heart whose tip points toward `-(dx, dy)` (a clover leaf when 4 of them meet).
    pub(super) fn shp_heart_turned(c: (f32, f32), s: f32, dx: f32, dy: f32) -> ShapeFn {
        let heart = shp_union(vec![
            shp_circle((-s * 0.24, -s * 0.1), s * 0.28),
            shp_circle((s * 0.24, -s * 0.1), s * 0.28),
            shp_poly(&[(-s * 0.49, 0.0), (s * 0.49, 0.0), (0.0, s * 0.48)]),
        ]);
        // The heart above points down (+y): turn the tip toward -(dx, dy).
        let target = Vec2::new(-dx, -dy).to_angle();
        let back = Vec2::from_angle(std::f32::consts::FRAC_PI_2 - target);
        let c = Vec2::from(c);
        Box::new(move |p| heart(back.rotate(p - c)))
    }
    /// Zigzag lightning bolt in a `s` square at (x, y).
    pub(super) fn shp_bolt(x: f32, y: f32, s: f32) -> ShapeFn {
        shp_poly(&[
            (x + 0.15 * s, y),
            (x + 0.62 * s, y),
            (x + 0.44 * s, y + 0.38 * s),
            (x + 0.78 * s, y + 0.38 * s),
            (x + 0.22 * s, y + s),
            (x + 0.34 * s, y + 0.55 * s),
            (x, y + 0.55 * s),
        ])
    }
    /// Flame (teardrop) standing on `base`, `r` wide-radius, `h` tall, tip leaning by `lean`.
    pub(super) fn shp_flame(base: (f32, f32), r: f32, h: f32, lean: f32) -> ShapeFn {
        let (bx, by) = base;
        shp_union(vec![shp_circle((bx, by - r), r), shp_poly(&[(bx - r * 0.97, by - r * 1.25), (bx + lean, by - h), (bx + r * 0.97, by - r * 1.25)])])
    }
    /// 4-point star.
    pub(super) fn shp_sparkle(c: (f32, f32), r: f32) -> ShapeFn {
        let k = r * 0.28;
        shp_poly(&[(c.0, c.1 - r), (c.0 + k, c.1 - k), (c.0 + r, c.1), (c.0 + k, c.1 + k), (c.0, c.1 + r), (c.0 - k, c.1 + k), (c.0 - r, c.1), (c.0 - k, c.1 - k)])
    }
    pub(super) fn shp_union(parts: Vec<ShapeFn>) -> ShapeFn {
        Box::new(move |p| parts.iter().any(|s| s(p)))
    }
    pub(super) fn shp_and(a: ShapeFn, b: ShapeFn) -> ShapeFn {
        Box::new(move |p| a(p) && b(p))
    }
    pub(super) fn shp_minus(a: ShapeFn, b: ShapeFn) -> ShapeFn {
        Box::new(move |p| a(p) && !b(p))
    }
    /// Turns a shape by `angle` (radians, negative = counterclockwise on screen) around `c`.
    pub(super) fn shp_turn(s: ShapeFn, c: (f32, f32), angle: f32) -> ShapeFn {
        let (c, back) = (Vec2::from(c), Vec2::from_angle(-angle));
        Box::new(move |p| s(back.rotate(p - c) + c))
    }

    /// A shape rasterized at the supersample resolution.
    struct Mask(Vec<bool>);

    impl Mask {
        fn of(shape: &ShapeFn) -> Self {
            let mut m = vec![false; MS * MS];
            for sy in 0..MS {
                for sx in 0..MS {
                    m[sy * MS + sx] = shape(Vec2::new((sx as f32 + 0.5) / SS as f32, (sy as f32 + 0.5) / SS as f32));
                }
            }
            Mask(m)
        }
        fn at(&self, sx: i32, sy: i32) -> bool {
            sx >= 0 && sy >= 0 && (sx as usize) < MS && (sy as usize) < MS && self.0[sy as usize * MS + sx as usize]
        }
        /// Distance (in pixels) from every sample to the shape (two-pass chamfer).
        fn distance(&self) -> Vec<f32> {
            let mut d: Vec<f32> = self.0.iter().map(|&b| if b { 0.0 } else { 1e6 }).collect();
            let (o, g) = (1.0, std::f32::consts::SQRT_2);
            for y in 0..MS {
                for x in 0..MS {
                    let i = y * MS + x;
                    let mut v = d[i];
                    if x > 0 {
                        v = v.min(d[i - 1] + o);
                    }
                    if y > 0 {
                        v = v.min(d[i - MS] + o);
                        if x > 0 {
                            v = v.min(d[i - MS - 1] + g);
                        }
                        if x + 1 < MS {
                            v = v.min(d[i - MS + 1] + g);
                        }
                    }
                    d[i] = v;
                }
            }
            for y in (0..MS).rev() {
                for x in (0..MS).rev() {
                    let i = y * MS + x;
                    let mut v = d[i];
                    if x + 1 < MS {
                        v = v.min(d[i + 1] + o);
                    }
                    if y + 1 < MS {
                        v = v.min(d[i + MS] + o);
                        if x + 1 < MS {
                            v = v.min(d[i + MS + 1] + g);
                        }
                        if x > 0 {
                            v = v.min(d[i + MS - 1] + g);
                        }
                    }
                    d[i] = v;
                }
            }
            d.iter().map(|v| v / SS as f32).collect()
        }
    }

    impl Canvas {
        /// Like `fill`, but tested on supersample coordinates (0..SIZE*SS).
        fn fill_samples(&mut self, color: Rgba, inside: impl Fn(usize, usize) -> bool) -> &mut Self {
            for y in 0..SIZE {
                for x in 0..SIZE {
                    let mut hits = 0;
                    for sy in 0..SS {
                        for sx in 0..SS {
                            if inside(x * SS + sx, y * SS + sy) {
                                hits += 1;
                            }
                        }
                    }
                    if hits == 0 {
                        continue;
                    }
                    let a = color[3] * hits as f32 / (SS * SS) as f32;
                    self.blend_px(x, y, color, a);
                }
            }
            self
        }

        fn blend_px(&mut self, x: usize, y: usize, color: Rgba, a: f32) {
            let dst = &mut self.px[y * SIZE + x];
            for c in 0..3 {
                dst[c] = color[c] * a + dst[c] * (1.0 - a);
            }
            dst[3] = a + dst[3] * (1.0 - a);
        }

        fn shaded_mask(&mut self, color: Rgba, m: &Mask) -> &mut Self {
            // Rims: samples whose neighbour toward the light (or away from it) is outside.
            let (sx, sy) = (10, 11);
            let (hx, hy) = (-6, -6);
            let alpha = color[3];
            self.fill_samples(color, |x, y| m.0[y * MS + x])
                .fill_samples(with_alpha(darker(color, 0.42), alpha), |x, y| m.0[y * MS + x] && !m.at(x as i32 + sx, y as i32 + sy))
                .fill_samples(with_alpha(lighter(color, 0.5), alpha), |x, y| m.0[y * MS + x] && !m.at(x as i32 + hx, y as i32 + hy))
        }

        /// Paints a shape in 3 tones (no stroke).
        pub(super) fn shaded(&mut self, color: Rgba, shape: &ShapeFn) -> &mut Self {
            let m = Mask::of(shape);
            self.shaded_mask(color, &m)
        }

        /// Bold ink stroke around a shape, then the shape in 3 tones.
        pub(super) fn inked(&mut self, color: Rgba, shape: &ShapeFn) -> &mut Self {
            let m = Mask::of(shape);
            let d = m.distance();
            self.fill_samples(INK, |x, y| d[y * MS + x] <= 1.9);
            self.shaded_mask(color, &m)
        }

        /// Soft radial glow (alpha fading out to `r`).
        pub(super) fn soft_glow(&mut self, c: (f32, f32), r: f32, color: Rgba) -> &mut Self {
            let c = Vec2::from(c);
            for y in 0..SIZE {
                for x in 0..SIZE {
                    let d = Vec2::new(x as f32 + 0.5, y as f32 + 0.5).distance(c) / r;
                    if d < 1.0 {
                        self.blend_px(x, y, color, color[3] * (1.0 - d) * (1.0 - d));
                    }
                }
            }
            self
        }

        /// Soft drop shadow under everything drawn so far.
        pub(super) fn soft_shadow(&mut self, off: (i32, i32), strength: f32) -> &mut Self {
            let src = self.px.clone();
            let alpha = |x: i32, y: i32| -> f32 {
                if x < 0 || y < 0 || x >= SIZE as i32 || y >= SIZE as i32 { 0.0 } else { src[y as usize * SIZE + x as usize][3] }
            };
            for y in 0..SIZE as i32 {
                for x in 0..SIZE as i32 {
                    let mut s = 0.0;
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            s += alpha(x - off.0 + dx, y - off.1 + dy);
                        }
                    }
                    let sa = s / 9.0 * strength;
                    let own = src[y as usize * SIZE + x as usize];
                    let out_a = own[3] + sa * (1.0 - own[3]);
                    if out_a <= 0.0 {
                        continue;
                    }
                    let dst = &mut self.px[y as usize * SIZE + x as usize];
                    for ch in 0..3 {
                        dst[ch] = (own[ch] * own[3] + INK[ch] * sa * (1.0 - own[3])) / out_a;
                    }
                    dst[3] = out_a;
                }
            }
            self
        }
    }
}
use shaded::*;

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
