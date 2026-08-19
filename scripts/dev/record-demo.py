"""README 에 넣을 구동 GIF 를 만든다.

polycanv 를 PTY 에 붙여 띄우고, 정해진 키를 사람처럼 쳐 넣고, 그동안 나온 바이트를
**시간과 함께** 적는다. 그다음 pyte 로 재생하며 일정 간격의 화면을 그려 ffmpeg 로 잇는다.

    uv run --with pillow python scripts/dev/record-demo.py docs/demo/launcher.gif

**폰트를 둘 섞는다.** 박스 문자는 Menlo 가, 한글은 Apple SD Gothic Neo 가 갖고 있고
둘 다 가진 폰트가 이 기계에 없다. 한 벌로 그리면 어느 쪽이든 두부가 된다.
"""

from __future__ import annotations

import contextlib
import fcntl
import os
import pathlib
import pty
import struct
import subprocess
import sys
import tempfile
import termios
import time

import pyte
from PIL import Image, ImageDraw, ImageFont

ROWS, COLS = int(os.environ.get("ROWS", "28")), int(os.environ.get("COLS", "100"))
FPS = int(os.environ.get("FPS", "10"))
CELL_W, CELL_H = 9, 19
FONT_SIZE = 15
#: 한글은 두 칸을 쓴다. 한 칸 폭으로 그리면 겹쳐서 뭉갠다.
HANGUL_SIZE = 15

#: 아래 여백. 마지막 줄의 한글이 셀 높이를 넘어 내려가 잘린다.
PAD = 5

MONO = "/System/Library/Fonts/Menlo.ttc"
HANGUL = "/System/Library/Fonts/AppleSDGothicNeo.ttc"

#: 화면 색. 터미널 기본값이 무엇인지 모르므로 우리가 정한다.
BG = "#11131a"
FG = "#d8dee9"

NAMED = {
    "black": "#2e3440",
    "red": "#bf616a",
    "green": "#a3be8c",
    "brown": "#ebcb8b",
    "yellow": "#ebcb8b",
    "blue": "#5e81ac",
    "magenta": "#b48ead",
    "cyan": "#88c0d0",
    "white": "#e5e9f0",
}


def color(value: str, fallback: str) -> str:
    if value == "default":
        return fallback
    if value in NAMED:
        return NAMED[value]
    if len(value) == 6:
        with contextlib.suppress(ValueError):
            int(value, 16)
            return f"#{value}"
    return fallback


def is_hangul(ch: str) -> bool:
    code = ord(ch)
    return 0xAC00 <= code <= 0xD7A3 or 0x1100 <= code <= 0x11FF or 0x3130 <= code <= 0x318F


class Tolerant(pyte.Screen):
    """질의 시퀀스(DSR 등)에 답하지 않는다 — 화면만 필요하다."""

    def write_process_input(self, data: str) -> None:
        pass


SCRIPT_HELP = """스크립트 문법 (한 줄에 하나):

    wait 2.5              그냥 기다린다
    key ctrl+b            키 하나. 이름은 polycanv 가 쓰는 것과 같다
    type echo hello       글자를 하나씩 친다
    drag 40,8 60,14       누른 채 끌어다 놓는다 (칸 좌표, 0 부터)
"""


def mouse(button: int, col: int, row: int, release: bool = False) -> bytes:
    """SGR 마우스 보고. 터미널이 실제로 보내는 것과 같은 형식이다."""
    end = "m" if release else "M"
    return f"\x1b[<{button};{col + 1};{row + 1}{end}".encode()


def actions(script: str, gap: float):
    """스크립트를 (지연, 바이트) 목록으로 편다."""
    from polycanv.keymap import sequence

    out: list[tuple[float, bytes]] = []
    for raw in script.splitlines():
        line = raw.split("#")[0].strip()
        if not line:
            continue
        verb, _, rest = line.partition(" ")
        if verb == "wait":
            out.append((float(rest), b""))
        elif verb == "key":
            name = rest.strip()
            # 글자 하나짜리 키는 그 글자가 곧 입력이다 — 앱에서는 Textual 이 채워 준다.
            text = sequence(name, name if len(name) == 1 else None)
            if text is None:
                raise SystemExit(f"모르는 키: {rest}")
            out.append((gap, text.encode()))
        elif verb == "type":
            for ch in rest:
                out.append((gap / 4, ch.encode()))
        elif verb == "drag":
            a, b = rest.split()
            x1, y1 = (int(v) for v in a.split(","))
            x2, y2 = (int(v) for v in b.split(","))
            out.append((gap, mouse(0, x1, y1)))
            steps = 8
            for i in range(1, steps + 1):
                # 한 번에 옮기지 않는다. 끌리는 것이 보여야 자유 배치라는 게 전해진다.
                out.append(
                    (
                        0.06,
                        mouse(32, x1 + (x2 - x1) * i // steps, y1 + (y2 - y1) * i // steps),
                    )
                )
            out.append((gap / 2, mouse(0, x2, y2, release=True)))
        else:
            raise SystemExit(f"모르는 명령: {verb}\n\n{SCRIPT_HELP}")
    return out


def clean_home(root: str) -> tuple[dict[str, str], str]:
    """녹화용으로 씻은 환경과 시작 디렉터리.

    **이걸 호출하는 사람의 기억에 맡기지 않는다.** 그냥 찍으면 프롬프트에 사용자명·
    호스트명·실제 경로가 그대로 들어가고, 그게 공개 저장소의 이미지로 남는다.
    """
    home = os.path.join(root, "home")
    work = os.path.join(root, "work", "api")
    os.makedirs(home, exist_ok=True)
    os.makedirs(work, exist_ok=True)
    for name in ("README.md", "main.py", "routes.py", "models.py"):
        pathlib.Path(work, name).touch()

    shell = os.path.join(home, "demo-shell")
    pathlib.Path(shell).write_text(
        "#!/bin/sh\nBASH_SILENCE_DEPRECATION_WARNING=1 PS1='api $ ' "
        "exec /bin/bash --norc --noprofile -i\n",
        encoding="utf-8",
    )
    os.chmod(shell, 0o755)

    env = {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "TERM": "xterm-256color",
        "LANG": os.environ.get("LANG", "en_US.UTF-8"),
        "HOME": home,
        "SHELL": shell,
        "XDG_CONFIG_HOME": os.path.join(home, ".config"),
        "PS1": "api $ ",
        "BASH_SILENCE_DEPRECATION_WARNING": "1",
    }
    return env, work


def record(argv: list[str], steps, seconds: float, warmup: float, env: dict[str, str], cwd: str):
    """자식을 띄우고 (시각, 바이트) 를 모은다."""
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))

    pid = os.fork()
    if pid == 0:
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
        for t in (0, 1, 2):
            os.dup2(slave, t)
        os.chdir(cwd)
        os.execvpe(argv[0], argv, env)
    os.close(slave)

    chunks: list[tuple[float, bytes]] = []
    start = time.time()
    pending = list(steps)
    due = warmup
    os.set_blocking(master, False)
    while time.time() - start < seconds:
        now = time.time() - start
        # 한 번에 몰아 쓰지 않는다. 붙여 보내면 앱이 한 덩어리로 읽어 순서가 달라진다(실측).
        while pending and now >= due:
            delay, data = pending.pop(0)
            if data:
                os.write(master, data)
            due += delay
        try:
            data = os.read(master, 65536)
            if not data:
                break
            chunks.append((now, data))
        except BlockingIOError:
            time.sleep(0.02)
        except OSError:
            break

    with contextlib.suppress(ProcessLookupError):
        os.kill(pid, 15)
    with contextlib.suppress(ChildProcessError):
        os.waitpid(pid, 0)
    return chunks


def snapshot(screen) -> list[list[tuple[str, str, str]]]:
    """지금 화면을 평범한 값으로 떠낸다.

    pyte 의 행은 기본값을 아는 dict 라 그대로 복사하면 빈 칸을 물었을 때 터진다.
    """
    rows = []
    for y in range(ROWS):
        row = screen.buffer[y]
        rows.append([(c.data, c.fg, c.bg) for c in (row[x] for x in range(COLS))])
    return rows


def frames(chunks, seconds: float):
    """일정 간격으로 화면을 떠낸다."""
    screen = Tolerant(COLS, ROWS)
    stream = pyte.ByteStream(screen)
    step = 1.0 / FPS
    at = 0.0
    index = 0
    out = []
    while at < seconds:
        while index < len(chunks) and chunks[index][0] <= at:
            with contextlib.suppress(Exception):
                stream.feed(chunks[index][1])
            index += 1
        out.append(snapshot(screen))
        at += step
    return out


def draw(buffer, mono, hangul) -> Image.Image:
    img = Image.new("RGB", (COLS * CELL_W, ROWS * CELL_H + PAD), BG)
    pen = ImageDraw.Draw(img)
    for y, row in enumerate(buffer):
        for x, (ch, fg, bg_name) in enumerate(row):
            bg = color(bg_name, BG)
            if bg != BG:
                pen.rectangle([x * CELL_W, y * CELL_H, (x + 1) * CELL_W, (y + 1) * CELL_H], fill=bg)
            if not ch or ch == " ":
                continue
            font = hangul if is_hangul(ch) else mono
            pen.text((x * CELL_W, y * CELL_H), ch, font=font, fill=color(fg, FG))
    return img


def main() -> None:
    target = sys.argv[1] if len(sys.argv) > 1 else "docs/demo/polycanv.gif"
    argv = os.environ.get("CMD", "polycanv").split()
    script = pathlib.Path(os.environ["SCRIPT"]).read_text(encoding="utf-8")
    steps = actions(script, float(os.environ.get("GAP", "0.5")))
    seconds = float(os.environ.get("SECONDS", "16"))

    # 죽는 셸이 마지막에 히스토리를 쓰면서 지우기와 겹친다. 임시 디렉터리다 — 남아도 된다.
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as root:
        env, cwd = clean_home(root)
        chunks = record(argv, steps, seconds, float(os.environ.get("WARMUP", "3")), env, cwd)
    print(f"recorded {sum(len(c) for _, c in chunks)} bytes in {len(chunks)} chunks")

    mono = ImageFont.truetype(MONO, FONT_SIZE)
    hangul = ImageFont.truetype(HANGUL, HANGUL_SIZE)

    os.makedirs(os.path.dirname(target) or ".", exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        shots = frames(chunks, seconds)
        for i, buffer in enumerate(shots):
            draw(buffer, mono, hangul).save(os.path.join(tmp, f"{i:05d}.png"))
        print(f"rendered {len(shots)} frames")

        palette = os.path.join(tmp, "palette.png")
        run = subprocess.run  # noqa: S603
        run(
            ["ffmpeg", "-y", "-i", os.path.join(tmp, "%05d.png"), "-vf", "palettegen", palette],
            check=True,
            capture_output=True,
        )
        run(
            [
                "ffmpeg",
                "-y",
                "-framerate",
                str(FPS),
                "-i",
                os.path.join(tmp, "%05d.png"),
                "-i",
                palette,
                "-lavfi",
                "paletteuse",
                "-loop",
                "0",
                target,
            ],
            check=True,
            capture_output=True,
        )
    print(f"-> {target} ({os.path.getsize(target) // 1024} KB)")


if __name__ == "__main__":
    main()
