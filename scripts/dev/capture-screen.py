"""띄운 화면을 그대로 받아 적는다.

**PTY 마스터가 받는 바이트가 곧 사용자가 보는 화면 전체**다. 그래서 PTY 를 직접 붙이고
자식이 뱉는 것을 모두 모은다. `replay-screen.py` 로 재생하면 눈으로 볼 수 있는 화면이 된다.

    OUT=/tmp/shot.raw ROWS=32 COLS=110 SECONDS=6 \\
      python3 scripts/dev/capture-screen.py polycanv

키를 넣어 특정 화면까지 몰고 갈 수 있다. `KEYS` 는 `WARMUP` 초 뒤에 그대로 써 넣는다.

    KEYS=$'\\x0e' ...   # ctrl+n — 도구 목록을 띄운 상태를 찍는다

**주의**: 창 크기는 자식을 실행하기 **전에** 잡아야 한다. 나중에 바꾸면 자식이 이미
읽은 뒤라 반영되지 않는다(실측).
"""

import fcntl
import os
import pty
import struct
import sys
import termios
import time

ROWS, COLS = int(os.environ.get("ROWS", "34")), int(os.environ.get("COLS", "150"))
OUT = os.environ["OUT"]
KEYS = os.environ.get("KEYS", "")
WARMUP = float(os.environ.get("WARMUP", "2.5"))
SECONDS = float(os.environ.get("SECONDS", "8"))
GAP = float(os.environ.get("GAP", "0.6"))

argv = sys.argv[1:]
if argv and argv[0] == "--":  # `-- cmd ...` 도 받아 준다
    argv = argv[1:]
if not argv:
    sys.exit("실행할 명령이 없습니다: capture-screen.py <cmd> [args...]")

master, slave = pty.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))

pid = os.fork()
if pid == 0:
    os.setsid()
    fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
    for t in (0, 1, 2):
        os.dup2(slave, t)
    os.execvp(argv[0], argv)
os.close(slave)

buf = bytearray()
start = time.time()
pending = list(KEYS)
next_key = WARMUP
os.set_blocking(master, False)
while time.time() - start < SECONDS:
    # 한 번에 몰아 쓰지 않고 한 글자씩 띄워 보낸다. 사람이 치는 것과 같아야
    # 앱이 키를 하나씩 처리한다 — 붙여 보내면 처리 순서가 달라진다.
    if pending and time.time() - start >= next_key:
        os.write(master, pending.pop(0).encode())
        next_key += GAP
    try:
        chunk = os.read(master, 65536)
        if not chunk:
            break
        buf += chunk
    except BlockingIOError:
        time.sleep(0.05)
    except OSError:
        break

os.kill(pid, 15)
os.waitpid(pid, 0)

with open(OUT, "wb") as out:
    out.write(bytes(buf))
print(f"captured {len(buf)} bytes -> {OUT}")
