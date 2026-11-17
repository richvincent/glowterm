//! Cyberspace: a flight along the streets of a city whose towers are made
//! of scrolling data, in neon on black.
//!
//! The city is an endless grid. Every third row and column of grid squares
//! is a street; every other square holds a tower. The view is drawn one
//! screen column at a time by walking a ray across the grid and drawing each
//! tower it meets, nearest first, wherever a nearer one has not already
//! covered the column.

use super::{hash, scale, Animation, Rgb, Sample};
use std::f32::consts::FRAC_PI_2;

/// Scene updates per second, independent of the frame rate.
const TICK: f32 = 1.0 / 12.0;
/// Grid squares travelled per second.
const SPEED: f32 = 1.1;
const EYE_HEIGHT: f32 = 0.9;
/// Half-width of the view, as a multiple of the forward direction.
const VIEW: f32 = 0.8;
/// Towers further than this many squares are not drawn.
const FAR: f32 = 16.0;
/// Streets run along grid lines that are multiples of this.
const BLOCK: i32 = 3;

const NEON: [Rgb; 6] =
    [[0, 255, 255], [255, 0, 200], [60, 120, 255], [255, 180, 0], [60, 255, 120], [160, 80, 255]];
const GLYPHS: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', '+', '-', '=',
    '<', '>', '/', '*', '{', '}', '[', ']', '(', ')', 'λ', 'π', 'Σ', '∂', '∫', '≈', '≠',
];
const DIRS: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

fn is_tower(i: i32, j: i32) -> bool {
    i.rem_euclid(BLOCK) != 0 && j.rem_euclid(BLOCK) != 0
}

pub struct Cyberspace {
    w: usize,
    h: usize,
    density: f32,
    t: f32,
    pending: f32,
    /// The intersection being approached or crossed (a grid square).
    node: (i32, i32),
    /// Indexes into `DIRS`: the heading into and out of that intersection.
    heading_in: usize,
    heading_out: usize,
    /// True once inside the intersection square, false on the street to it.
    crossing: bool,
    /// Distance covered along the current stretch.
    along: f32,
    turns: u32,
    cells: Vec<Option<Sample>>,
}

impl Cyberspace {
    pub fn new(w: usize, h: usize, density: f32) -> Self {
        let mut c = Cyberspace {
            w: 0,
            h: 0,
            density: density.clamp(0.1, 2.5),
            t: 0.0,
            pending: 0.0,
            node: (0, 0),
            heading_in: 0,
            heading_out: 0,
            crossing: false,
            along: 0.0,
            turns: 0,
            cells: Vec::new(),
        };
        c.resize(w, h);
        c
    }

    /// Moves the camera along its route: straight down each street, then
    /// through the intersection either straight on or round a quarter-circle.
    fn advance(&mut self, mut distance: f32) {
        loop {
            let turning = self.heading_in != self.heading_out;
            let length = match (self.crossing, turning) {
                (false, _) => (BLOCK - 1) as f32,
                (true, false) => 1.0,
                (true, true) => FRAC_PI_2 * 0.5,
            };
            if self.along + distance < length {
                self.along += distance;
                return;
            }
            distance -= length - self.along;
            self.along = 0.0;
            if !self.crossing {
                self.crossing = true;
                continue;
            }
            // Leaving the intersection: head for the next one and decide
            // now which way to go when we get there.
            let (dx, dy) = DIRS[self.heading_out];
            self.node = (self.node.0 + dx * BLOCK, self.node.1 + dy * BLOCK);
            self.heading_in = self.heading_out;
            self.turns += 1;
            self.heading_out = match hash(self.node.0, self.node.1, self.turns as i32) % 10 {
                0..=1 => (self.heading_in + 1) % 4,
                2..=3 => (self.heading_in + 3) % 4,
                _ => self.heading_in,
            };
            self.crossing = false;
        }
    }

    /// Camera position and the angle it faces.
    fn camera(&self) -> ((f32, f32), f32) {
        let (ix, iy) = (DIRS[self.heading_in].0 as f32, DIRS[self.heading_in].1 as f32);
        let (ox, oy) = (DIRS[self.heading_out].0 as f32, DIRS[self.heading_out].1 as f32);
        let facing = self.heading_in as f32 * FRAC_PI_2;
        // Where the route enters the intersection square.
        let entry = (self.node.0 as f32 + 0.5 - 0.5 * ix, self.node.1 as f32 + 0.5 - 0.5 * iy);
        if !self.crossing {
            let back = (BLOCK - 1) as f32 - self.along;
            return ((entry.0 - ix * back, entry.1 - iy * back), facing);
        }
        if self.heading_in == self.heading_out {
            return ((entry.0 + ix * self.along, entry.1 + iy * self.along), facing);
        }
        let turned = self.along / 0.5;
        let (sin, cos) = turned.sin_cos();
        let pos = (
            entry.0 + 0.5 * ox + 0.5 * (ix * sin - ox * cos),
            entry.1 + 0.5 * oy + 0.5 * (iy * sin - oy * cos),
        );
        // Left or right, from the cross product of the two headings.
        ((pos), facing + (ix * oy - iy * ox) * turned)
    }

    fn draw(&mut self) {
        let (w, h) = (self.w, self.h);
        self.cells.fill(None);
        if w == 0 || h == 0 {
            return;
        }
        let ((px, py), facing) = self.camera();
        // A slow sway, so the flight does not feel like it is on rails.
        let angle = facing + 0.1 * (self.t * 0.45).sin();
        let (dir_y, dir_x) = angle.sin_cos();
        let (plane_x, plane_y) = (-dir_y * VIEW, dir_x * VIEW);
        let horizon = h as f32 * 0.6;
        // Rows per grid square at distance 1 (a cell is twice as tall as wide).
        let rows_per_unit = w as f32 / (4.0 * VIEW);
        let glyph_share = (70.0 * self.density) as u32;

        for x in 0..w {
            let cam = 2.0 * x as f32 / w as f32 - 1.0;
            let (rx, ry) = (dir_x + plane_x * cam, dir_y + plane_y * cam);
            // The ray of the column to the left, to tell how much of a wall
            // one screen column spans.
            let (lx, ly) = (rx - plane_x * 2.0 / w as f32, ry - plane_y * 2.0 / w as f32);
            let (mut mx, mut my) = (px.floor() as i32, py.floor() as i32);
            let (ddx, ddy) = ((1.0 / rx).abs(), (1.0 / ry).abs());
            let (step_x, mut side_x) =
                if rx < 0.0 { (-1, (px - mx as f32) * ddx) } else { (1, (mx as f32 + 1.0 - px) * ddx) };
            let (step_y, mut side_y) =
                if ry < 0.0 { (-1, (py - my as f32) * ddy) } else { (1, (my as f32 + 1.0 - py) * ddy) };

            // Rows from `covered` down are already drawn in this column.
            let mut covered = h as f32;
            let mut ground_from = horizon + EYE_HEIGHT * rows_per_unit / FAR;

            for _ in 0..48 {
                let x_side = side_x < side_y;
                let dist = if x_side { side_x } else { side_y };
                if x_side {
                    side_x += ddx;
                    mx += step_x;
                } else {
                    side_y += ddy;
                    my += step_y;
                }
                if dist > FAR {
                    break;
                }
                if !is_tower(mx, my) {
                    continue;
                }
                let dist = dist.max(0.05);
                let id = hash(mx, my, 7);
                let height = 1.3 + ((id % 1000) as f32 / 1000.0).powi(2) * 6.0;
                let top = horizon - (height - EYE_HEIGHT) * rows_per_unit / dist;
                let bottom = horizon + EYE_HEIGHT * rows_per_unit / dist;
                if covered == h as f32 {
                    ground_from = bottom;
                }
                if top >= covered {
                    continue;
                }
                // Where along the wall this column's ray landed, and where
                // the previous column's did.
                let (along, before) = if x_side {
                    (py + dist * ry, py + dist * rx / lx * ly)
                } else {
                    (px + dist * rx, px + dist * ry / ly * lx)
                };
                let span = (along - before).abs().min(0.5);
                let u = along - along.floor();
                let colour = NEON[(id >> 10) as usize % NEON.len()];
                let fade = (1.0 / (1.0 + dist * 0.22)) * if x_side { 1.0 } else { 0.7 };
                let fade = (fade * 8.0).ceil() / 8.0;
                // Text is laid out on a grid that gets finer as the wall gets
                // nearer, so characters stay about one cell in size and are
                // never smeared across several.
                let fine = 2f32.powi(((rows_per_unit / dist).log2().floor() as i32).clamp(0, 6));
                let column = (along * fine).floor() as i32;
                let new_column = column != (before * fine).floor() as i32;
                // Each tower's text scrolls at its own speed, up or down.
                let scroll = self.t * (0.15 + (id >> 14) as f32 % 5.0 * 0.15) * if id & 1 == 0 { 1.0 } else { -1.0 };
                let face = hash(mx, my, x_side as i32 + fine as i32 * 2) as i32;
                let row_of = |y: f32| {
                    let z = EYE_HEIGHT + (horizon - y) * dist / rows_per_unit;
                    ((z + scroll) * fine).floor() as i32
                };
                let first = top.ceil().max(0.0) as usize;
                let last = (bottom.min(covered - 1.0).floor().max(-1.0) as isize).min(h as isize - 1);
                for y in first as isize..=last {
                    let row = row_of(y as f32);
                    let sample = if y as usize == first && top >= 0.0 {
                        Some(Sample::glyph('─', scale(colour, (fade * 1.3).min(1.0))))
                    } else if u < span || u > 1.0 - span {
                        Some(Sample::glyph('│', scale(colour, (fade * 1.3).min(1.0))))
                    } else if new_column && row != row_of(y as f32 - 1.0) {
                        let g = hash(face, column, row);
                        // Lines of text of varying length, with gaps.
                        if g % 100 < glyph_share && hash(face, column / 6, row) % 4 != 0 {
                            let ch = GLYPHS[(g >> 8) as usize % GLYPHS.len()];
                            // Now and then a character burns white.
                            let c = if (g >> 16) % 13 == 0 { [235, 255, 255] } else { colour };
                            Some(Sample::glyph(ch, scale(c, fade)))
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    self.cells[y as usize * w + x] = sample;
                }
                covered = covered.min(top.max(0.0));
                if covered <= 0.0 {
                    break;
                }
            }

            // The ground: a grid of dots receding to the horizon.
            for y in (ground_from.ceil().max(horizon + 1.0) as usize)..h {
                let d = EYE_HEIGHT * rows_per_unit / (y as f32 - horizon);
                let (gx, gy) = (px + rx * d, py + ry * d);
                let line = 0.04 + d * 0.015;
                if (gx - gx.round()).abs() < line || (gy - gy.round()).abs() < line {
                    let fade = ((1.0 / (1.0 + d * 0.3)) * 6.0).ceil() / 6.0;
                    self.cells[y * w + x] = Some(Sample::glyph('·', scale([40, 130, 255], fade)));
                }
            }
        }
    }
}

impl Animation for Cyberspace {
    fn resize(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.cells = vec![None; w * h];
        self.draw();
    }

    fn step(&mut self, dt: f32) {
        self.pending = (self.pending + dt).min(4.0 * TICK);
        let mut moved = false;
        while self.pending >= TICK {
            self.pending -= TICK;
            self.t += TICK;
            self.advance(SPEED * TICK);
            moved = true;
        }
        if moved {
            self.draw();
        }
    }

    fn sample(&self, x: usize, y: usize) -> Option<Sample> {
        self.cells.get(y * self.w + x).copied().flatten()
    }
}
