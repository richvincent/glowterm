# Using glowterm

## Starting it

```
glowterm [OPTIONS] [-- COMMAND [ARGS...]]
```

With no command, glowterm starts your login shell (`$SHELL -l`, or `/bin/zsh -l` if `SHELL` is not set) in the current directory. Anything after `--` is run instead:

```
glowterm -- htop
glowterm -a aurora -- ssh myserver
```

glowterm ends when that program ends, and exits with the same status. It must be run in a terminal; it refuses to start with its input or output redirected.

## Options

| Option | Default | Meaning |
|---|---|---|
| `-a`, `--anim NAME` | `matrix` | Animation to start with. See `--list`. |
| `-l`, `--list` | | Print the animations and exit. |
| `-i`, `--intensity N` | `1.0` | Overall animation brightness, 0.0 to 1.0. |
| `-t`, `--text-dim N` | `0.3` | Share of that brightness kept behind lines of text, 0.0 to 1.0. `0` removes the animation from text lines altogether. |
| `-d`, `--density N` | `1.0` | How busy the animation is. Each animation interprets it in its own way (see below). |
| `-f`, `--fps N` | `20` | Animation frames drawn per second, 1 to 60. Some animations update their scene more slowly than this by design. Shell output is drawn as it arrives, up to about 60 times a second, whatever this is set to. |
| `-s`, `--scroll N` | `3` | Lines scrolled per wheel notch, 1 to 100. |
| `--scrollback N` | `10000` | Lines of history kept. |
| `--no-mouse` | | Do not capture the mouse. Click-drag selection works normally; the wheel cannot scroll glowterm's history. |
| `--no-hotkey` | | Send Ctrl+] to the program instead of using it to switch animation. |
| `--256` | | Use the 256-colour palette even when the terminal supports truecolor. |
| `-h`, `--help` | | Print help and exit. |

### What `--density` does to each animation

| Animation | Effect of a higher density |
|---|---|
| `matrix` | Shorter gaps between drops in each column |
| `aurora` | More curtains (three at 1.0, up to six) |
| `fire` | Taller flames (45% of the screen at 1.0) |
| `starfield` | More stars |
| `plasma` | Less dark space between the bands |
| `cyberspace` | More characters on the tower faces |
| `nightride` | More lit windows and more riders |
| `arcade` | No effect |

## Keys and mouse

| Input | Effect |
|---|---|
| Ctrl+] | Switch to the next animation. Its name appears in the top-right corner for a moment. Only a keypress counts; the same byte inside pasted text is passed through. |
| Wheel up / down | Scroll back and forward through history. |
| Typing | Jump back to the live screen. |
| Option + drag (iTerm2) | Select text. In many other terminals the override key is Shift. |

Everything else you type goes to the program unchanged.

### Scrolling

While you are looking at history, a marker in the top-right corner shows how far back you are and how much history there is (for example `↑ 12/340`), and the cursor is hidden. New output does not move your view. Scroll back down, or type anything, to return.

In a full-screen program that has not asked for the mouse itself (`less`, `man`), the wheel sends arrow keys, so it scrolls that program. Programs that do use the mouse (vim with `mouse=a`, htop, tmux with mouse on) receive it untouched.

## How the animation stays out of your way

- It is drawn only in cells the program has left empty. Text, and any cell with a background colour, underline or reverse video, covers it.
- Behind a line of text, from two columns before its first character to two after its last, it is dimmed to `--text-dim`. The same applies around the cursor.
- Gaps of up to three cells between words are kept completely clear, so an animation glyph is never mistaken for part of a word.

## Colour

glowterm uses 24-bit colour when the `COLORTERM` environment variable contains `truecolor` or `24bit`. Otherwise, or with `--256`, it maps colours to the 256-colour palette, which looks coarser.

## Running with tmux or zellij

**glowterm in a pane** behaves like any full-screen program. Wheel scrolling needs the multiplexer to forward the mouse (`set -g mouse on` in tmux). If the multiplexer is not set up for truecolor, colours will look banded. The multiplexer redraws everything glowterm sends, so heavy animations cost more CPU than in a bare terminal.

**A multiplexer inside glowterm** is less certain, because glowterm's emulation is simpler than a real terminal's. This combination has not been tested.

**Nesting guard.** glowterm sets `GLOWTERM=1` for the program it runs and refuses to start if that variable is already set. A tmux server started from inside glowterm inherits the variable, and every pane it opens afterwards will refuse to run glowterm, even once the original has exited. Start tmux first and glowterm inside a pane, or `unset GLOWTERM` in the affected pane.

## Troubleshooting

| Symptom | Cause and fix |
|---|---|
| `glowterm: already running inside glowterm` | `GLOWTERM` is set in this shell. If you are not actually inside glowterm, run `unset GLOWTERM`. |
| `glowterm: needs to run in a terminal` | Input or output is redirected. Run it directly in a terminal window. |
| Colours look blocky or banded | The terminal, or a multiplexer in between, is not advertising truecolor. Check `echo $COLORTERM`. |
| Click-drag does not select text | glowterm has the mouse. Hold your terminal's override key while dragging (Option in iTerm2, Shift in many others), or start with `--no-mouse`. |
| A program's borders appear as letters such as `lqqk` | It uses the old line-drawing character set, which glowterm's emulator does not support. Run that program outside glowterm. |
| Ctrl+] does nothing in vim or telnet | glowterm uses it. Start with `--no-hotkey`. |
| Box or block characters look double-width and misaligned | Your terminal treats "ambiguous-width" characters as wide. Turn that setting off (in iTerm2: Profiles, Text, "Ambiguous characters are double-width"). |
| The terminal feels sluggish or its CPU use is high | Lower the frame rate (`-f 10`), lower `-d`, or pick a lighter animation (`starfield`, `nightride`, `arcade`). |
| Terminal left in a strange state after a crash | Run `reset`. |

## What is not supported

glowterm tells programs it is an `xterm-256color` terminal (it sets `TERM` for them), but its emulator covers less than a real xterm:

- The line-drawing character set (`ESC ( 0`), used for borders by some older full-screen programs such as `dialog`, `mc` and `whiptail`
- Insert mode and a few cursor commands (back-tab, `CSI s` / `CSI u` save and restore), which may misplace text in `emacs -nw` and some scripts
- Inline images (sixel, iTerm2 and kitty image protocols)
- Hyperlinks (OSC 8)
- The kitty keyboard protocol and other extended key reporting
- Colour queries (programs asking the terminal for its background colour get no answer)
- Windows
