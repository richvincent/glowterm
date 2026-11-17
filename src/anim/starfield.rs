//! Starfield: stars rush toward the viewer from the centre of the screen,
//! brightening and stretching into streaks as they pass.

use super::{scale, Animation, Rgb, Rng, Sample};

/// Depth travelled per second; a star starts at depth 1.
const SPEED: f32 = 0.3;

struct Star {
    x: f32,
    y: f32,
    z: f32,
    tint: Rgb,
}

pub struct Starfield {
    w: usize,
    h: usize,
    density: f32,
    stars: Vec<Star>,
    cells: Vec<Option<Sample>>,
    rng: Rng,
}

fn star(rng: &mut Rng, z: f32) -> Star {
    const TINTS: [Rgb; 4] = [[255, 255, 255], [190, 215, 255], [150, 190, 255], [255, 235, 200]];
    Star { x: rng.range(-1.0, 1.0), y: rng.range(-1.0, 1.0), z, tint: TINTS[rng.below(TINTS.len())] }
}

impl Starfield {
    pub fn new(w: usize, h: usize, density: f32) -> Self {
        let mut s = Starfield {
            w: 0,
            h: 0,
            density: density.clamp(0.05, 8.0),
            stars: Vec::new(),
            cells: Vec::new(),
            rng: Rng::new(),
        };
        s.resize(w, h);
        s
    }
}

impl Animation for Starfield {
    fn resize(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.cells = vec![None; w * h];
        let count = ((w * h) as f32 / 45.0 * self.density) as usize;
        self.stars = (0..count)
            .map(|_| {
                let z = self.rng.range(0.05, 1.0);
                star(&mut self.rng, z)
            })
            .collect();
    }

    fn step(&mut self, dt: f32) {
        let (w, h) = (self.w as f32, self.h as f32);
        let (cx, cy) = (w / 2.0, h / 2.0);
        self.cells.fill(None);
        let Self { stars, cells, rng, .. } = self;
        for s in stars.iter_mut() {
            s.z -= SPEED * dt;
            let (mut sx, mut sy) = (cx + s.x / s.z * cx, cy + s.y / s.z * cy);
            if s.z <= 0.02 || sx < 0.0 || sx >= w || sy < 0.0 || sy >= h {
                *s = star(rng, 1.0);
                (sx, sy) = (cx + s.x * cx, cy + s.y * cy);
                if sx < 0.0 || sx >= w || sy < 0.0 || sy >= h {
                    continue;
                }
            }
            let near = 1.0 - s.z;
            let bright = (0.15 + near * 1.1).min(1.0);
            let ch = match s.z {
                z if z > 0.7 => '.',
                z if z > 0.45 => '·',
                z if z > 0.22 => '+',
                _ => '*',
            };
            let at = |x: f32, y: f32| y as usize * w as usize + x as usize;
            cells[at(sx, sy)] = Some(Sample::glyph(ch, scale(s.tint, bright)));

            // The streak trails back toward the centre. A cell is about
            // twice as tall as it is wide, hence the factor of two.
            let (dx, dy) = (sx - cx, sy - cy);
            let longest = dx.abs().max(dy.abs());
            if longest < 1.0 {
                continue;
            }
            let (ux, uy) = (dx / longest, dy / longest);
            let (vx, vy) = (ux.abs(), uy.abs() * 2.0);
            let streak = if vx > 2.4 * vy {
                '─'
            } else if vy > 2.4 * vx {
                '│'
            } else if (ux > 0.0) == (uy > 0.0) {
                '╲'
            } else {
                '╱'
            };
            let length = (near.powi(3) * 6.0) as usize;
            for k in 1..=length {
                let (tx, ty) = (sx - ux * k as f32, sy - uy * k as f32);
                if tx < 0.0 || tx >= w || ty < 0.0 || ty >= h {
                    break;
                }
                let cell = &mut cells[at(tx, ty)];
                if cell.is_none() {
                    let fade = bright * (1.0 - k as f32 / (length as f32 + 1.0)) * 0.7;
                    *cell = Some(Sample::glyph(streak, scale(s.tint, fade)));
                }
            }
        }
    }

    fn sample(&self, x: usize, y: usize) -> Option<Sample> {
        self.cells.get(y * self.w + x).copied().flatten()
    }
}
