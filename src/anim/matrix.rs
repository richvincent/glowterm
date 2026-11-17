//! The Matrix digital rain: half-width katakana falling in columns, each
//! drop led by a near-white glyph and trailed by a fading green tail.

use super::{Animation, Rgb, Rng, Sample};

/// Brightness steps in a tail. Each time a drop advances one row, about this
/// many cells change colour, so it also bounds how much is redrawn.
const LEVELS: usize = 10;
const HEAD: u8 = LEVELS as u8 + 1;

/// Fraction of cells whose glyph flickers to a new one each second.
const MUTATION_RATE: f32 = 0.08;

struct Drop {
    head: f32,
    speed: f32,
    len: f32,
}

struct Column {
    drops: Vec<Drop>,
    /// Seconds until this column releases its next drop.
    cooldown: f32,
}

pub struct Matrix {
    w: usize,
    h: usize,
    density: f32,
    glyphs: Vec<char>,
    /// Per cell: 0 dark, 1..=LEVELS tail brightness, HEAD for a leading glyph.
    lit: Vec<u8>,
    cols: Vec<Column>,
    palette: [Rgb; LEVELS + 2],
    mutation_carry: f32,
    rng: Rng,
}

fn glyph(rng: &mut Rng) -> char {
    const EXTRA: &[char] = &[
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'Z', ':', '.', '"', '=', '*', '+', '-',
        '<', '>', '|',
    ];
    if rng.below(5) == 0 {
        EXTRA[rng.below(EXTRA.len())]
    } else {
        // Half-width katakana, U+FF66..=U+FF9D: one cell wide, as in the film.
        char::from_u32(0xFF66 + rng.below(0xFF9D - 0xFF66 + 1) as u32).unwrap_or('ｱ')
    }
}

fn palette() -> [Rgb; LEVELS + 2] {
    let mut p = [[0u8; 3]; LEVELS + 2];
    for (i, c) in p.iter_mut().enumerate().take(LEVELS + 1).skip(1) {
        let t = i as f32 / LEVELS as f32;
        let g = 28.0 + 227.0 * t.powf(1.4);
        // The cells just behind the head pick up some of its white.
        let r = if t > 0.8 { 110.0 * (t - 0.8) / 0.2 } else { 0.0 };
        let b = g * 0.26 + r * 0.4;
        *c = [r as u8, g as u8, b.min(255.0) as u8];
    }
    p[HEAD as usize] = [215, 255, 225];
    p
}

impl Matrix {
    pub fn new(w: usize, h: usize, density: f32) -> Self {
        let mut m = Matrix {
            w: 0,
            h: 0,
            density: density.clamp(0.05, 8.0),
            glyphs: Vec::new(),
            lit: Vec::new(),
            cols: Vec::new(),
            palette: palette(),
            mutation_carry: 0.0,
            rng: Rng::new(),
        };
        m.resize(w, h);
        m
    }
}

impl Animation for Matrix {
    fn resize(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.glyphs = (0..w * h).map(|_| glyph(&mut self.rng)).collect();
        self.lit = vec![0; w * h];
        // Stagger the first drops so the rain builds up instead of arriving
        // as one flat line.
        self.cols = (0..w)
            .map(|_| Column { drops: Vec::new(), cooldown: self.rng.range(0.0, 2.5) / self.density })
            .collect();
    }

    fn step(&mut self, dt: f32) {
        let (w, h) = (self.w, self.h);
        if w == 0 || h == 0 {
            return;
        }
        let Self { cols, glyphs, lit, rng, density, .. } = self;

        for (x, col) in cols.iter_mut().enumerate() {
            col.cooldown -= dt;
            if col.cooldown <= 0.0 {
                let speed = rng.range(7.0, 24.0);
                let len = rng.range(6.0, (h as f32 * 0.6).max(8.0));
                let gap = rng.range(h as f32 * 0.3, h as f32 * 1.3) / *density;
                col.cooldown = (len + gap) / speed;
                col.drops.push(Drop { head: -1.0, speed, len });
            }
            for d in col.drops.iter_mut() {
                let before = d.head.floor() as i64;
                d.head += d.speed * dt;
                let after = d.head.floor() as i64;
                // Every cell the head enters gets a fresh glyph.
                for r in (before + 1).max(0)..=after.min(h as i64 - 1) {
                    glyphs[r as usize * w + x] = glyph(rng);
                }
            }
            col.drops.retain(|d| d.head - d.len < h as f32);
        }

        self.mutation_carry += (w * h) as f32 * MUTATION_RATE * dt;
        while self.mutation_carry >= 1.0 {
            self.mutation_carry -= 1.0;
            let i = rng.below(w * h);
            glyphs[i] = glyph(rng);
        }

        lit.fill(0);
        for (x, col) in cols.iter().enumerate() {
            for d in &col.drops {
                let head_row = d.head.floor() as i64;
                let top = ((d.head - d.len).ceil() as i64).max(0);
                for y in top..=head_row.min(h as i64 - 1) {
                    let dist = (head_row - y) as f32;
                    let level = if dist == 0.0 {
                        HEAD
                    } else {
                        let t = (1.0 - dist / d.len).max(0.0);
                        (1 + (t * LEVELS as f32) as usize).min(LEVELS) as u8
                    };
                    let cell = &mut lit[y as usize * w + x];
                    *cell = (*cell).max(level);
                }
            }
        }
    }

    fn sample(&self, x: usize, y: usize) -> Option<Sample> {
        let i = y * self.w + x;
        match *self.lit.get(i)? {
            0 => None,
            level => Some(Sample::glyph(self.glyphs[i], self.palette[level as usize])),
        }
    }
}
