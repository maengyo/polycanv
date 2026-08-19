"""zellij 화면 전체를 캡처한다.

플러그인 패인은 `dump-screen` 이 빈 출력이라 개별로는 못 본다. 하지만 **PTY 마스터가 받는
바이트가 곧 사용자가 보는 화면 전체**다 — 사이드바 포함. 그래서 PTY 를 직접 붙이고
마지막 화면 상태를 그대로 받아 적는다.
"""
import os, pty, sys, time, fcntl, termios, struct, subprocess, re

ROWS, COLS = int(os.environ.get("ROWS", "34")), int(os.environ.get("COLS", "150"))
SESSION = os.environ["SESSION"]
OUT = os.environ["OUT"]

master, slave = pty.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))

pid = os.fork()
if pid == 0:
    os.setsid(); fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
    for t in (0, 1, 2):
        os.dup2(slave, t)
    os.execvp(sys.argv[1], sys.argv[1:])
os.close(slave)

buf = bytearray()
deadline = time.time() + float(os.environ.get("SECONDS", "60"))
os.set_blocking(master, False)
while time.time() < deadline:
    try:
        chunk = os.read(master, 65536)
        if not chunk:
            break
        buf += chunk
    except BlockingIOError:
        time.sleep(0.05)
    except OSError:
        break

open(OUT, "wb").write(bytes(buf))
print(f"captured {len(buf)} bytes -> {OUT}")
