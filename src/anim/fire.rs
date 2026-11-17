//! Fire: heat rises from a white-hot row along the bottom edge, drifting
//! sideways and cooling as it goes. Each cell shows two heat values, one
//! above the other, using a half-block character.

use super::{lerp, Animation, Rgb, Rng, Sample};

const HOTTEST: u8 = 20;
/// Simulation steps per second, independent of the frame rate.
const TICK: f32 = 1.0 / 12.0;

pub struct Fire {
    w: usize,
    h: usize,
    /// Rows of cells the flames may occupy, counted from the bottom.
    rows: usize,
    /// Share of the screen height the flames reach.
    reach: f32,
    /// Heat per half-cell, `2 * rows` high, top row first.
    heat: Vec<u8>,
    palette: [Rgb; HOTTEST as usize + 1],
    pending: f32,
    rng: Rng,
}

fn palette() -> [Rgb; HOTTEST as usize + 1] {
    const STOPS: [(f32, Rgb); 6] = [
        (0.0, [0, 0, 0]),
        (0.2, [90, 10, 5]),
        (0.45, [200, 50, 10]),
        (0.7, [230, 140, 20]),
        (0.88, [235, 210, 60]),
        (1.0, [255, 255, 220]),
    ];
    let mut p = [[0u8; 3]; HOTTEST as usize + 1];
    for (i, c) in p.iter_mut().enumerate() {
        let t = i as f32 / HOTTEST as f32;
        let k = STOPS.iter().rposition(|s| s.0 <= t).unwrap_or(0).min(STOPS.len() - 2);
        let (a, b) = (STOPS[k], STOPS[k + 1]);
        *c = lerp(a.1, b.1, (t - a.0) / (b.0 - a.0));
    }
    p
}

impl Fire {
    pub fn new(w: usize, h: usize, density: f32) -> Self {
        let mut f = Fire {
            w: 0,
            h: 0,
            rows: 0,
            reach: (0.45 * density).clamp(0.1, 1.0),
            heat: Vec::new(),
            palette: palette(),
            pending: 0.0,
            rng: Rng::new(),
        };
        f.resize(w, h);
        f
    }

    fn tick(&mut self) {
        let (w, fh) = (self.w, self.rows * 2);
        if w == 0 || fh < 2 {
            return;
        }
        // Cool fast enough that flames die out near the top of their area.
        let cooling = HOTTEST as f32 / (0.8 * fh as f32);
        for y in 0..fh - 1 {
            for x in 0..w {
                let below = self.heat[(y + 1) * w + x] as f32;
                let drift = self.rng.below(3) as isize - 1;
                let lost = cooling * self.rng.range(0.0, 2.0);
                let lost = lost.floor() + if self.rng.unit() < lost.fract() { 1.0 } else { 0.0 };
                let to = (x as isize + drift).clamp(0, w as isize - 1) as usize;
                self.heat[y * w + to] = (below - lost).max(0.0) as u8;
            }
        }
    }
}

impl Animation for Fire {
    fn resize(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.rows = ((h as f32 * self.reach).ceil() as usize).clamp(1.min(h), h);
        self.heat = vec![0; w * self.rows * 2];
        let n = self.heat.len();
        if n >= w {
            self.heat[n - w..].fill(HOTTEST);
        }
    }

    fn step(&mut self, dt: f32) {
        self.pending = (self.pending + dt).min(4.0 * TICK);
        while self.pending >= TICK {
            self.pending -= TICK;
            self.tick();
        }
    }

    fn sample(&self, x: usize, y: usize) -> Option<Sample> {
        let row = (y + self.rows).checked_sub(self.h)?;
        let top = *self.heat.get(row * 2 * self.w + x)?;
        let bottom = *self.heat.get((row * 2 + 1) * self.w + x)?;
        if top == 0 && bottom == 0 {
            return None;
        }
        Some(Sample { ch: '▀', fg: self.palette[top as usize], bg: Some(self.palette[bottom as usize]) })
    }
}
