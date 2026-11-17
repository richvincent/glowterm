//! Background animations. To add one: implement `Animation`, then register
//! it in `ALL` and `create` below.

mod arcade;
mod aurora;
mod cyberspace;
mod fire;
mod matrix;
mod nightride;
mod plasma;
mod starfield;

pub type Rgb = [u8; 3];

/// What an animation draws in one cell.
#[derive(Clone, Copy)]
pub struct Sample {
    pub ch: char,
    pub fg: Rgb,
    /// Background colour, for animations that fill cells (half-blocks).
    pub bg: Option<Rgb>,
}

impl Sample {
    pub fn glyph(ch: char, fg: Rgb) -> Self {
        Sample { ch, fg, bg: None }
    }
}

pub trait Animation {
    /// The grid changed size; rebuild any per-cell state.
    fn resize(&mut self, w: usize, h: usize);
    /// Advance the simulation by `dt` seconds.
    fn step(&mut self, dt: f32);
    /// What to draw in an empty cell, or `None` to leave it dark.
    fn sample(&self, x: usize, y: usize) -> Option<Sample>;
}

/// (name, description) of every animation, for `--list`.
pub const ALL: &[(&str, &str)] = &[
    ("matrix", "Katakana digital rain"),
    ("aurora", "Slow curtains of green and violet light"),
    ("fire", "Flames rising from the bottom edge"),
    ("starfield", "Stars streaming out from the centre"),
    ("plasma", "Flowing full-spectrum colour fields"),
    ("cyberspace", "Flight through a neon city of data towers"),
    ("nightride", "Tail-light trails through a neon city at night"),
    ("arcade", "Original early-console-style games playing themselves"),
];

pub fn create(name: &str, w: usize, h: usize, density: f32) -> Option<Box<dyn Animation>> {
    match name {
        "matrix" => Some(Box::new(matrix::Matrix::new(w, h, density))),
        "aurora" => Some(Box::new(aurora::Aurora::new(w, h, density))),
        "fire" => Some(Box::new(fire::Fire::new(w, h, density))),
        "starfield" => Some(Box::new(starfield::Starfield::new(w, h, density))),
        "plasma" => Some(Box::new(plasma::Plasma::new(w, h, density))),
        "cyberspace" => Some(Box::new(cyberspace::Cyberspace::new(w, h, density))),
        "nightride" => Some(Box::new(nightride::NightRide::new(w, h, density))),
        "arcade" => Some(Box::new(arcade::Arcade::new(w, h, density))),
        _ => None,
    }
}

/// Scales a colour's brightness by `k`.
pub fn scale(c: Rgb, k: f32) -> Rgb {
    c.map(|v| (v as f32 * k).clamp(0.0, 255.0) as u8)
}

/// Blends from `a` (t = 0) to `b` (t = 1).
pub fn lerp(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t) as u8)
}

/// Repeatable pseudo-random number from three integers, for procedural
/// detail that must stay put from frame to frame.
pub fn hash(a: i32, b: i32, c: i32) -> u32 {
    let mut h = (a as u32).wrapping_mul(0x9E37_79B1)
        ^ (b as u32).wrapping_mul(0x85EB_CA77)
        ^ (c as u32).wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

/// Small xorshift generator; animation randomness needs no more than this.
pub struct Rng(u64);

impl Rng {
    pub fn new() -> Self {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Rng(t | 1)
    }
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}
