//! Aurora: curtains of light whose lower edge ripples slowly across the
//! screen, bright green at the edge and fading up through teal to violet.

use super::{lerp, scale, Animation, Rgb, Sample};

const GREEN: Rgb = [40, 255, 120];
const TEAL: Rgb = [30, 200, 210];
const VIOLET: Rgb = [140, 70, 230];
/// Brightness and colour are rounded to this many steps, so that slow
/// changes redraw a cell only now and then.
const LEVELS: f32 = 12.0;
const HUES: f32 = 6.0;

pub struct Aurora {
    w: usize,
    h: usize,
    t: f32,
    curtains: usize,
    cells: Vec<Option<Sample>>,
}

impl Aurora {
    pub fn new(w: usize, h: usize, density: f32) -> Self {
        let curtains = ((3.0 * density).round() as usize).clamp(1, 6);
        let mut a = Aurora { w: 0, h: 0, t: 0.0, curtains, cells: Vec::new() };
        a.resize(w, h);
        a
    }
}

impl Animation for Aurora {
    fn resize(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.cells = vec![None; w * h];
    }

    fn step(&mut self, dt: f32) {
        self.t += dt;
        let (w, h, t) = (self.w, self.h, self.t);
        let hf = h as f32;
        for x in 0..w {
            let xf = x as f32;
            // Vertical rays: neighbouring columns shimmer out of step.
            let ray = 0.5 + 0.5 * (xf * 1.9 + t * 0.9 + 2.5 * (xf * 0.23 + t * 0.17).sin()).sin();
            // Broad dark lanes drift through the curtains as well.
            let ray = (0.25 + 0.75 * ray) * (0.55 + 0.45 * (xf * 0.045 - t * 0.11).sin()).min(1.0);
            for y in 0..h {
                let (mut total, mut hue) = (0.0f32, 0.0f32);
                for k in 0..self.curtains {
                    let p = k as f32 * 2.4;
                    let base = 0.15 + 0.7 * (k as f32 + 1.0) / (self.curtains as f32 + 1.0);
                    let edge = hf
                        * (base
                            + 0.11 * (xf * (0.031 + 0.007 * p) + t * 0.19 + p).sin()
                            + 0.06 * (xf * 0.083 - t * 0.27 + p * 1.7).sin());
                    let tail = hf * (0.30 + 0.12 * (xf * 0.05 + t * 0.31 + p).sin());
                    let d = edge - y as f32;
                    // Long fade above the edge, a sharp cut-off below it.
                    let b = if d >= 0.0 { (-d / tail).exp() } else { (d / 0.8).exp() };
                    let b = b * (0.65 + 0.35 * (t * 0.13 + p).sin().abs());
                    total += b;
                    hue += b * (d / tail).max(0.0);
                }
                let level = ((total * ray).min(1.0) * LEVELS).round() / LEVELS;
                self.cells[y * w + x] = if level < 2.0 / LEVELS {
                    None
                } else {
                    let u = ((hue / total.max(1e-3)) * HUES).round() / HUES;
                    let colour =
                        if u < 0.5 { lerp(GREEN, TEAL, u * 2.0) } else { lerp(TEAL, VIOLET, (u - 0.5) * 1.6) };
                    let ch = if level >= 0.75 {
                        '┃'
                    } else if level >= 0.35 {
                        '│'
                    } else {
                        '╎'
                    };
                    Some(Sample::glyph(ch, scale(colour, level)))
                };
            }
        }
    }

    fn sample(&self, x: usize, y: usize) -> Option<Sample> {
        self.cells.get(y * self.w + x).copied().flatten()
    }
}
