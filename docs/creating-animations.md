# Creating an animated background

An animation is one Rust file in `src/anim/` plus three lines in `src/anim/mod.rs`. It never touches the terminal: it only answers the question "what should be in this empty cell right now?". glowterm handles the shell, the text, dimming and drawing.

This guide builds a complete animation, `fireflies`, then covers the rules that keep an animation good-looking and cheap.

## The contract

```rust
pub trait Animation {
    /// The grid changed size; rebuild any per-cell state.
    fn resize(&mut self, w: usize, h: usize);
    /// Advance the simulation by `dt` seconds.
    fn step(&mut self, dt: f32);
    /// What to draw in an empty cell, or `None` to leave it dark.
    fn sample(&self, x: usize, y: usize) -> Option<Sample>;
}

pub struct Sample {
    pub ch: char,          // the character to show
    pub fg: Rgb,           // its colour, [r, g, b]
    pub bg: Option<Rgb>,   // background colour, usually None
}
```

- `resize` is called once at start-up (from your constructor) and again whenever the window changes size. `w` and `h` are in character cells.
- `step` is called once per frame, with the real time elapsed (capped at 0.25 s). Do all the work here.
- `sample` is called for every empty cell, every frame, so keep it to a lookup. `x` runs left to right from 0, `y` top to bottom from 0.

`Sample::glyph(ch, fg)` builds a sample with no background.

## A complete example

Create `src/anim/fireflies.rs`:

```rust
//! Fireflies: soft points of light that drift and pulse.

use super::{scale, Animation, Rgb, Rng, Sample};

const GLOW: Rgb = [200, 255, 120];
/// Brightness is rounded to this many steps, so a fly is redrawn only when
/// it visibly changes.
const LEVELS: f32 = 6.0;

struct Fly {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    phase: f32,
}

pub struct Fireflies {
    w: usize,
    h: usize,
    density: f32,
    flies: Vec<Fly>,
    cells: Vec<Option<Sample>>,
    rng: Rng,
}

impl Fireflies {
    pub fn new(w: usize, h: usize, density: f32) -> Self {
        let mut f = Fireflies { w: 0, h: 0, density, flies: Vec::new(), cells: Vec::new(), rng: Rng::new() };
        f.resize(w, h);
        f
    }
}

impl Animation for Fireflies {
    fn resize(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.cells = vec![None; w * h];
        let count = ((w * h) as f32 / 150.0 * self.density) as usize;
        let rng = &mut self.rng;
        self.flies = (0..count)
            .map(|_| Fly {
                x: rng.range(0.0, w as f32),
                y: rng.range(0.0, h as f32),
                // Cells are about twice as tall as wide, so move half as
                // fast vertically to look the same speed on screen.
                dx: rng.range(-3.0, 3.0),
                dy: rng.range(-1.5, 1.5),
                phase: rng.range(0.0, 6.28),
            })
            .collect();
    }

    fn step(&mut self, dt: f32) {
        self.cells.fill(None);
        if self.w == 0 || self.h == 0 {
            return;
        }
        let (w, h) = (self.w as f32, self.h as f32);
        for fly in self.flies.iter_mut() {
            fly.x = (fly.x + fly.dx * dt).rem_euclid(w);
            fly.y = (fly.y + fly.dy * dt).rem_euclid(h);
            fly.phase += dt * 1.5;
            let level = ((0.5 + 0.5 * fly.phase.sin()) * LEVELS).round() / LEVELS;
            if level > 0.0 {
                let ch = if level > 0.7 { '*' } else { '·' };
                let (x, y) = ((fly.x as usize).min(self.w - 1), (fly.y as usize).min(self.h - 1));
                self.cells[y * self.w + x] = Some(Sample::glyph(ch, scale(GLOW, level)));
            }
        }
    }

    fn sample(&self, x: usize, y: usize) -> Option<Sample> {
        self.cells.get(y * self.w + x).copied().flatten()
    }
}
```

Then register it in `src/anim/mod.rs`, in three places:

```rust
mod fireflies;                                            // 1. with the other `mod` lines

pub const ALL: &[(&str, &str)] = &[
    // ...
    ("fireflies", "Soft points of light that drift and pulse"),   // 2. name and description
];

pub fn create(name: &str, w: usize, h: usize, density: f32) -> Option<Box<dyn Animation>> {
    match name {
        // ...
        "fireflies" => Some(Box::new(fireflies::Fireflies::new(w, h, density))),  // 3. constructor
        _ => None,
    }
}
```

Build and run:

```
cargo build --release
./target/release/glowterm -a fireflies
```

It now also appears in `--list` and in the Ctrl+] cycle, in the position you gave it in `ALL`.

The pattern in this example is the one every built-in animation uses: simulate in `step`, write the picture into a `Vec<Option<Sample>>` the size of the screen, and have `sample` read from it.

## Helpers

All in `src/anim/mod.rs`:

| Helper | Use |
|---|---|
| `Rng::new()`, `.unit()`, `.range(lo, hi)`, `.below(n)` | Random numbers: 0 to 1, a float range, an integer below `n` |
| `hash(a, b, c) -> u32` | A repeatable pseudo-random number from three integers. Use it for detail that must stay put between frames (which windows are lit, where the stars are) |
| `scale(colour, k)` | Dim or brighten a colour |
| `lerp(a, b, t)` | Blend two colours |
| `Sample::glyph(ch, fg)` | A sample with no background colour |

## Rules

### Never panic

glowterm restores the terminal if an animation panics, but the session is lost. Your code must survive any size, including 1×1, and a resize at any moment.

- Index with `.get()` in `sample`, as above.
- Guard against `w == 0` or `h == 0` before dividing or subtracting.
- Clamp any coordinate computed from floating point before using it as an index.
- Rebuild every per-cell buffer in `resize`.

### Use only single-width characters

Every character must occupy exactly one cell. ASCII, half-width katakana (U+FF66 to U+FF9D), box-drawing characters and block elements are safe. Emoji and full-width CJK characters are two cells wide and will corrupt the layout.

### Return `None` for dark

Do not return black or near-black samples; return `None`. Anything dimmer than about 10 out of 255 is dropped anyway, and `None` costs nothing to draw.

### Remember cells are tall

A character cell is roughly twice as tall as it is wide. Halve vertical speeds and distances (or double horizontal ones) when you want motion or shapes to look even, as `starfield` and `fireflies` do.

### Spend bytes carefully

This is the rule that matters most. Every cell that changes between two frames costs roughly 10 to 40 bytes sent to the terminal, which then has to parse and draw it. A full 200×60 screen changing every frame at 20 fps would be about 10 MB/s. Aim for **under about 1 MB/s at 200×60**; the built-in animations range from 3 KB/s to 1.1 MB/s.

Ways to stay inside the budget:

- **Round colours to a few steps.** If brightness is rounded to 8 or 12 levels, a slowly changing cell is redrawn only when it crosses a step, not every frame. Every built-in animation does this (`LEVELS` in `matrix`, `aurora`, `fireflies` above).
- **Leave most of the screen dark or still.** Sparse animations (`starfield`, `nightride`) are the cheapest.
- **Move slowly, or in whole cells.** A pattern that drifts a fraction of a cell per frame can change every cell every frame; one that moves one cell every few frames does not.
- **Update the scene less often than the frame rate** when it is expensive. Accumulate time and advance in fixed ticks:

  ```rust
  const TICK: f32 = 1.0 / 12.0;

  fn step(&mut self, dt: f32) {
      self.pending = (self.pending + dt).min(4.0 * TICK);
      while self.pending >= TICK {
          self.pending -= TICK;
          self.tick();            // advance and redraw the buffer
      }
  }
  ```

  `fire`, `cyberspace` and `arcade` work this way. It also makes the animation run at the same speed whatever `--fps` is.
- **Keep runs of the same colour together.** Neighbouring changed cells with the same colour share one colour command.

Measure rather than guess:

```
cargo build --release
tools/measure.py fireflies            # 200x60 for 8 seconds
tools/measure.py fireflies 24 80 5    # rows, columns, seconds
```

It prints the output rate and CPU time and saves a capture you can view as text:

```
cargo run --release --example snap 60 200 < /tmp/glowterm-fireflies.bin
```

That text picture is also the quickest way to check the layout of what you drew without judging it by eye in motion.

### Make time-based motion, not frame-based

Multiply speeds by `dt` (or use fixed ticks). Never assume `step` is called 20 times a second: the user can set `--fps` anywhere from 1 to 60.

### Respect `density`

`density` is the user's "how busy" control: 1.0 is your default look, higher means more. Map it to whatever makes sense (number of particles, coverage, height) and clamp it so extreme values cannot break anything. If it has no sensible meaning for your animation, ignore it.

## Filling cells with colour

For solid areas, use a block character in the foreground colour rather than a background colour: `'█'` fills the cell.

To get two "pixels" per cell, stacked vertically, use a half block with a background:

```rust
Some(Sample { ch: '▀', fg: top_colour, bg: Some(bottom_colour) })
```

`fire` and `arcade` do this. If only the top pixel is lit use `'▀'` with no background; if only the bottom, `'▄'`.

## What you get for free

- **Text always wins.** Your samples are only requested for cells the shell has left empty.
- **Dimming behind text** (`--text-dim`) and overall brightness (`--intensity`) are applied to your colours for you, including the background colour.
- **256-colour fallback** is handled by the renderer.
- **Changing only what changed** is the renderer's job; you just describe the current picture.

## Testing

Logic that can be checked without a terminal deserves a unit test in the same file; `arcade.rs` has one that plays every game for a minute at several sizes, including 8×8, to catch panics. Run tests with:

```
cargo test --release
```

## Checklist

- [ ] One file in `src/anim/`, registered in three places in `mod.rs`
- [ ] Survives 1×1 and repeated resizes
- [ ] Only single-width characters
- [ ] Colours rounded to a small number of steps
- [ ] Speeds scaled by `dt` or driven by fixed ticks
- [ ] `tools/measure.py` shows about 1 MB/s or less at 200×60
- [ ] Looked at in a real terminal, with text on screen, at `--text-dim` 0.3 and 0
