//! glowterm: your shell, with a light show behind it.
//!
//! The shell runs in a pseudo-terminal. Its output is fed to an in-memory
//! terminal emulator, and every frame the emulator's screen is laid over an
//! animation, which shows through wherever the shell has left a cell empty.

mod anim;
mod render;

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use render::{compose, Layer, Look, Renderer};
use std::io::{Read, Write};
use std::num::NonZeroU16;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{sync_channel, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

type Error = Box<dyn std::error::Error + Send + Sync>;

const HELP: &str = "\
glowterm - your shell, with a light show behind it

USAGE:
    glowterm [OPTIONS] [-- COMMAND [ARGS...]]

Runs COMMAND (default: your login shell) with an animation behind its text.
Exit the shell to leave.

Ctrl+] switches to the next animation while running.

The mouse wheel scrolls back through earlier output; typing returns to the
prompt. In full-screen programs that do not use the mouse themselves (less,
man) the wheel sends arrow keys. To select text, hold your terminal's
override key while dragging (Option in iTerm2; Shift in many others).

OPTIONS:
    -a, --anim NAME       Animation to run (default: matrix)
    -l, --list            List the animations
    -i, --intensity N     Animation brightness, 0.0-1.0 (default: 1.0)
    -t, --text-dim N      Brightness kept behind and just around text,
                          0.0-1.0 (default: 0.3; 0 clears it completely)
    -d, --density N       How busy the animation is (default: 1.0; the
                          arcade animation ignores it)
    -f, --fps N           Animation frames per second, 1-60 (default: 20)
    -s, --scroll N        Lines the mouse wheel scrolls per notch (default: 3)
        --scrollback N    Lines of history to keep (default: 10000)
        --no-mouse        Leave the mouse to the terminal: plain click-drag
                          selection works, but the wheel cannot scroll
        --no-hotkey       Pass Ctrl+] through to the program instead of
                          using it to switch animation
        --256             Use 256 colours even if truecolor is available
    -h, --help            Show this help
";

struct Opts {
    anim: String,
    intensity: f32,
    text_dim: f32,
    density: f32,
    fps: f32,
    force_256: bool,
    scroll_lines: i32,
    scrollback: usize,
    mouse: bool,
    hotkey: bool,
    command: Vec<String>,
}

fn parse_args() -> Result<Option<Opts>, Error> {
    let mut o = Opts {
        anim: "matrix".into(),
        intensity: 1.0,
        text_dim: 0.3,
        density: 1.0,
        fps: 20.0,
        force_256: false,
        scroll_lines: 3,
        scrollback: 10_000,
        mouse: true,
        hotkey: true,
        command: Vec::new(),
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut num = |name: &str| -> Result<f32, Error> {
            let v = args.next().ok_or(format!("{name} needs a value"))?;
            match v.parse::<f32>() {
                Ok(n) if n.is_finite() => Ok(n),
                _ => Err(format!("{name}: '{v}' is not a number").into()),
            }
        };
        match a.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(None);
            }
            "-l" | "--list" => {
                for (name, about) in anim::ALL {
                    println!("{name:<10} {about}");
                }
                return Ok(None);
            }
            "-i" | "--intensity" => o.intensity = num("--intensity")?.clamp(0.0, 1.0),
            "-t" | "--text-dim" => o.text_dim = num("--text-dim")?.clamp(0.0, 1.0),
            "-d" | "--density" => o.density = num("--density")?,
            "-f" | "--fps" => o.fps = num("--fps")?.clamp(1.0, 60.0),
            "--256" => o.force_256 = true,
            "-s" | "--scroll" => o.scroll_lines = num("--scroll")?.clamp(1.0, 100.0) as i32,
            "--scrollback" => o.scrollback = num("--scrollback")?.clamp(0.0, 1e6) as usize,
            "--no-mouse" => o.mouse = false,
            "--no-hotkey" => o.hotkey = false,
            "-a" | "--anim" => o.anim = args.next().ok_or("--anim needs a name")?,
            "--" => {
                o.command = args.collect();
                break;
            }
            other => return Err(format!("unknown option '{other}' (try --help)").into()),
        }
    }
    Ok(Some(o))
}

// ---------------------------------------------------------------- terminal

static ORIGINAL_TERMIOS: OnceLock<libc::termios> = OnceLock::new();

/// Undoes everything glowterm, or the program inside it, may have switched
/// on in the real terminal.
const RESTORE: &[u8] = b"\x1b[?2026l\x1b[0m\x1b[?9l\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1005l\x1b[?1006l\
\x1b[?1004l\x1b[?2004l\x1b[?1l\x1b>\x1b[0 q\x1b[?25h\x1b[?1049l";

fn write_stdout(mut buf: &[u8]) {
    while !buf.is_empty() {
        let n = unsafe { libc::write(1, buf.as_ptr().cast(), buf.len()) };
        if n > 0 {
            buf = &buf[n as usize..];
            continue;
        }
        match std::io::Error::last_os_error().kind() {
            std::io::ErrorKind::Interrupted => {}
            std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(1)),
            _ => return,
        }
    }
}

fn enter_raw_mode() -> Result<(), Error> {
    unsafe {
        let mut t: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(0, &mut t) != 0 {
            return Err("could not read terminal settings".into());
        }
        let _ = ORIGINAL_TERMIOS.set(t);
        let mut raw = t;
        libc::cfmakeraw(&mut raw);
        if libc::tcsetattr(0, libc::TCSANOW, &raw) != 0 {
            return Err("could not switch the terminal to raw mode".into());
        }
    }
    write_stdout(b"\x1b[?1049h\x1b[2J\x1b[H");
    Ok(())
}

fn restore_terminal() {
    write_stdout(RESTORE);
    if let Some(t) = ORIGINAL_TERMIOS.get() {
        unsafe { libc::tcsetattr(0, libc::TCSANOW, t) };
    }
}

fn terminal_size() -> (u16, u16) {
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    let ok = unsafe { libc::ioctl(1, libc::TIOCGWINSZ, &mut ws) } == 0;
    if ok && ws.ws_row > 0 && ws.ws_col > 0 {
        (ws.ws_row, ws.ws_col)
    } else {
        (24, 80)
    }
}

/// The emulator library can panic on wide characters when it is one column
/// wide, so it is never made narrower than this, whatever the window is.
const MIN_EMULATOR_COLS: u16 = 2;

fn nz(v: u16) -> NonZeroU16 {
    NonZeroU16::new(v.max(1)).unwrap_or(NonZeroU16::MIN)
}

// ------------------------------------------------------ emulator callbacks

/// Collects what the emulator cannot deal with itself: queries the program
/// expects an answer to, and settings that belong to the real terminal.
#[derive(Default)]
struct Hooks {
    /// Bytes to send back to the program, as if typed by the terminal.
    reply: Vec<u8>,
    /// Bytes to hand on to the real terminal.
    pass: Vec<u8>,
}

impl Hooks {
    fn osc(&mut self, parts: &[&[u8]]) {
        self.pass.extend_from_slice(b"\x1b]");
        self.pass.extend_from_slice(&parts.join(&b';'));
        self.pass.push(0x07);
    }
}

impl vt100::Callbacks for Hooks {
    fn audible_bell(&mut self, _: &mut vt100::Screen) {
        self.pass.push(0x07);
    }
    fn set_window_icon_name(&mut self, _: &mut vt100::Screen, name: &[u8]) {
        self.osc(&[b"1", name]);
    }
    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        self.osc(&[b"2", title]);
    }
    fn copy_to_clipboard(&mut self, _: &mut vt100::Screen, ty: &[u8], data: &[u8]) {
        self.osc(&[b"52", ty, data]);
    }
    fn unhandled_osc(&mut self, _: &mut vt100::Screen, params: &[&[u8]]) {
        // OSC 7 tells the terminal the working directory (new tabs open there).
        if params.first() == Some(&&b"7"[..]) {
            self.osc(params);
        }
    }
    fn unhandled_csi(
        &mut self,
        screen: &mut vt100::Screen,
        i1: Option<u8>,
        _i2: Option<u8>,
        params: &[&[u16]],
        c: char,
    ) {
        let first = params.first().and_then(|p| p.first()).copied().unwrap_or(0);
        match (i1, c) {
            // Cursor position report.
            (None, 'n') if first == 6 => {
                let (row, col) = screen.cursor_position();
                self.reply.extend_from_slice(format!("\x1b[{};{}R", row + 1, col + 1).as_bytes());
            }
            // Status report: "terminal OK".
            (None, 'n') if first == 5 => self.reply.extend_from_slice(b"\x1b[0n"),
            // Device attributes.
            (None, 'c') if first == 0 => self.reply.extend_from_slice(b"\x1b[?1;2c"),
            (Some(b'>'), 'c') if first == 0 => self.reply.extend_from_slice(b"\x1b[>0;10;1c"),
            // Focus reporting is the real terminal's to deliver.
            (Some(b'?'), 'h' | 'l') if params.iter().any(|p| *p == [1004]) => {
                self.pass.extend_from_slice(format!("\x1b[?1004{c}").as_bytes());
            }
            _ => {}
        }
    }
}

/// Input-related modes the program asked for. The real terminal has to be in
/// the same modes, because keystrokes go to the program untranslated.
#[derive(Clone, Copy, PartialEq, Default)]
struct Modes {
    app_cursor: bool,
    app_keypad: bool,
    paste: bool,
    mouse: vt100::MouseProtocolMode,
    encoding: vt100::MouseProtocolEncoding,
    cursor_style: vt100::CursorStyle,
}

impl Modes {
    /// With `capture`, glowterm takes the mouse for itself (to scroll with
    /// the wheel) whenever the program has not asked for it.
    fn of(s: &vt100::Screen, capture: bool) -> Self {
        use vt100::{MouseProtocolEncoding as E, MouseProtocolMode as M};
        let (mouse, encoding) = match s.mouse_protocol_mode() {
            M::None if capture => (M::PressRelease, E::Sgr),
            mode => (mode, s.mouse_protocol_encoding()),
        };
        Modes {
            app_cursor: s.application_cursor(),
            app_keypad: s.application_keypad(),
            paste: s.bracketed_paste(),
            mouse,
            encoding,
            cursor_style: s.cursor_style(),
        }
    }

    fn apply(&self, old: &Modes, out: &mut Vec<u8>) {
        use vt100::{CursorStyle as C, MouseProtocolEncoding as E, MouseProtocolMode as M};
        if self.app_cursor != old.app_cursor {
            out.extend_from_slice(if self.app_cursor { b"\x1b[?1h" } else { b"\x1b[?1l" });
        }
        if self.app_keypad != old.app_keypad {
            out.extend_from_slice(if self.app_keypad { b"\x1b=" } else { b"\x1b>" });
        }
        if self.paste != old.paste {
            out.extend_from_slice(if self.paste { b"\x1b[?2004h" } else { b"\x1b[?2004l" });
        }
        if self.mouse != old.mouse || self.encoding != old.encoding {
            out.extend_from_slice(b"\x1b[?9l\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1005l\x1b[?1006l");
            out.extend_from_slice(match self.mouse {
                M::None => b"",
                M::Press => b"\x1b[?9h",
                M::PressRelease => b"\x1b[?1000h",
                M::ButtonMotion => b"\x1b[?1002h",
                M::AnyMotion => b"\x1b[?1003h",
            });
            out.extend_from_slice(match self.encoding {
                E::Default => b"",
                E::Utf8 => b"\x1b[?1005h",
                E::Sgr => b"\x1b[?1006h",
            });
        }
        if self.cursor_style != old.cursor_style {
            let n = match self.cursor_style {
                C::Default => 0,
                C::BlinkBlock => 1,
                C::SteadyBlock => 2,
                C::BlinkUnderline => 3,
                C::SteadyUnderline => 4,
                C::BlinkVerticalBar => 5,
                C::SteadyVerticalBar => 6,
            };
            out.extend_from_slice(format!("\x1b[{n} q").as_bytes());
        }
    }
}

// -------------------------------------------------------------------- main

enum Event {
    Output(Vec<u8>),
    Eof,
    Resize,
    Quit,
    /// Move the view this many lines back into history (negative: forward).
    Scroll(i32),
    /// Return the view to the live screen.
    Snap,
    /// Switch to the next animation.
    NextAnimation,
}

/// Ctrl+], the key that switches animation.
const HOTKEY: u8 = 0x1d;

// What the input thread needs to know about the screen, as bit flags.
/// The mouse is ours: its reports are consumed here, not sent to the program.
const CAPTURE: u8 = 1;
const ALT_SCREEN: u8 = 2;
const APP_CURSOR: u8 = 4;
const SCROLLED: u8 = 8;

enum Mouse {
    /// A complete report: its button code and length in bytes.
    Report(u32, usize),
    Incomplete,
    Invalid,
}

/// Parses the rest of an SGR mouse report, after its `ESC [ <` prefix:
/// `button;column;row` followed by `M` (press) or `m` (release).
fn parse_mouse(rest: &[u8]) -> Mouse {
    for (i, &b) in rest.iter().enumerate().take(24) {
        match b {
            b'0'..=b'9' | b';' => {}
            b'M' | b'm' => {
                let button = rest[..i].split(|&c| c == b';').next().unwrap_or(&[]);
                let button = std::str::from_utf8(button).ok().and_then(|v| v.parse().ok());
                return button.map_or(Mouse::Invalid, |b| Mouse::Report(b, i + 1));
            }
            _ => return Mouse::Invalid,
        }
    }
    if rest.len() < 24 { Mouse::Incomplete } else { Mouse::Invalid }
}

/// Forwards keystrokes to the program. While the mouse is captured, mouse
/// reports are taken out of the stream and the wheel is turned into
/// scrolling.
fn pump_input(
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    flags: Arc<AtomicU8>,
    tx: SyncSender<Event>,
    lines: i32,
    hotkey: bool,
) {
    let mut buf = [0u8; 4096];
    let mut held: Vec<u8> = Vec::new();
    let mut forward: Vec<u8> = Vec::new();
    let mut stdin = std::io::stdin().lock();
    loop {
        let n = match stdin.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        let f = flags.load(Ordering::Relaxed);
        held.extend_from_slice(&buf[..n]);
        forward.clear();
        if f & CAPTURE == 0 {
            forward.append(&mut held);
        } else {
            let data = std::mem::take(&mut held);
            let mut i = 0;
            while i < data.len() {
                if data[i..].starts_with(b"\x1b[<") {
                    match parse_mouse(&data[i + 3..]) {
                        Mouse::Report(button, len) => {
                            i += 3 + len;
                            // Wheel: bit 6 set, low bits 0 = up, 1 = down.
                            if button & 64 != 0 && button & 3 < 2 {
                                let up = button & 3 == 0;
                                if f & ALT_SCREEN != 0 {
                                    let key: &[u8] = match (f & APP_CURSOR != 0, up) {
                                        (false, true) => b"\x1b[A",
                                        (false, false) => b"\x1b[B",
                                        (true, true) => b"\x1bOA",
                                        (true, false) => b"\x1bOB",
                                    };
                                    for _ in 0..lines {
                                        forward.extend_from_slice(key);
                                    }
                                } else if tx.send(Event::Scroll(if up { lines } else { -lines })).is_err() {
                                    return;
                                }
                            }
                            continue;
                        }
                        // The report was split across reads; wait for the rest.
                        Mouse::Incomplete => {
                            held.extend_from_slice(&data[i..]);
                            break;
                        }
                        Mouse::Invalid => {}
                    }
                }
                forward.push(data[i]);
                i += 1;
            }
        }
        // Only a keypress on its own counts. The same byte inside a larger
        // read is pasted or program-generated data and is passed through.
        if hotkey && !forward.is_empty() && forward.iter().all(|&b| b == HOTKEY) {
            for _ in 0..forward.len() {
                if tx.send(Event::NextAnimation).is_err() {
                    return;
                }
            }
            forward.clear();
        }
        if forward.is_empty() {
            continue;
        }
        // Typing while looking at history returns to the prompt.
        if f & SCROLLED != 0 && tx.send(Event::Snap).is_err() {
            return;
        }
        let Ok(mut w) = writer.lock() else { break };
        if w.write_all(&forward).and_then(|_| w.flush()).is_err() {
            break;
        }
    }
}

fn main() {
    let code = match run() {
        Ok(code) => code,
        Err(e) => {
            // The failure may have come after the terminal was taken over.
            if ORIGINAL_TERMIOS.get().is_some() {
                restore_terminal();
            }
            eprintln!("glowterm: {e}");
            1
        }
    };
    std::process::exit(code);
}

fn run() -> Result<i32, Error> {
    let Some(opts) = parse_args()? else { return Ok(0) };

    if unsafe { libc::isatty(0) == 0 || libc::isatty(1) == 0 } {
        return Err("needs to run in a terminal".into());
    }
    if std::env::var_os("GLOWTERM").is_some() {
        return Err("already running inside glowterm".into());
    }

    let (mut rows, mut cols) = terminal_size();
    let mut anim = anim::create(&opts.anim, cols as usize, rows as usize, opts.density)
        .ok_or_else(|| format!("no animation named '{}' (try --list)", opts.anim))?;

    let colorterm = std::env::var("COLORTERM").unwrap_or_default();
    let truecolor = !opts.force_256 && (colorterm.contains("truecolor") || colorterm.contains("24bit"));

    // Start the program on its own pseudo-terminal.
    let pair = native_pty_system().openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;
    let mut cmd = if opts.command.is_empty() {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        let mut c = CommandBuilder::new(shell);
        c.arg("-l");
        c
    } else {
        let mut c = CommandBuilder::new(&opts.command[0]);
        c.args(&opts.command[1..]);
        c
    };
    if let Ok(dir) = std::env::current_dir() {
        cmd.cwd(dir);
    }
    cmd.env("GLOWTERM", "1");
    // Describe the terminal the program is really talking to: glowterm's own
    // emulator, which understands the common xterm set and no more.
    cmd.env("TERM", "xterm-256color");
    let mut child = pair.slave.spawn_command(cmd)?;
    drop(pair.slave);
    let master = pair.master;
    let mut reader = master.try_clone_reader()?;
    let writer = Arc::new(Mutex::new(master.take_writer()?));

    enter_raw_mode()?;
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_panic(info);
        // A panic on a helper thread would otherwise leave the main loop
        // drawing over the terminal it has just handed back.
        std::process::exit(101);
    }));

    // A bounded channel, so a program that floods output waits for us
    // rather than queueing without limit.
    let (tx, rx) = sync_channel::<Event>(64);

    let tx_out = tx.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 16 * 1024];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if tx_out.send(Event::Output(buf[..n].to_vec())).is_err() {
                        return;
                    }
                }
            }
        }
        let _ = tx_out.send(Event::Eof);
    });

    // Answers to the program's queries are written by their own thread. If
    // the program is not reading its input, that write can block for good,
    // and the main loop must never be the one stuck in it.
    let (reply_tx, reply_rx) = sync_channel::<Vec<u8>>(256);
    {
        let writer = Arc::clone(&writer);
        std::thread::spawn(move || {
            for bytes in reply_rx {
                let Ok(mut w) = writer.lock() else { break };
                if w.write_all(&bytes).and_then(|_| w.flush()).is_err() {
                    break;
                }
            }
        });
    }

    let flags = Arc::new(AtomicU8::new(0));
    {
        let (writer, flags, tx) = (Arc::clone(&writer), Arc::clone(&flags), tx.clone());
        let (lines, hotkey) = (opts.scroll_lines, opts.hotkey);
        std::thread::spawn(move || pump_input(writer, flags, tx, lines, hotkey));
    }

    use signal_hook::consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM, SIGWINCH};
    let mut signals = signal_hook::iterator::Signals::new([SIGWINCH, SIGTERM, SIGHUP, SIGINT, SIGQUIT])?;
    let tx_sig = tx.clone();
    std::thread::spawn(move || {
        for sig in signals.forever() {
            let ev = if sig == SIGWINCH { Event::Resize } else { Event::Quit };
            if tx_sig.send(ev).is_err() {
                break;
            }
        }
    });
    drop(tx);

    let mut parser = vt100::Parser::new_with_callbacks(nz(rows), nz(cols.max(MIN_EMULATOR_COLS)), opts.scrollback, Hooks::default());
    let mut renderer = Renderer::new(cols as usize, rows as usize, truecolor);
    let mut layer = Layer::default();
    let mut frame = Vec::new();
    let mut out: Vec<u8> = Vec::with_capacity(64 * 1024);
    let look = Look { intensity: opts.intensity, text_dim: opts.text_dim };
    let mut modes = Modes::default();
    let mut shown_cursor: Option<(u16, u16, bool)> = None;

    let frame_time = Duration::from_secs_f32(1.0 / opts.fps);
    let mut next_tick = Instant::now();
    let mut last_step = Instant::now();
    // Set when the shell has new output: the time by which to show it.
    let mut shell_due: Option<Instant> = None;
    let mut layer_stale = true;
    let mut killed = false;
    let mut last_draw = Instant::now();
    let mut current = anim::ALL.iter().position(|(name, _)| *name == opts.anim).unwrap_or(0);
    // A short message in the corner, and when to take it down.
    let mut notice: Option<(String, Instant)> = None;

    'main: loop {
        let deadline = shell_due.map_or(next_tick, |d| d.min(next_tick));
        let mut event = match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(ev) => Some(ev),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        let mut handled = 0;
        while let Some(ev) = event.take() {
            match ev {
                Event::Output(bytes) => {
                    parser.process(&bytes);
                    let hooks = parser.callbacks_mut();
                    if !hooks.reply.is_empty() {
                        // If the queue is full the program is not reading
                        // its answers anyway; drop this one.
                        let _ = reply_tx.try_send(std::mem::take(&mut hooks.reply));
                    }
                    layer_stale = true;
                    // Wait a moment so a burst of output is drawn once.
                    // and never redraw for output more than about 60 times
                    // a second, however fast it arrives.
                    shell_due.get_or_insert_with(|| {
                        (Instant::now() + Duration::from_millis(4)).max(last_draw + Duration::from_millis(16))
                    });
                }
                Event::Resize => {
                    let (r, c) = terminal_size();
                    if (r, c) != (rows, cols) {
                        (rows, cols) = (r, c);
                        let _ = master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
                        parser.set_size(nz(rows), nz(cols.max(MIN_EMULATOR_COLS)));
                        anim.resize(cols as usize, rows as usize);
                        renderer.resize(cols as usize, rows as usize);
                        out.extend_from_slice(b"\x1b[0m\x1b[2J");
                        layer_stale = true;
                        shell_due = Some(Instant::now());
                    }
                }
                Event::Scroll(lines) => {
                    let screen = parser.screen_mut();
                    if !screen.alternate_screen() {
                        let to = (screen.scrollback() as i64 + lines as i64).max(0) as usize;
                        screen.set_scrollback(to);
                        layer_stale = true;
                        shell_due = Some(Instant::now());
                    }
                }
                Event::Snap => {
                    if parser.screen().scrollback() != 0 {
                        parser.screen_mut().set_scrollback(0);
                        layer_stale = true;
                        shell_due = Some(Instant::now());
                    }
                }
                Event::NextAnimation => {
                    current = (current + 1) % anim::ALL.len();
                    let name = anim::ALL[current].0;
                    if let Some(next) = anim::create(name, cols as usize, rows as usize, opts.density) {
                        anim = next;
                        notice = Some((format!(" {name} "), Instant::now() + Duration::from_millis(1500)));
                        layer_stale = true;
                        shell_due = Some(Instant::now());
                    }
                }
                Event::Eof => break 'main,
                Event::Quit => {
                    let _ = child.kill();
                    killed = true;
                    break 'main;
                }
            }
            handled += 1;
            if handled < 64 {
                event = rx.try_recv().ok();
            }
        }

        // Tell the input thread how to treat the mouse and the wheel.
        let screen = parser.screen();
        let capture = opts.mouse && screen.mouse_protocol_mode() == vt100::MouseProtocolMode::None;
        let mut f = 0;
        for (on, bit) in [
            (capture, CAPTURE),
            (screen.alternate_screen(), ALT_SCREEN),
            (screen.application_cursor(), APP_CURSOR),
            (screen.scrollback() != 0, SCROLLED),
        ] {
            if on {
                f |= bit;
            }
        }
        flags.store(f, Ordering::Relaxed);

        let now = Instant::now();
        let tick = now >= next_tick;
        if tick {
            anim.step(now.duration_since(last_step).as_secs_f32().min(0.25));
            last_step = now;
            next_tick = now + frame_time;
            if matches!(child.try_wait(), Ok(Some(_))) {
                break;
            }
        }
        let shell_ready = shell_due.is_some_and(|d| now >= d);
        if !tick && !shell_ready {
            continue;
        }
        shell_due = None;
        last_draw = now;

        // Draw: terminal settings first, then whatever cells changed.
        let (w, h) = (cols as usize, rows as usize);
        out.append(&mut parser.callbacks_mut().pass);
        let screen = parser.screen();
        let new_modes = Modes::of(screen, opts.mouse);
        new_modes.apply(&modes, &mut out);
        modes = new_modes;

        let scrolled = screen.scrollback();
        if notice.as_ref().is_some_and(|(_, until)| now >= *until) {
            notice = None;
            layer_stale = true;
        }
        if layer_stale {
            layer.rebuild(screen, w, h);
            if scrolled > 0 {
                layer.label(&format!(" \u{2191} {scrolled}/{} ", screen.scrollback_len()), w);
            } else if let Some((text, _)) = &notice {
                layer.label(text, w);
            }
            layer_stale = false;
        }
        let (crow, ccol) = screen.cursor_position();
        // While looking at history the cursor's row is not on screen.
        let visible = !screen.hide_cursor() && scrolled == 0;
        let cursor = visible.then_some((crow as usize, ccol as usize));
        compose(&layer, anim.as_ref(), &look, w, h, cursor, &mut frame);

        let before = out.len();
        renderer.draw(&mut frame, &layer.wide_rows, &mut out);
        let drew = out.len() > before;
        if drew || shown_cursor != Some((crow, ccol, visible)) {
            out.extend_from_slice(format!("\x1b[{};{}H", crow + 1, ccol + 1).as_bytes());
            out.extend_from_slice(if visible { b"\x1b[?25h" } else { b"\x1b[?25l" });
            shown_cursor = Some((crow, ccol, visible));
        }
        if !out.is_empty() {
            // Synchronized output: the terminal shows the frame all at once.
            // The cursor is hidden while it is being moved around.
            let mut buf = Vec::with_capacity(out.len() + 32);
            buf.extend_from_slice(b"\x1b[?2026h");
            if drew {
                buf.extend_from_slice(b"\x1b[?25l");
            }
            buf.append(&mut out);
            buf.extend_from_slice(b"\x1b[?2026l");
            write_stdout(&buf);
        }
    }

    restore_terminal();
    if killed {
        return Ok(143);
    }
    Ok(child.wait().map(|s| s.exit_code() as i32).unwrap_or(0))
}
