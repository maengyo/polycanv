"""터미널을 N개 띄웠을 때 무엇을 얼마나 쓰는가.

**비교 대상이 있어야 의미가 있다.** polycanv 혼자 재보면 "40MB" 가 큰지 작은지 알 수 없다.
그래서 같은 조건으로 셋을 잰다:

    bare      PTY 에 셸만 N개 — 바닥값. 셸 자체가 쓰는 것
    tmux      같은 일을 하는 표준 도구 (C 로 쓰였다)
    polycanv  우리 것

재는 것은 **프로세스 트리 전체**다. 자식만 세거나 부모만 세면 둘 다 거짓말이 된다.

    uv run python scripts/dev/bench.py 4
"""

from __future__ import annotations

import fcntl
import os
import pty
import select
import struct
import subprocess
import sys
import termios
import time

ROWS, COLS = 40, 140
SETTLE = 4.0  # 띄우고 잠잠해질 때까지
WINDOW = 6.0  # CPU 를 재는 구간


def tree(pid: int) -> list[int]:
    """이 프로세스와 그 아래 전부."""
    out = subprocess.run(
        ["ps", "-Ao", "pid=,ppid="], capture_output=True, text=True, check=False
    ).stdout
    children: dict[int, list[int]] = {}
    for line in out.splitlines():
        try:
            p, pp = (int(v) for v in line.split())
        except ValueError:
            continue
        children.setdefault(pp, []).append(p)
    found, stack = [], [pid]
    while stack:
        cur = stack.pop()
        found.append(cur)
        stack.extend(children.get(cur, []))
    return found


def usage(pids: list[int]) -> tuple[int, float]:
    """(RSS 합계 KB, CPU 누적 초)."""
    if not pids:
        return 0, 0.0
    # ⚠️ `-p` 를 빼면 **인자가 무시되고 머신 전체**가 나온다(실측: 4개를 물었는데 93개).
    #    합계가 그럴듯한 숫자로 나와서 틀린 줄도 모른다.
    out = subprocess.run(
        ["ps", "-o", "rss=,time=", "-p", ",".join(str(p) for p in pids)],
        capture_output=True,
        text=True,
        check=False,
    ).stdout
    rss, cpu = 0, 0.0
    for line in out.splitlines():
        parts = line.split()
        if len(parts) != 2:
            continue
        rss += int(parts[0])
        clock = parts[1].replace("-", ":").split(":")
        seconds = 0.0
        for piece in clock:
            seconds = seconds * 60 + float(piece)
        cpu += seconds
    return rss, cpu


def spawn(argv: list[str]) -> tuple[int, int]:
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
    os.set_blocking(master, False)
    return pid, master


def drain(fds: list[int]) -> None:
    """PTY 를 비워 준다. 안 읽으면 자식이 버퍼가 차서 멈춘다."""
    for fd in fds:
        try:
            while os.read(fd, 65536):
                pass
        except (BlockingIOError, OSError):
            pass


def measure(label: str, pids: list[int], fds: list[int]) -> None:
    deadline = time.time() + SETTLE
    while time.time() < deadline:
        drain(fds)
        time.sleep(0.05)

    all_pids = [p for pid in pids for p in tree(pid)]
    rss0, cpu0 = usage(all_pids)

    deadline = time.time() + WINDOW
    while time.time() < deadline:
        drain(fds)
        time.sleep(0.05)

    rss1, cpu1 = usage(all_pids)
    busy = (cpu1 - cpu0) / WINDOW * 100
    print(
        f"  {label:22} RSS {rss1 / 1024:7.1f} MB"
        f"   유휴 CPU {busy:5.1f}%   (프로세스 {len(all_pids)}개)"
    )


def under_load(label: str, pids: list[int], fds: list[int], command: str) -> None:
    """쏟아지는 출력을 얼마나 빨리·싸게 삼키는가.

    유휴 상태만 재면 터미널의 진짜 비용을 놓친다. 빌드 로그나 `cat` 한 번이
    화면을 멈추게 하는지가 실제로 쓸 때 체감되는 부분이다.
    """
    deadline = time.time() + SETTLE
    while time.time() < deadline:
        drain(fds)
        time.sleep(0.05)

    all_pids = [p for pid in pids for p in tree(pid)]
    _, cpu0 = usage(all_pids)

    start = time.time()
    os.write(fds[0], command.encode())

    # ⚠️ **여기서 잠들면 안 된다.** 주기적으로 비우면 PTY 버퍼가 차서 자식이 쓰기에서
    #    멈추고, 그러면 재는 것이 상대 프로그램이 아니라 이 루프가 된다
    #    (실측: 셸만 재는데 38초가 나왔다 — 셸이 느린 게 아니라 우리가 안 읽어서였다).
    quiet_since = None
    while time.time() - start < 120:
        ready, _, _ = select.select(fds, [], [], 0.2)
        got = False
        for fd in ready:
            try:
                while os.read(fd, 1 << 20):
                    got = True
            except (BlockingIOError, OSError):
                pass
        now = time.time()
        if got:
            quiet_since = None
        elif quiet_since is None:
            quiet_since = now
        elif now - quiet_since > 1.5:
            break

    elapsed = time.time() - start - 1.5
    _, cpu1 = usage(all_pids)
    print(f"  {label:22} {elapsed:6.2f}초   CPU {cpu1 - cpu0:5.2f}초")


def bare(n: int):
    pairs = [spawn(["/bin/sh"]) for _ in range(n)]
    return [p for p, _ in pairs], [f for _, f in pairs]


def tmux(n: int):
    session = f"bench{os.getpid()}"
    # **전용 소켓을 쓴다.** 그냥 띄우면 사용자가 이미 돌리던 tmux 서버에 붙어서
    # 남의 세션까지 함께 재게 된다(실측: 프로세스 15개, RSS 56MB 로 부풀었다).
    sock = ["tmux", "-L", session]
    subprocess.run(
        [*sock, "new-session", "-d", "-s", session, "-x", str(COLS), "-y", str(ROWS), "/bin/sh"],
        check=True,
    )
    for _ in range(n - 1):
        # 분할이 아니라 창으로 늘린다 — 분할은 자리가 없으면 실패한다.
        # `-t 이름` 은 창 이름으로도 읽힌다. 콜론을 붙여 세션임을 못박는다.
        subprocess.run([*sock, "new-window", "-t", f"{session}:", "/bin/sh"], check=True)
    pid, fd = spawn([*sock, "attach", "-t", session])
    # tmux 는 **서버가 따로 산다.** 클라이언트 트리만 재면 셸들이 통째로 빠져
    # tmux 가 1MB 짜리 프로그램으로 보인다(실측).
    server = subprocess.run(
        [*sock, "display-message", "-p", "-t", session, "#{pid}"],
        capture_output=True,
        text=True,
        check=False,
    ).stdout.strip()
    pids = [pid] + ([int(server)] if server.isdigit() else [])
    return pids, [fd], session


def polycanv(n: int, keys_gap: float = 0.35):
    pid, fd = spawn([os.environ.get("POLYCANV", "polycanv")])
    time.sleep(2.5)
    for _ in range(n - 1):  # 첫 하나는 스스로 뜬다
        for key in ("\x02", "t"):
            os.write(fd, key.encode())
            time.sleep(keys_gap)
        drain([fd])
    return [pid], [fd]


LOAD = "seq 1 200000\r"


def main() -> None:
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 4
    if "--load" in sys.argv:
        print(f"부하: `{LOAD.strip()}` 를 한 터미널에서 실행\n")
        pids, fds = bare(1)
        under_load("bare (셸만 — 바닥값)", pids, fds, LOAD)
        for p in pids:
            os.kill(p, 15)

        pids, fds, session = tmux(1)
        under_load("tmux", pids, fds, LOAD)
        subprocess.run(["tmux", "-L", session, "kill-server"], check=False)

        pids, fds = polycanv(1)
        under_load("polycanv", pids, fds, LOAD)
        for p in pids:
            os.kill(p, 15)
        return
    print(f"터미널 {n}개 · {COLS}x{ROWS} · CPU 는 {WINDOW:.0f}초 유휴 구간 평균\n")

    pids, fds = bare(n)
    measure("bare (셸만)", pids, fds)
    for p in pids:
        os.kill(p, 15)

    pids, fds, session = tmux(n)
    measure("tmux", pids, fds)
    subprocess.run(["tmux", "-L", session, "kill-server"], check=False)

    pids, fds = polycanv(n)
    measure("polycanv", pids, fds)
    for p in pids:
        os.kill(p, 15)


if __name__ == "__main__":
    main()
