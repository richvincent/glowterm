//! Arcade: original games in the style of late-70s home consoles (big square
//! pixels, a handful of flat colours on black) playing themselves. Every so
//! often the channel changes, through a burst of static, to another game.
//!
//! The games are generic genres with their own layouts and sprites; none
//! reproduces a particular published title.

use super::{hash, Animation, Rgb, Rng, Sample};

/// Game steps per second, independent of the frame rate.
const TICK: f32 = 1.0 / 20.0;

const WHITE: u8 = 1;
const RED: u8 = 2;
const ORANGE: u8 = 3;
const YELLOW: u8 = 4;
const GREEN: u8 = 5;
const TEAL: u8 = 6;
const BLUE: u8 = 7;
const PURPLE: u8 = 8;
const PINK: u8 = 9;
const GREY: u8 = 10;
const GRASS: u8 = 11;
const SHADOW: u8 = 12;
/// Colour 0 is "nothing here": the cell is left to the terminal.
const PALETTE: [Rgb; 13] = [
    [0, 0, 0],
    [236, 236, 236],
    [200, 72, 72],
    [214, 132, 48],
    [222, 200, 70],
    [80, 170, 70],
    [60, 180, 170],
    [70, 110, 220],
    [150, 90, 200],
    [220, 110, 170],
    [120, 120, 120],
    [20, 70, 30],
    [50, 50, 60],
];

/// The low-resolution picture the games draw on.
struct Frame {
    w: i32,
    h: i32,
    px: Vec<u8>,
}

impl Frame {
    fn new(w: usize, h: usize) -> Self {
        Frame { w: w as i32, h: h as i32, px: vec![0; w * h] }
    }
    fn clear(&mut self) {
        self.px.fill(0);
    }
    fn set(&mut self, x: i32, y: i32, c: u8) {
        if x >= 0 && x < self.w && y >= 0 && y < self.h {
            self.px[(y * self.w + x) as usize] = c;
        }
    }
    fn get(&self, x: i32, y: i32) -> u8 {
        if x >= 0 && x < self.w && y >= 0 && y < self.h { self.px[(y * self.w + x) as usize] } else { 0 }
    }
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: u8) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, c);
            }
        }
    }
    /// Draws a small picture given as rows of text, '#' for a filled pixel,
    /// centred horizontally on `x`.
    fn sprite(&mut self, x: i32, y: i32, rows: &[&str], c: u8) {
        for (dy, row) in rows.iter().enumerate() {
            for (dx, ch) in row.chars().enumerate() {
                if ch == '#' {
                    self.set(x + dx as i32 - row.len() as i32 / 2, y + dy as i32, c);
                }
            }
        }
    }
}

trait Game {
    /// Advances the game one step and redraws the frame.
    fn tick(&mut self, f: &mut Frame, rng: &mut Rng);
}

const GAMES: usize = 5;

fn start(which: usize, f: &Frame, rng: &mut Rng) -> Box<dyn Game> {
    match which {
        0 => Box::new(Volley::new(f, rng)),
        1 => Box::new(Breaker::new(f, rng)),
        2 => Box::new(Shooter::new(f)),
        3 => Box::new(Racer::new(f)),
        _ => Box::new(Trails::new(f, rng)),
    }
}

// ---------------------------------------------------------------- volley
// Two bats, one ball. Each bat aims slightly off, so points do get scored.

struct Volley {
    ball: (f32, f32),
    vel: (f32, f32),
    bats: [f32; 2],
    aim: [f32; 2],
    score: [i32; 2],
}

impl Volley {
    fn new(f: &Frame, rng: &mut Rng) -> Self {
        let mut v = Volley { ball: (0.0, 0.0), vel: (0.0, 0.0), bats: [f.h as f32 / 2.0; 2], aim: [0.0; 2], score: [0; 2] };
        v.serve(f, rng, 1.0);
        v
    }
    fn serve(&mut self, f: &Frame, rng: &mut Rng, direction: f32) {
        let (w, h) = (f.w as f32, f.h as f32);
        self.ball = (w / 2.0, rng.range(h * 0.3, h * 0.7));
        self.vel = (direction * w * 0.02, rng.range(-h * 0.02, h * 0.02));
    }
}

impl Game for Volley {
    fn tick(&mut self, f: &mut Frame, rng: &mut Rng) {
        let (w, h) = (f.w as f32, f.h as f32);
        let bat = (h / 6.0).max(3.0);
        for side in 0..2 {
            let coming = (side == 0) == (self.vel.0 < 0.0);
            let target = if coming { self.ball.1 + self.aim[side] } else { h / 2.0 };
            let step = (target - (self.bats[side] + bat / 2.0)).clamp(-h * 0.035, h * 0.035);
            self.bats[side] = (self.bats[side] + step).clamp(1.0, h - 1.0 - bat);
        }
        let before = self.ball.0;
        self.ball.0 += self.vel.0;
        self.ball.1 += self.vel.1;
        if self.ball.1 < 1.0 || self.ball.1 > h - 2.0 {
            self.vel.1 = -self.vel.1;
            self.ball.1 = self.ball.1.clamp(1.0, h - 2.0);
        }
        for (side, face) in [(0, 3.0), (1, w - 4.0)] {
            let crossed = if side == 0 { before > face && self.ball.0 <= face } else { before < face && self.ball.0 >= face };
            let off = self.ball.1 - (self.bats[side] + bat / 2.0);
            if crossed && off.abs() <= bat / 2.0 + 0.5 {
                self.vel.0 = (-self.vel.0 * 1.06).clamp(-w * 0.04, w * 0.04);
                self.vel.1 = (self.vel.1 + off / bat * h * 0.03).clamp(-h * 0.045, h * 0.045);
                self.ball.0 = face;
                // Decide how badly each side will misjudge the next return.
                self.aim = [rng.range(-bat * 0.62, bat * 0.62), rng.range(-bat * 0.62, bat * 0.62)];
            }
        }
        if self.ball.0 < 0.0 || self.ball.0 >= w {
            let winner = if self.ball.0 < 0.0 { 1 } else { 0 };
            self.score[winner] += 1;
            if self.score[winner] > 7 {
                self.score = [0; 2];
            }
            self.serve(f, rng, if winner == 0 { 1.0 } else { -1.0 });
        }

        f.clear();
        f.rect(0, 0, f.w, 1, TEAL);
        f.rect(0, f.h - 1, f.w, 1, TEAL);
        for y in (2..f.h - 2).step_by(4) {
            f.rect(f.w / 2, y, 1, 2, GREY);
        }
        for i in 0..self.score[0] {
            f.rect(f.w / 2 - 3 - i * 2, 2, 1, 2, ORANGE);
        }
        for i in 0..self.score[1] {
            f.rect(f.w / 2 + 3 + i * 2, 2, 1, 2, BLUE);
        }
        f.rect(2, self.bats[0] as i32, 1, bat as i32, ORANGE);
        f.rect(f.w - 3, self.bats[1] as i32, 1, bat as i32, BLUE);
        f.set(self.ball.0 as i32, self.ball.1 as i32, WHITE);
    }
}

// --------------------------------------------------------------- breaker
// A ball, a bat, and a wall of blocks laid out in one of three patterns.

struct Breaker {
    cols: i32,
    rows: i32,
    brick_w: i32,
    top: i32,
    bricks: Vec<u8>,
    ball: (f32, f32),
    vel: (f32, f32),
    bat: f32,
    aim: f32,
}

impl Breaker {
    fn new(f: &Frame, rng: &mut Rng) -> Self {
        let brick_w = if f.w < 60 { 3 } else { 4 };
        let mut b = Breaker {
            cols: f.w / brick_w,
            rows: (f.h / 8).clamp(3, 8),
            brick_w,
            top: f.h / 8,
            bricks: Vec::new(),
            ball: (0.0, 0.0),
            vel: (0.0, 0.0),
            bat: f.w as f32 / 2.0,
            aim: 0.0,
        };
        b.build(rng);
        b.launch(f, rng);
        b
    }
    fn build(&mut self, rng: &mut Rng) {
        let pattern = rng.below(3);
        self.bricks.clear();
        for r in 0..self.rows {
            for c in 0..self.cols {
                let keep = match pattern {
                    0 => (r + c) % 2 == 0,
                    1 => c >= r && c < self.cols - r,
                    _ => c % 3 != 2,
                };
                let colour = [TEAL, PURPLE, YELLOW, PINK][((r + c / 2) % 4) as usize];
                self.bricks.push(if keep { colour } else { 0 });
            }
        }
    }
    fn launch(&mut self, f: &Frame, rng: &mut Rng) {
        self.ball = (self.bat, f.h as f32 - 5.0);
        self.vel = (rng.range(-0.012, 0.012) * f.w as f32, -0.03 * f.h as f32);
    }
    /// Knocks out the block at this point, if there is one.
    fn hit(&mut self, x: f32, y: f32) -> bool {
        let (c, r) = (x as i32 / self.brick_w, (y as i32 - self.top).div_euclid(2));
        if y < self.top as f32 || r >= self.rows || c < 0 || c >= self.cols {
            return false;
        }
        let brick = &mut self.bricks[(r * self.cols + c) as usize];
        std::mem::take(brick) != 0
    }
}

impl Game for Breaker {
    fn tick(&mut self, f: &mut Frame, rng: &mut Rng) {
        let (w, h) = (f.w as f32, f.h as f32);
        let bat_w = (w / 9.0).max(5.0);
        let bat_y = h - 3.0;
        self.bat = (self.bat + (self.ball.0 + self.aim - self.bat).clamp(-w * 0.03, w * 0.03)).clamp(bat_w / 2.0, w - bat_w / 2.0);

        self.ball.0 += self.vel.0;
        if self.ball.0 < 0.0 || self.ball.0 >= w || self.hit(self.ball.0, self.ball.1) {
            self.ball.0 -= self.vel.0;
            self.vel.0 = -self.vel.0;
        }
        let before = self.ball.1;
        self.ball.1 += self.vel.1;
        if self.ball.1 < 0.0 || self.hit(self.ball.0, self.ball.1) {
            self.ball.1 -= self.vel.1;
            self.vel.1 = -self.vel.1;
        }
        let off = self.ball.0 - self.bat;
        if self.vel.1 > 0.0 && before < bat_y && self.ball.1 >= bat_y && off.abs() <= bat_w / 2.0 + 0.5 {
            self.vel.1 = -self.vel.1;
            self.vel.0 = off / bat_w * w * 0.04;
            self.ball.1 = bat_y - 0.01;
            self.aim = rng.range(-bat_w * 0.45, bat_w * 0.45);
        }
        if self.ball.1 >= h {
            self.launch(f, rng);
        }
        if self.bricks.iter().all(|&b| b == 0) {
            self.build(rng);
        }

        f.clear();
        for r in 0..self.rows {
            for c in 0..self.cols {
                let colour = self.bricks[(r * self.cols + c) as usize];
                if colour != 0 {
                    f.rect(c * self.brick_w, self.top + r * 2, self.brick_w - 1, 1, colour);
                }
            }
        }
        f.rect((self.bat - bat_w / 2.0) as i32, bat_y as i32, bat_w as i32, 1, ORANGE);
        f.set(self.ball.0 as i32, self.ball.1 as i32, WHITE);
    }
}

// --------------------------------------------------------------- shooter
// A ship at the bottom picks off craft that weave down from the top.

const SHIP: [&str; 3] = ["  #  ", " ### ", "#####"];
const CRAFT: [[&str; 3]; 3] =
    [["# # #", " ### ", "#   #"], [" ### ", "## ##", " # # "], ["#   #", "#####", " # # "]];

struct Foe {
    centre: f32,
    swing: f32,
    rate: f32,
    fall: f32,
    age: f32,
    kind: usize,
}

impl Foe {
    fn at(&self) -> (f32, f32) {
        (self.centre + self.swing * (self.age * self.rate).sin(), self.age * self.fall)
    }
}

struct Shooter {
    ship: f32,
    foes: Vec<Foe>,
    shots: Vec<(f32, f32)>,
    /// Explosions: position and steps left.
    bursts: Vec<(f32, f32, u8)>,
    reload: u8,
    next_foe: u8,
    drift: f32,
}

impl Shooter {
    fn new(f: &Frame) -> Self {
        Shooter { ship: f.w as f32 / 2.0, foes: Vec::new(), shots: Vec::new(), bursts: Vec::new(), reload: 0, next_foe: 0, drift: 0.0 }
    }
}

impl Game for Shooter {
    fn tick(&mut self, f: &mut Frame, rng: &mut Rng) {
        let (w, h) = (f.w as f32, f.h as f32);
        self.drift += 0.5;
        if self.next_foe == 0 {
            let swing = rng.range(3.0, w * 0.2);
            self.foes.push(Foe {
                centre: rng.range(swing + 3.0, (w - swing - 3.0).max(swing + 4.0)),
                swing,
                rate: rng.range(0.05, 0.15),
                fall: h * rng.range(0.006, 0.014),
                age: 0.0,
                kind: rng.below(CRAFT.len()),
            });
            self.next_foe = 12 + rng.below(20) as u8;
        }
        self.next_foe -= 1;
        for foe in self.foes.iter_mut() {
            foe.age += 1.0;
        }
        self.foes.retain(|foe| foe.at().1 < h - 4.0);

        // Chase whichever craft is lowest, and fire when lined up.
        if let Some(target) = self.foes.iter().map(Foe::at).max_by(|a, b| a.1.total_cmp(&b.1)) {
            self.ship = (self.ship + (target.0 - self.ship).clamp(-w * 0.02, w * 0.02)).clamp(3.0, w - 3.0);
        }
        self.reload = self.reload.saturating_sub(1);
        if self.reload == 0 && self.foes.iter().any(|foe| (foe.at().0 - self.ship).abs() < 3.0) {
            self.shots.push((self.ship, h - 5.0));
            self.reload = 6;
        }
        for shot in self.shots.iter_mut() {
            shot.1 -= h * 0.05;
        }
        let Self { foes, shots, bursts, .. } = self;
        shots.retain(|shot| {
            let struck = foes.iter().position(|foe| {
                let (x, y) = foe.at();
                (x - shot.0).abs() <= 2.5 && (y + 1.0 - shot.1).abs() <= h * 0.03 + 1.5
            });
            if let Some(i) = struck {
                let (x, y) = foes.remove(i).at();
                bursts.push((x, y, 5));
            }
            struck.is_none() && shot.1 > 0.0
        });
        for burst in bursts.iter_mut() {
            burst.2 -= 1;
        }
        bursts.retain(|burst| burst.2 > 0);

        f.clear();
        for y in 0..f.h {
            for x in 0..f.w {
                if hash(x, y - self.drift as i32, 3) % 97 == 0 {
                    f.set(x, y, SHADOW);
                }
            }
        }
        for foe in &self.foes {
            let (x, y) = foe.at();
            f.sprite(x as i32, y as i32, &CRAFT[foe.kind], [GREEN, PINK, YELLOW][foe.kind]);
        }
        for &(x, y, left) in &self.bursts {
            let reach = 5 - left as i32;
            for (dx, dy) in [(-1, -1), (1, -1), (-1, 1), (1, 1), (0, 0)] {
                f.set(x as i32 + dx * reach, y as i32 + 1 + dy * reach, if left > 2 { WHITE } else { ORANGE });
            }
        }
        for &(x, y) in &self.shots {
            f.rect(x as i32, y as i32, 1, 2, WHITE);
        }
        f.sprite(self.ship as i32, f.h - 4, &SHIP, TEAL);
    }
}

// ----------------------------------------------------------------- racer
// A winding road seen from above; the car threads through slower traffic.

struct Racer {
    travelled: f32,
    car: f32,
    lane: f32,
    /// Other cars: lane (-1, 0 or 1), distance along the road, colour.
    traffic: Vec<(f32, f32, u8)>,
    next_car: u8,
}

impl Racer {
    fn new(f: &Frame) -> Self {
        Racer { travelled: 0.0, car: f.w as f32 / 2.0, lane: 0.0, traffic: Vec::new(), next_car: 10 }
    }
}

impl Game for Racer {
    fn tick(&mut self, f: &mut Frame, rng: &mut Rng) {
        let (w, h) = (f.w as f32, f.h as f32);
        let half = w * 0.2;
        let centre = |d: f32| w / 2.0 + w * 0.14 * (d * 0.035).sin() + w * 0.07 * (d * 0.013 + 1.0).sin();
        self.travelled += 1.0;
        let here = self.travelled + 7.0;

        if self.next_car == 0 {
            let colour = [YELLOW, PINK, BLUE, PURPLE][rng.below(4)];
            self.traffic.push((rng.below(3) as f32 - 1.0, self.travelled + h + 5.0, colour));
            self.next_car = 18 + rng.below(25) as u8;
        }
        self.next_car -= 1;
        for car in self.traffic.iter_mut() {
            car.1 += 0.45;
        }
        self.traffic.retain(|car| car.1 > self.travelled - 5.0);

        // Take whichever lane has the most clear road ahead, staying put on a tie.
        let clear = |lane: f32| {
            self.traffic
                .iter()
                .filter(|c| c.0 == lane && c.1 > here - 4.0)
                .map(|c| c.1 - here)
                .fold(f32::MAX, f32::min)
        };
        if clear(self.lane) < 20.0 {
            self.lane = [-1.0, 0.0, 1.0].into_iter().max_by(|a, b| clear(*a).total_cmp(&clear(*b))).unwrap_or(0.0);
        }
        let want = centre(here) + self.lane * half * 0.55;
        self.car += (want - self.car).clamp(-w * 0.025, w * 0.025);

        f.clear();
        for y in 0..f.h {
            let d = self.travelled + (f.h - y) as f32;
            let c = centre(d);
            let (left, right) = ((c - half) as i32, (c + half) as i32);
            f.rect(0, y, left, 1, GRASS);
            f.rect(right + 1, y, f.w - right - 1, 1, GRASS);
            let kerb = if (d / 3.0) as i32 % 2 == 0 { RED } else { WHITE };
            f.set(left, y, kerb);
            f.set(right, y, kerb);
            if (d / 4.0) as i32 % 2 == 0 {
                f.set(c as i32, y, GREY);
            }
        }
        let body = |f: &mut Frame, x: f32, y: i32, colour: u8| {
            f.rect(x as i32 - 1, y, 3, 4, colour);
            f.set(x as i32, y + 1, SHADOW);
        };
        for &(lane, d, colour) in &self.traffic {
            body(f, centre(d) + lane * half * 0.55, f.h - (d - self.travelled) as i32, colour);
        }
        body(f, self.car, f.h - 7, ORANGE);
    }
}

// ---------------------------------------------------------------- trails
// Two lines race around the arena, each leaving a wall the other must avoid.

const STEPS: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

struct Trails {
    w: i32,
    h: i32,
    walls: Vec<u8>,
    /// Position and heading of each line.
    heads: [(i32, i32, usize); 2],
    /// Steps left to show the crash before the next round.
    pause: u8,
}

impl Trails {
    fn new(f: &Frame, rng: &mut Rng) -> Self {
        let mut t = Trails { w: f.w, h: f.h, walls: Vec::new(), heads: [(0, 0, 0); 2], pause: 0 };
        t.reset(rng);
        t
    }
    fn reset(&mut self, rng: &mut Rng) {
        self.walls = vec![0; (self.w * self.h) as usize];
        let y = self.h / 4 + rng.below((self.h / 2).max(1) as usize) as i32;
        self.heads = [(self.w / 5, y, 0), (self.w * 4 / 5, self.h - 1 - y, 2)];
    }
    fn free(&self, x: i32, y: i32) -> bool {
        x > 0 && y > 0 && x < self.w - 1 && y < self.h - 1 && self.walls[(y * self.w + x) as usize] == 0
    }
    /// How far a line could go in this direction before hitting something.
    fn room(&self, (x, y, _): (i32, i32, usize), dir: usize) -> i32 {
        (1..=14).take_while(|k| self.free(x + STEPS[dir].0 * k, y + STEPS[dir].1 * k)).count() as i32
    }
}

impl Game for Trails {
    fn tick(&mut self, f: &mut Frame, rng: &mut Rng) {
        if self.pause > 0 {
            self.pause -= 1;
            if self.pause == 0 {
                self.reset(rng);
            }
        } else {
            for i in 0..2 {
                let head = self.heads[i];
                let mut dir = head.2;
                if self.room(head, dir) == 0 || rng.below(40) == 0 {
                    let jitter = rng.below(3) as i32;
                    dir = [dir, (dir + 1) % 4, (dir + 3) % 4]
                        .into_iter()
                        .max_by_key(|&d| self.room(head, d) * 4 + (d as i32 + jitter) % 3)
                        .unwrap_or(dir);
                }
                if self.room(head, dir) == 0 {
                    self.pause = 25;
                    break;
                }
                self.walls[(head.1 * self.w + head.0) as usize] = if i == 0 { TEAL } else { PINK };
                self.heads[i] = (head.0 + STEPS[dir].0, head.1 + STEPS[dir].1, dir);
            }
        }

        f.clear();
        f.px.copy_from_slice(&self.walls);
        f.rect(0, 0, f.w, 1, GREY);
        f.rect(0, f.h - 1, f.w, 1, GREY);
        f.rect(0, 0, 1, f.h, GREY);
        f.rect(f.w - 1, 0, 1, f.h, GREY);
        let flash = self.pause > 0 && self.pause % 4 < 2;
        for head in self.heads {
            f.set(head.0, head.1, if flash { RED } else { WHITE });
        }
    }
}

// ------------------------------------------------------------- the "set"

pub struct Arcade {
    w: usize,
    /// Screen columns (and half-rows) per game pixel.
    zoom: usize,
    frame: Frame,
    game: Box<dyn Game>,
    which: usize,
    /// Seconds until the channel changes.
    remaining: f32,
    static_ticks: u8,
    pending: f32,
    rng: Rng,
}

impl Arcade {
    pub fn new(w: usize, h: usize, _density: f32) -> Self {
        let mut rng = Rng::new();
        let frame = Frame::new(1, 1);
        let game = start(0, &frame, &mut rng);
        let mut a = Arcade { w: 0, zoom: 1, frame, game, which: 0, remaining: 0.0, static_ticks: 0, pending: 0.0, rng };
        a.resize(w, h);
        a
    }

    fn change_channel(&mut self) {
        self.which = (self.which + 1 + self.rng.below(GAMES - 1)) % GAMES;
        self.game = start(self.which, &self.frame, &mut self.rng);
        self.remaining = self.rng.range(18.0, 30.0);
    }
}

impl Animation for Arcade {
    fn resize(&mut self, w: usize, h: usize) {
        self.w = w;
        self.zoom = (w / 80).clamp(1, 4);
        self.frame = Frame::new(w.div_ceil(self.zoom).max(8), (h * 2 / self.zoom).max(8));
        self.which = self.rng.below(GAMES);
        self.change_channel();
    }

    fn step(&mut self, dt: f32) {
        self.pending = (self.pending + dt).min(4.0 * TICK);
        while self.pending >= TICK {
            self.pending -= TICK;
            if self.static_ticks > 0 {
                self.static_ticks -= 1;
                for p in self.frame.px.iter_mut() {
                    *p = [0, 0, 0, SHADOW, GREY, WHITE][self.rng.below(6)];
                }
                continue;
            }
            self.game.tick(&mut self.frame, &mut self.rng);
            self.remaining -= TICK;
            if self.remaining <= 0.0 {
                self.change_channel();
                self.static_ticks = 6;
            }
        }
    }

    fn sample(&self, x: usize, y: usize) -> Option<Sample> {
        // Each cell shows two game pixels, one above the other.
        let (px, top, bottom) = ((x / self.zoom) as i32, (y * 2 / self.zoom) as i32, ((y * 2 + 1) / self.zoom) as i32);
        let (a, b) = (self.frame.get(px, top), self.frame.get(px, bottom));
        match (a, b) {
            (0, 0) => None,
            (a, 0) => Some(Sample::glyph('▀', PALETTE[a as usize])),
            (0, b) => Some(Sample::glyph('▄', PALETTE[b as usize])),
            (a, b) if a == b => Some(Sample::glyph('█', PALETTE[a as usize])),
            (a, b) => Some(Sample { ch: '▀', fg: PALETTE[a as usize], bg: Some(PALETTE[b as usize]) }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every game runs for a minute at several screen sizes without
    /// panicking and keeps something on screen. Run with `--nocapture` to
    /// see a frame of each.
    #[test]
    fn games_run() {
        let mut rng = Rng::new();
        for (w, h) in [(100, 60), (80, 48), (20, 12), (8, 8)] {
            for which in 0..GAMES {
                let mut frame = Frame::new(w, h);
                let mut game = start(which, &frame, &mut rng);
                let mut lit = 0;
                for _ in 0..1200 {
                    game.tick(&mut frame, &mut rng);
                    lit += frame.px.iter().filter(|&&p| p != 0).count();
                }
                assert!(lit > 1200, "game {which} at {w}x{h} drew almost nothing");
                if (w, h) == (80, 48) {
                    println!("--- game {which}");
                    for y in (0..h).step_by(2) {
                        let row: String = (0..w)
                            .map(|x| b" #RoygtbpkG.:"[frame.get(x as i32, y as i32) as usize] as char)
                            .collect();
                        println!("{row}");
                    }
                }
            }
        }
    }
}
