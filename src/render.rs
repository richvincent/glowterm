//! Compositing (shell text over animation) and diff-based drawing.

use crate::anim::Animation;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Col {
    Default,
    Idx(u8),
    Rgb(u8, u8, u8),
}

pub const BOLD: u8 = 1;
pub const DIM: u8 = 2;
pub const ITALIC: u8 = 4;
pub const UNDERLINE: u8 = 8;
pub const INVERSE: u8 = 16;

#[derive(Clone, PartialEq)]
pub struct Cell {
    pub ch: char,
    /// Combining characters after `ch`; almost always `None`.
    pub extra: Option<Box<str>>,
    pub fg: Col,
    pub bg: Col,
    pub attr: u8,
    /// 1 normally, 2 for a wide character, 0 for the cell a wide one covers.
    pub width: u8,
}

pub const BLANK: Cell =
    Cell { ch: ' ', extra: None, fg: Col::Default, bg: Col::Default, attr: 0, width: 1 };

/// The shell's screen, flattened, plus what the compositor needs to know
/// about where the animation may show through.
#[derive(Default)]
pub struct Layer {
    pub cells: Vec<Cell>,
    /// Cells with nothing in them, where the animation is drawn.
    pub transparent: Vec<bool>,
    /// Per row, the columns `start..end` that text occupies (with a margin).
    pub zone: Vec<(usize, usize)>,
    pub wide_rows: Vec<bool>,
}

const ZONE_MARGIN: usize = 2;
/// Runs of empty cells up to this long, between text, stay dark.
const WORD_GAP: usize = 3;

fn col(c: vt100::Color) -> Col {
    match c {
        vt100::Color::Default => Col::Default,
        vt100::Color::Idx(i) => Col::Idx(i),
        vt100::Color::Rgb(r, g, b) => Col::Rgb(r, g, b),
    }
}

impl Layer {
    pub fn rebuild(&mut self, screen: &vt100::Screen, w: usize, h: usize) {
        self.cells.clear();
        self.cells.resize(w * h, BLANK);
        self.transparent.clear();
        self.transparent.resize(w * h, true);
        self.zone.clear();
        self.wide_rows.clear();

        for y in 0..h {
            let (mut first, mut last, mut wide) = (usize::MAX, 0usize, false);
            for x in 0..w {
                let Some(c) = screen.cell(y as u16, x as u16) else { continue };
                let i = y * w + x;
                if c.is_wide_continuation() {
                    self.cells[i].width = 0;
                    self.transparent[i] = false;
                    wide = true;
                    last = x;
                    continue;
                }
                let text = c.contents();
                let empty = (text.is_empty() || text == " ")
                    && c.bgcolor() == vt100::Color::Default
                    && !c.inverse()
                    && !c.underline();
                if empty {
                    continue;
                }
                let mut chars = text.chars();
                let ch = chars.next().unwrap_or(' ');
                let rest = chars.as_str();
                let mut attr = 0;
                for (on, bit) in [
                    (c.bold(), BOLD),
                    (c.dim(), DIM),
                    (c.italic(), ITALIC),
                    (c.underline(), UNDERLINE),
                    (c.inverse(), INVERSE),
                ] {
                    if on {
                        attr |= bit;
                    }
                }
                wide |= c.is_wide();
                self.cells[i] = Cell {
                    ch,
                    extra: if rest.is_empty() { None } else { Some(rest.into()) },
                    fg: col(c.fgcolor()),
                    bg: col(c.bgcolor()),
                    attr,
                    width: if c.is_wide() { 2 } else { 1 },
                };
                self.transparent[i] = false;
                first = first.min(x);
                last = x;
            }
            // Short gaps inside a line are the spaces between words; keep
            // them clear so the animation never reads as part of the text.
            if first != usize::MAX {
                let mut x = first;
                while x <= last {
                    let start = x;
                    while x <= last && self.transparent[y * w + x] {
                        x += 1;
                    }
                    if x - start <= WORD_GAP {
                        self.transparent[y * w + start..y * w + x].fill(false);
                    }
                    x += 1;
                }
            }
            self.zone.push(if first == usize::MAX {
                (0, 0)
            } else {
                (first.saturating_sub(ZONE_MARGIN), (last + 1 + ZONE_MARGIN).min(w))
            });
            self.wide_rows.push(wide);
        }
    }
}

impl Layer {
    /// Writes a short highlighted note over the top-right corner.
    pub fn label(&mut self, text: &str, w: usize) {
        let n = text.chars().count();
        if n > w || self.cells.len() < w {
            return;
        }
        let start = w - n;
        // Do not leave half of a wide character behind.
        if self.cells[start].width == 0 && start > 0 {
            self.cells[start - 1] = BLANK;
        }
        for (i, ch) in text.chars().enumerate() {
            self.cells[start + i] = Cell { ch, attr: INVERSE, ..BLANK };
            self.transparent[start + i] = false;
        }
    }
}

pub struct Look {
    /// Overall animation brightness, 0..=1.
    pub intensity: f32,
    /// Extra dimming applied behind lines of text, 0..=1.
    pub text_dim: f32,
}

/// Lays the shell's cells over the animation into `frame`.
pub fn compose(
    layer: &Layer,
    anim: &dyn Animation,
    look: &Look,
    w: usize,
    h: usize,
    cursor: Option<(usize, usize)>,
    frame: &mut Vec<Cell>,
) {
    frame.clear();
    frame.reserve(w * h);
    for y in 0..h {
        let (mut z0, mut z1) = layer.zone[y];
        // Keep the area around the cursor calm too, so typing stays legible.
        if let Some((cy, cx)) = cursor {
            if cy == y {
                let (c0, c1) = (cx.saturating_sub(ZONE_MARGIN), (cx + 1 + ZONE_MARGIN).min(w));
                (z0, z1) = if z0 == z1 { (c0, c1) } else { (z0.min(c0), z1.max(c1)) };
            }
        }
        for x in 0..w {
            let i = y * w + x;
            if !layer.transparent[i] {
                frame.push(layer.cells[i].clone());
                continue;
            }
            let mut cell = BLANK;
            if cursor != Some((y, x)) {
                if let Some(s) = anim.sample(x, y) {
                    let k = look.intensity * if x >= z0 && x < z1 { look.text_dim } else { 1.0 };
                    let visible = |c: &[u8; 3]| c[0].max(c[1]).max(c[2]) >= 10;
                    let fg = crate::anim::scale(s.fg, k);
                    let bg = s.bg.map(|c| crate::anim::scale(c, k)).filter(visible);
                    // Too dark to see: leave the cell empty.
                    if visible(&fg) || bg.is_some() {
                        cell.ch = s.ch;
                        cell.fg = Col::Rgb(fg[0], fg[1], fg[2]);
                        cell.bg = bg.map_or(Col::Default, |c| Col::Rgb(c[0], c[1], c[2]));
                    }
                }
            }
            frame.push(cell);
        }
    }
}

pub struct Renderer {
    w: usize,
    h: usize,
    prev: Vec<Cell>,
    prev_wide: Vec<bool>,
    truecolor: bool,
    force: bool,
}

impl Renderer {
    pub fn new(w: usize, h: usize, truecolor: bool) -> Self {
        let mut r = Renderer { w: 0, h: 0, prev: Vec::new(), prev_wide: Vec::new(), truecolor, force: true };
        r.resize(w, h);
        r
    }

    /// Forget what is on screen; the next frame is drawn in full.
    pub fn resize(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.prev = vec![BLANK; w * h];
        self.prev_wide = vec![false; h];
        self.force = true;
    }

    /// Appends the escape sequences that turn the previous frame into
    /// `frame`, then keeps `frame` as the new previous one (the old one is
    /// handed back in its place for reuse).
    pub fn draw(&mut self, frame: &mut Vec<Cell>, wide_rows: &[bool], out: &mut Vec<u8>) {
        let (w, h) = (self.w, self.h);
        let mut pen: Option<(Col, Col, u8)> = None;
        let mut at: Option<(usize, usize)> = None;
        let mut utf8 = [0u8; 4];

        for y in 0..h {
            let row = y * w..(y + 1) * w;
            // Overwriting half of a wide character disturbs its other half,
            // so rows that hold any are redrawn whole when they change.
            let full = self.force
                || ((self.prev_wide[y] || wide_rows[y]) && frame[row.clone()] != self.prev[row]);
            let mut x = 0;
            while x < w {
                let c = &frame[y * w + x];
                if c.width == 0 {
                    x += 1;
                    continue;
                }
                if full || *c != self.prev[y * w + x] {
                    if at != Some((x, y)) {
                        out.extend_from_slice(format!("\x1b[{};{}H", y + 1, x + 1).as_bytes());
                    }
                    if pen != Some((c.fg, c.bg, c.attr)) {
                        self.sgr(c, pen, out);
                        pen = Some((c.fg, c.bg, c.attr));
                    }
                    out.extend_from_slice(c.ch.encode_utf8(&mut utf8).as_bytes());
                    if let Some(extra) = &c.extra {
                        out.extend_from_slice(extra.as_bytes());
                    }
                    let nx = x + c.width as usize;
                    // At the right edge the cursor position is ambiguous.
                    at = if nx < w { Some((nx, y)) } else { None };
                }
                x += c.width as usize;
            }
        }
        if pen.is_some() {
            out.extend_from_slice(b"\x1b[0m");
        }
        std::mem::swap(frame, &mut self.prev);
        self.prev_wide.copy_from_slice(wide_rows);
        self.force = false;
    }

    /// Switches the terminal's pen to this cell's colours and attributes,
    /// sending only what differs from the current pen where it can.
    fn sgr(&self, c: &Cell, pen: Option<(Col, Col, u8)>, out: &mut Vec<u8>) {
        let mut s = String::from("\x1b[");
        match pen {
            Some((fg, bg, attr)) if attr == c.attr => {
                if fg != c.fg {
                    self.color(&mut s, c.fg, 30);
                }
                if bg != c.bg {
                    self.color(&mut s, c.bg, 40);
                }
            }
            _ => {
                s.push_str("0;");
                for (bit, code) in
                    [(BOLD, "1;"), (DIM, "2;"), (ITALIC, "3;"), (UNDERLINE, "4;"), (INVERSE, "7;")]
                {
                    if c.attr & bit != 0 {
                        s.push_str(code);
                    }
                }
                if c.fg != Col::Default {
                    self.color(&mut s, c.fg, 30);
                }
                if c.bg != Col::Default {
                    self.color(&mut s, c.bg, 40);
                }
            }
        }
        s.pop(); // the trailing ';'
        s.push('m');
        out.extend_from_slice(s.as_bytes());
    }

    /// Appends the code for one colour, with a trailing ';'. `base` is 30
    /// for foreground, 40 for background.
    fn color(&self, s: &mut String, c: Col, base: u8) {
        use std::fmt::Write;
        let _ = match c {
            Col::Default => write!(s, "{};", base + 9),
            Col::Idx(i) if i < 8 => write!(s, "{};", base + i),
            Col::Idx(i) if i < 16 => write!(s, "{};", base + 60 + i - 8),
            Col::Idx(i) => write!(s, "{};5;{};", base + 8, i),
            Col::Rgb(r, g, b) if self.truecolor => write!(s, "{};2;{};{};{};", base + 8, r, g, b),
            Col::Rgb(r, g, b) => write!(s, "{};5;{};", base + 8, cube(r, g, b)),
        };
    }
}

/// Nearest colour in the xterm 6x6x6 cube, for terminals without truecolor.
fn cube(r: u8, g: u8, b: u8) -> u8 {
    let q = |v: u8| -> u8 {
        if v < 48 {
            0
        } else if v < 115 {
            1
        } else {
            (v - 35) / 40
        }
    };
    16 + 36 * q(r) + 6 * q(g) + q(b)
}
