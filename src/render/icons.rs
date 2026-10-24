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
