//! Plasma: overlapping waves mapped through the whole spectrum, textured
//! with a second set of waves so it reads as drifting light, not flat paint.

use super::{scale, Animation, Rgb, Sample};

const HUES: usize = 32;
/// Texture, dimmest to brightest, with the brightness each is drawn at.
const RAMP: [(char, f32); 5] = [('·', 0.5), (':', 0.62), ('░', 0.75), ('▒', 0.88), ('▓', 1.0)];

pub struct Plasma {
    w: usize,
    h: usize,
    t: f32,
    /// Texture values below this are left dark.
    cutoff: f32,
    palette: [Rgb; HUES],
    cells: Vec<Option<Sample>>,
}

fn spectrum() -> [Rgb; HUES] {
    let mut p = [[0u8; 3]; HUES];
    for (i, c) in p.iter_mut().enumerate() {
        // Fully saturated hue wheel.
        let h = i as f32 / HUES as f32 * 6.0;
        let x = 1.0 - (h % 2.0 - 1.0).abs();
        let (r, g, b) = match h as usize {
            0 => (1.0, x, 0.0),
            1 => (x, 1.0, 0.0),
            2 => (0.0, 1.0, x),
            3 => (0.0, x, 1.0),
            4 => (x, 0.0, 1.0),
            _ => (1.0, 0.0, x),
        };
        *c = [r, g, b].map(|v| (40.0 + 215.0 * v) as u8);
    }
    p
}

impl Plasma {
    pub fn new(w: usize, h: usize, density: f32) -> Self {
        let cutoff = (0.2 / density.max(0.05)).clamp(0.0, 0.9);
        let mut p = Plasma { w: 0, h: 0, t: 0.0, cutoff, palette: spectrum(), cells: Vec::new() };
        p.resize(w, h);
        p
    }
}

impl Animation for Plasma {
    fn resize(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.cells = vec![None; w * h];
    }

    fn step(&mut self, dt: f32) {
        self.t += dt;
        let (w, h, t) = (self.w, self.h, self.t);
        let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
        for y in 0..h {
            let yf = y as f32;
            for x in 0..w {
                let xf = x as f32;
                let (dx, dy) = ((xf - cx) * 0.5, yf - cy);
                let v = (xf * 0.055 + t * 0.31).sin()
                    + (yf * 0.11 - t * 0.23).sin()
                    + (xf * 0.035 + yf * 0.07 + t * 0.19).sin()
                    + ((dx * dx + dy * dy).sqrt() * 0.16 - t * 0.37).sin();
                let hue = (v * 0.125 + 0.5 + t * 0.01).rem_euclid(1.0);
                let grain = 0.5 + 0.5 * (xf * 0.13 + t * 0.27 + 1.3 * (yf * 0.19 - t * 0.21).sin()).sin();
                self.cells[y * w + x] = if grain < self.cutoff {
                    None
                } else {
                    let step = (grain - self.cutoff) / (1.0 - self.cutoff) * RAMP.len() as f32;
                    let (ch, bright) = RAMP[(step as usize).min(RAMP.len() - 1)];
                    let colour = self.palette[(hue * HUES as f32) as usize % HUES];
                    Some(Sample::glyph(ch, scale(colour, bright)))
                };
            }
        }
    }

    fn sample(&self, x: usize, y: usize) -> Option<Sample> {
        self.cells.get(y * self.w + x).copied().flatten()
    }
}
