//! Development aid: reads raw terminal output on stdin, replays it through a
//! terminal emulator, and prints the resulting screen as plain text.
//! Usage: snap ROWS COLS < capture.bin
use std::io::Read;
use std::num::NonZeroU16;

fn main() {
    let mut a = std::env::args().skip(1).map(|v| v.parse::<u16>().unwrap_or(24));
    let rows = NonZeroU16::new(a.next().unwrap_or(24)).unwrap();
    let cols = NonZeroU16::new(a.next().unwrap_or(80)).unwrap();
    let mut data = Vec::new();
    std::io::stdin().read_to_end(&mut data).unwrap();
    // Stop before the alternate screen is left, so the last frame survives.
    let end = data.windows(8).rposition(|w| w == b"\x1b[?1049l").unwrap_or(data.len());
    let mut p = vt100::Parser::new(rows, cols, 0);
    p.process(&data[..end]);
    println!("{}", p.screen().contents());
    let (r, c) = p.screen().cursor_position();
    eprintln!("cursor at row {r} col {c}; alt screen {}", p.screen().alternate_screen());
}
