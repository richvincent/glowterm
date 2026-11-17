# How glowterm works

glowterm sits between your terminal and your shell. The shell believes it is talking to a terminal; in fact it is talking to glowterm, which keeps a model of the screen in memory, mixes an animation into the blank parts, and draws the result on the real terminal.

```
 keyboard, mouse ──► input thread ──────────────► PTY ──► shell / program
                        │ (wheel, Ctrl+])                    │
                        ▼                                    ▼ output bytes
 real terminal ◄── renderer ◄── compositor ◄── emulator (vt100 crate)
                                    ▲
                                animation
```

## Pieces

| File | Responsibility |
|---|---|
| `src/main.rs` | Options, the pseudo-terminal, threads, the event loop, input handling, keeping the real terminal's modes in step |
| `src/render.rs` | Flattening the emulator's screen, compositing it over the animation, and drawing only what changed |
| `src/anim/mod.rs` | The `Animation` trait, the list of animations, shared helpers |
| `src/anim/*.rs` | One file per animation |
| `examples/snap.rs` | Development aid: replays captured output and prints the resulting screen as text |
| `tools/measure.py` | Development aid: runs glowterm in a scripted terminal and reports output rate and CPU time |

## The life of a frame

1. **Output arrives.** A reader thread passes bytes from the pseudo-terminal to the main thread over a bounded channel. The main thread feeds them to the emulator, which updates its grid of cells.
2. **Queries are answered.** Programs sometimes ask the terminal a question (where is the cursor, what kind of terminal are you). The emulator does not answer these itself, so glowterm does, writing the reply back to the program.
3. **The shell layer is rebuilt** if the emulator's screen changed. Each cell is copied into a flat grid and marked either as content or as transparent. Each row also records the span of columns its text occupies.
4. **The animation advances** by the time since the last frame.
5. **The frame is composed.** Content cells are copied through. For each transparent cell the animation is asked what to draw; the result is dimmed if the cell lies in a text span.
6. **The frame is drawn.** It is compared cell by cell with the previous frame, and escape sequences are emitted only for cells that differ. The whole update is wrapped in a synchronized-output block so the terminal shows it in one go.

Shell output is drawn within a few milliseconds of arriving; the animation ticks at `--fps`.

## Input

Keystrokes are written straight to the pseudo-terminal by the input thread, so typing never waits on drawing. The thread removes two things from the stream:

- **Mouse reports**, when glowterm has captured the mouse. Wheel events become scroll requests to the main thread (or arrow keys, in a full-screen program); other mouse events are dropped.
- **Ctrl+]** (byte `0x1D`), which becomes a "next animation" request.

The main thread tells the input thread what state the screen is in (mouse captured or not, full-screen program or not, and so on) through a shared set of flags.

## Keeping the real terminal in step

Because keystrokes go to the program untranslated, the real terminal has to be in whatever input modes the program asked for: application cursor keys, bracketed paste, mouse reporting, cursor shape. After each batch of output glowterm compares the modes the emulator has recorded with the ones it last sent to the real terminal, and sends the difference. Window titles, the bell, clipboard writes (OSC 52) and working-directory reports (OSC 7) are passed through as they are.

When the program has not asked for the mouse, glowterm asks for it itself, so that it can see the wheel.

## Leaving cleanly

On every exit path (the program ending, a termination signal, a panic) glowterm sends one fixed sequence that turns off everything it or the program may have turned on, leaves the alternate screen, and restores the saved terminal settings.

## Design choices worth knowing

- **Animations are character-based.** Everything is drawn with ordinary text cells and colours, so glowterm works in any terminal with colour support and needs no GPU. The cost is resolution: a "pixel" is a character cell, or half of one using half-block characters.
- **Output volume is the budget.** The terminal has to parse and draw everything glowterm sends. The renderer sends only changed cells, and animations are written to change as few cells per frame as they can (coarse colour steps, slow motion, fixed internal update rates). See [creating-animations.md](creating-animations.md).
- **The emulator is a library.** Screen state comes from the `vt100` crate. It covers what everyday programs use; what it does not support is listed in [usage.md](usage.md).
