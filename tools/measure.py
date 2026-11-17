#!/usr/bin/env python3
"""Run glowterm in a scripted pseudo-terminal and report what it costs.

    tools/measure.py ANIMATION [ROWS COLS [SECONDS]]

Prints the bytes per second glowterm sent to the terminal and the CPU time it
used. Saves the captured output to /tmp/glowterm-ANIMATION.bin, which
`cargo run --release --example snap ROWS COLS < FILE` turns into a text
picture of the final screen. Build first with `cargo build --release`.
"""
import fcntl, os, pty, select, signal, struct, sys, termios, time

anim = sys.argv[1] if len(sys.argv) > 1 else "matrix"
rows = int(sys.argv[2]) if len(sys.argv) > 3 else 60
cols = int(sys.argv[3]) if len(sys.argv) > 3 else 200
seconds = float(sys.argv[4]) if len(sys.argv) > 4 else 8.0
binary = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "target", "release", "glowterm")

pid, fd = pty.fork()
if pid == 0:
    os.environ.pop("GLOWTERM", None)
    os.environ["COLORTERM"] = "truecolor"
    os.execv(binary, ["glowterm", "--anim", anim, "--", "/bin/sh"])

fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
os.kill(pid, signal.SIGWINCH)
data = b""


def pump(duration):
    global data
    end = time.time() + duration
    while time.time() < end:
        if select.select([fd], [], [], 0.05)[0]:
            try:
                data += os.read(fd, 65536)
            except OSError:
                return


pump(1.0)
os.write(fd, b"echo hello from the shell\r")
pump(1.0)
start = len(data)
pump(seconds)
sent = len(data) - start
path = "/tmp/glowterm-%s.bin" % anim
with open(path, "wb") as f:
    f.write(data)
os.write(fd, b"exit\r")
pump(1.5)
_, status, usage = os.wait4(pid, 0)
print("%s at %dx%d: %.0f KB/s, %.2f s CPU over %.0f s, exit status %s" % (
    anim, cols, rows, sent / seconds / 1024, usage.ru_utime + usage.ru_stime, seconds + 3.5,
    os.WEXITSTATUS(status) if os.WIFEXITED(status) else "signal"))
print("capture saved to", path)
