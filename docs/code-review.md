# Code review, November 2026

An independent reviewer (a separate AI agent that had not written the code and was given the source, not the author's opinion of it) reviewed glowterm 0.1.0 on 2026-11-16. It read all of `src/`, ran the build, tests and clippy, read the `vt100` and `portable-pty` sources where behaviour depended on them, and wrote throw-away harnesses to reproduce what it suspected. It changed nothing in the project.

It reported 16 findings, none rated high. Nine were fixed straight away and the fixes checked by experiment; seven remain open and are listed with the reason.

## Baseline

| Check | Before the review's fixes | After |
|---|---|---|
| `cargo build --release` | clean | clean |
| `cargo test --release` | 1 of 1 passing | 1 of 1 passing |
| `cargo clippy --release` | 1 error, 1 warning | clean |

## Fixed

| # | Severity | Finding | Fix | How the fix was checked |
|---|---|---|---|---|
| 1 | medium | **Hard hang.** A program in raw mode that sends cursor-position queries without reading its input filled the PTY's input queue; the main thread blocked writing the replies, stopped draining output, and ignored SIGTERM. Only SIGKILL ended it, leaving the terminal unusable. | Replies are written by a dedicated thread through a bounded queue; the main loop never writes to the PTY. Replies are dropped when the queue is full. | The reproducing command (a loop printing 40 queries at a time under `stty raw`) now keeps drawing; SIGTERM exits with 143 and the terminal restored. |
| 2 (part) | medium | **Emulator panic at one column.** The `vt100` library panics on some emoji when the screen is one column wide, ending the session. | The emulator is never made narrower than two columns, whatever the window size. | `echo 'I ❤️ glowterm'` in a 1-column window: no panic, clean exit. |
| 3 | medium | **Shell output redrawn at up to about 250 times a second**, regardless of `--fps` (239 fps measured during `seq 1 2000000`). | Output-triggered redraws are limited to about 60 a second. `--help` now says `--fps` is the animation's frame rate. | A flood of output produced 564 frames in about 8 s (roughly 70 a second, including animation frames). |
| 5 | low | **`kill -INT` or `kill -QUIT` from outside ended glowterm without restoring the terminal.** | Both signals are now handled like SIGTERM. | External SIGINT: exit 143, terminal restored. |
| 6 | low | **An error after taking over the terminal skipped the restore** (one such path existed: failing to install signal handlers). | The error path in `main` restores the terminal if it had been taken over. | By reading; the path cannot easily be triggered. |
| 7 | low | **X10 mouse mode was left on** if the program enabled it (`ESC [ ? 9 h`) and exited. | The restore sequence now turns it off. | Captured output: the last `?9l` follows the last `?9h`. |
| 11 | low | **Ctrl+]'s byte was stripped from pasted data**, and switched the animation. | The hotkey is recognised only as a keypress on its own; the same byte inside a larger read is passed through. | `x`, byte 0x1D, `y` sent together reaches `cat -v` as `x^]y` and the animation does not change; a lone Ctrl+] still switches. |
| 12 | low | **`--fps nan` panicked** after the terminal had been taken over; `--scroll nan` gave zero lines per notch. | Non-finite numbers are rejected when options are parsed. | `glowterm --fps nan` prints an error and exits 1. |
| 14 | low | **A panic on a helper thread** restored the terminal but left the main loop drawing over it. | The panic hook now ends the process after restoring. | By reading; no reachable helper-thread panic was found. |

Also fixed from the same report:

- **`TERM` was inherited unchanged**, so under kitty, ghostty or alacritty a program would assume features glowterm's emulator lacks. The program now gets `TERM=xterm-256color`. Checked: started with `TERM=xterm-kitty`, the shell inside reports `xterm-256color`.
- **Both clippy complaints** (a hand-written 6.28 where `TAU` was meant; a boolean expression that could be simpler).
- **Help text** (finding 16): `--density` now says arcade ignores it; `--text-dim` describes what is actually dimmed; the selection hint no longer assumes iTerm2.

## Open

| # | Severity | Finding | Why it is open |
|---|---|---|---|
| 2 (rest) | medium | The `vt100` library can also panic at normal sizes on unusual sequences. One 24×80 case was reproduced by hand; random byte streams trigger it about once in 30,000. The terminal is restored, but the session is lost. | Containing it means catching the panic and rebuilding the emulator mid-session, which needs care to get right. Ordinary program output has not triggered it. Worth doing before relying on glowterm for long sessions. |
| 4 | medium / low | Emulation gaps in `vt100`, confirmed: the line-drawing character set (`ESC ( 0`) is ignored, so some programs' borders appear as letters such as `lqqk`; back-tab (`CSI Z`) and `CSI s` / `CSI u` cursor save and restore are ignored. From reading the source: insert mode and autowrap-off are also unhandled. Colour and capability queries get no reply, so programs asking wait out a timeout. Likely affected: `dialog`, `mc`, `whiptail`, `emacs -nw`. | These belong to the emulator library. They are now listed in [usage.md](usage.md). |
| 8 | low | After narrowing the window and scrolling back, a wide character in the last column can be drawn where it does not fit and wrap onto the next row. | Narrow case; cosmetic; clears on the next full redraw. |
| 9 | low | If the real terminal and glowterm disagree about a character's width (some emoji, or "ambiguous characters are double-width" enabled), the rest of that row is drawn one column off. | Inherent in trusting a width table; the reviewer's differential test found no error when widths agree. Covered in troubleshooting. |
| 10 | low | A mouse report split across two reads inside its first three bytes reaches the program as stray text. | Needs a read boundary at an exact point; holding back a lone Escape to prevent it would delay the Escape key. |
| 13 | low | For a few milliseconds after a program turns mouse reporting on or off, mouse events can be routed to the wrong place. | Tiny window; no observed effect. |
| 15 | low | A program that closes its terminal but keeps running would leave glowterm waiting for it after restoring the terminal. Not reproduced. A program killed by a signal is reported as exit status 1, not 128 plus the signal number. | Unusual; low impact. |

## Came out clean

- **Animations at extreme sizes.** All eight were run over 19 sizes (1×1, 2×3, 500×1, 1×300, 1000×3 and others) and 8 density values including NaN, infinity and negatives, with resizes, in debug and release builds. No panic.
- **Colour and attribute output.** Traced by hand: correct for 16-colour, 256-colour and truecolor.
- **Idle cost at 200×60, 20 fps.** 2 to 9% of one core for glowterm itself (aurora highest), 6 to 600 KB/s to the terminal.
- **Flooding.** The bounded channel applies back-pressure; Ctrl-C during `yes` took effect in about 30 ms.

## Not covered

The reviewer worked from scripted pseudo-terminals, as the author did. Nobody has yet reviewed glowterm's behaviour by eye across different terminal emulators, on Linux, or with a multiplexer inside it.
