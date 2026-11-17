//! Night ride: a side-on run along an elevated highway through a neon city.
//! Two layers of skyline slide past at different speeds, and other riders
//! streak by as long-exposure light trails, red going your way and white
//! coming the other.
//!
//! An original scene in the spirit of late-80s cyberpunk anime; it borrows
//! the mood, not any particular film's imagery.

use super::{hash, scale, Animation, Rgb, Rng, Sample};

const TAIL_RED: Rgb = [255, 35, 45];
const HEADLIGHT: Rgb = [255, 235, 180];
const SIGNS: [Rgb; 3] = [[255, 40, 200], [40, 230, 255], [255, 220, 60]];
/// Columns per second that the road markings slide by.
const ROAD_SPEED: f32 = 45.0;

/// One layer of skyline.
struct Skyline {
    /// Columns each building's plot takes up.
    plot: i64,
    /// Columns per second it slides past.
    speed: f32,
    /// Shortest and tallest building, as a share of the sky's height.
    heights: (f32, f32),
    near: bool,
}

const LAYERS: [Skyline; 2] = [
    Skyline { plot: 7, speed: 1.5, heights: (0.2, 0.45), near: false },
    Skyline { plot: 11, speed: 5.0, heights: (0.3, 0.62), near: true },
];

struct Rider {
    /// Column of the light itself; the trail stretches out behind it.
    x: f32,
    /// Columns per second across the screen; negative for oncoming riders.
    speed: f32,
    /// Position across the road, 0 (far side) to 1 (near side).
    lane: f32,
    length: usize,
    phase: f32,
}

pub struct NightRide {
    w: usize,
    h: usize,
    density: f32,
    t: f32,
    riders: Vec<Rider>,
    next_rider: f32,
    cells: Vec<Option<Sample>>,
    rng: Rng,
}

impl NightRide {
    pub fn new(w: usize, h: usize, density: f32) -> Self {
        let mut n = NightRide {
            w: 0,
            h: 0,
            density: density.clamp(0.1, 4.0),
            t: 0.0,
            riders: Vec::new(),
            next_rider: 0.0,
            cells: Vec::new(),
            rng: Rng::new(),
        };
        n.resize(w, h);
        n
    }

    fn draw(&mut self) {
        let (w, h, t) = (self.w, self.h, self.t);
        self.cells.fill(None);
        if w == 0 || h < 4 {
            return;
        }
        // The row the guard rail runs along; the road is everything below.
        let rail = ((h as f32 * 0.7) as usize).min(h - 2);
        let cells = &mut self.cells;

        // A few faint stars high up.
        for y in 0..rail / 2 {
            for x in 0..w {
                if hash(x as i32, y as i32, 11) % 300 == 0 {
                    cells[y * w + x] = Some(Sample::glyph('.', [70, 70, 110]));
                }
            }
        }

        for (layer, sky) in LAYERS.iter().enumerate() {
            let layer = layer as i32;
            let window_share = ((if sky.near { 58.0 } else { 40.0 }) * self.density) as u32;
            for x in 0..w {
                let world = x as i64 + (t * sky.speed) as i64;
                let (plot, bx) = (world.div_euclid(sky.plot) as i32, world.rem_euclid(sky.plot) as usize);
                let id = hash(plot, layer, 1);
                let width = sky.plot as usize - 1 - (id % 3) as usize;
                if bx >= width {
                    continue; // the gap between two buildings
                }
                let tall = sky.heights.0 + (sky.heights.1 - sky.heights.0) * ((id >> 4) % 100) as f32 / 100.0;
                let top = rail - ((tall * rail as f32) as usize).clamp(2.min(rail), rail);
                // A building hides whatever is behind it.
                for y in top..rail {
                    cells[y * w + x] = None;
                }
                let roof = if sky.near { [70, 55, 125] } else { [35, 45, 90] };
                cells[top * w + x] = Some(Sample::glyph('▔', roof));

                // A warning light on some rooftops, blinking slowly.
                if id % 7 == 3 && bx == width / 2 && top > 0 {
                    let on = (t * 0.8 + (id % 10) as f32 / 10.0).fract() < 0.5;
                    cells[(top - 1) * w + x] = on.then_some(Sample::glyph('*', [255, 40, 40]));
                }

                let sign = sky.near && id % 4 == 0 && width >= 5;
                let sign_len = 3 + (id >> 9) as usize % 5;
                for y in top + 1..rail {
                    let by = y - top;
                    if sign && bx == 0 {
                        // A vertical neon sign down the building's edge.
                        if (2..2 + sign_len).contains(&by) {
                            let g = hash(plot, by as i32, 5);
                            let ch = char::from_u32(0xFF71 + g % 44).unwrap_or('ｱ');
                            let flicker = hash(plot, (t * 9.0) as i32, 3) % 19 == 0;
                            let colour = SIGNS[(id >> 13) as usize % SIGNS.len()];
                            cells[y * w + x] = Some(Sample::glyph(ch, scale(colour, if flicker { 0.35 } else { 1.0 })));
                        }
                        continue;
                    }
                    // Faint walls, so the building reads as a shape.
                    if bx == 0 || bx + 1 == width {
                        let wall = if bx == 0 { '▏' } else { '▕' };
                        cells[y * w + x] = Some(Sample::glyph(wall, scale(roof, 0.75)));
                        continue;
                    }
                    if by % 2 == 0 || bx % 2 == 0 {
                        continue;
                    }
                    let g = hash(plot * 31 + layer, bx as i32, by as i32);
                    if g % 100 >= window_share {
                        continue;
                    }
                    let colour: Rgb = match (sky.near, (g >> 8) % 10) {
                        (false, _) => [70, 110, 150],
                        (true, 0..=6) => [255, 190, 90],
                        (true, 7..=8) => [90, 220, 255],
                        (true, _) => [255, 90, 130],
                    };
                    let bright = 0.5 + 0.25 * ((g >> 12) % 3) as f32;
                    cells[y * w + x] = Some(Sample::glyph('▪', scale(colour, bright)));
                }
            }
        }

        for x in 0..w {
            cells[rail * w + x] = Some(Sample::glyph('━', [55, 60, 90]));
        }

        // Lane markings rushing past underfoot.
        let road = h - 1 - rail;
        if road >= 3 {
            for line in 1..=2 {
                let y = rail + road * line / 3 + 1;
                for x in 0..w {
                    if (x as i64 + (t * ROAD_SPEED) as i64).rem_euclid(14) < 5 && y < h {
                        cells[y * w + x] = Some(Sample::glyph('─', [85, 85, 100]));
                    }
                }
            }
        }

        for r in &self.riders {
            let oncoming = r.speed < 0.0;
            let (colour, back) = if oncoming { (HEADLIGHT, 1.0) } else { (TAIL_RED, -1.0) };
            for k in 0..r.length {
                let px = r.x + back * k as f32;
                if px < 0.0 || px >= w as f32 {
                    continue;
                }
                // The trail follows the weaving line the rider took.
                let weave = 0.9 * (px * 0.035 + r.phase).sin();
                let y = rail as f32 + 1.0 + r.lane * (road.max(1) - 1) as f32 + weave;
                let y = (y.round() as usize).clamp(rail + 1, h - 1);
                let fade = 1.0 - k as f32 / r.length as f32;
                let sample = if k == 0 {
                    Sample::glyph('━', [255, 245, 240])
                } else {
                    let ch = if fade > 0.5 {
                        '━'
                    } else if fade > 0.2 {
                        '─'
                    } else {
                        '╌'
                    };
                    Sample::glyph(ch, scale(colour, (fade * 8.0).ceil() / 8.0))
                };
                cells[y * w + px as usize] = Some(sample);
            }
        }
    }
}

impl Animation for NightRide {
    fn resize(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.cells = vec![None; w * h];
        self.riders.clear();
    }

    fn step(&mut self, dt: f32) {
        self.t += dt;
        let w = self.w as f32;
        for r in self.riders.iter_mut() {
            r.x += r.speed * dt;
        }
        // Gone once the whole trail has left the screen.
        self.riders.retain(|r| {
            if r.speed > 0.0 { r.x - (r.length as f32) < w } else { r.x + r.length as f32 > 0.0 }
        });

        self.next_rider -= dt;
        if self.next_rider <= 0.0 && self.riders.len() < (6.0 * self.density).ceil() as usize {
            let rng = &mut self.rng;
            let oncoming = rng.below(4) == 0;
            self.riders.push(Rider {
                x: if oncoming { w } else { 0.0 },
                speed: if oncoming { -rng.range(90.0, 150.0) } else { rng.range(25.0, 70.0) },
                // Oncoming traffic keeps to the far side of the road.
                lane: if oncoming { rng.range(0.0, 0.3) } else { rng.range(0.35, 1.0) },
                length: (rng.range(0.2, 0.5) * w) as usize + 8,
                phase: rng.range(0.0, std::f32::consts::TAU),
            });
            self.next_rider = rng.range(0.3, 1.8) / self.density;
        }
        self.draw();
    }

    fn sample(&self, x: usize, y: usize) -> Option<Sample> {
        self.cells.get(y * self.w + x).copied().flatten()
    }
}
