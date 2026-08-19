"""캔버스 위에 놓이는 터미널 하나.

PTY 를 물고, 그 출력을 화면 상태로 유지하고, 키 입력을 되돌려 보낸다.
**자리와 크기를 스스로 안다** — 타일링이 아니라 자유 배치이기 때문이다.
"""

from __future__ import annotations

import contextlib
import fcntl
import os
import pty
import signal
import struct
import termios
from dataclasses import dataclass

import pyte
from rich.text import Text
from textual.events import MouseDown, MouseMove, MouseUp
from textual.widget import Widget

#: 테두리가 먹는 칸 수. 안쪽 크기를 계산할 때 빼야 PTY 가 실제 표시 영역을 안다.
BORDER = 2

#: 이보다 작아지면 안에서 도는 프로그램이 화면을 못 그린다.
MIN_WIDTH = 16
MIN_HEIGHT = 5

#: 모서리에서 이 범위 안을 누르면 이동이 아니라 크기 조절로 본다.
GRIP = 2

#: PTY 가 뱉은 바이트를 이만큼 보관한다.
#:
#: **왜 보관하는가** — `pyte` 의 `resize` 는 줄이 줄어들 때 위쪽을 버린다. 크기를 조금만
#: 줄여도 화면이 통째로 비는데(실측), 크기 조절이 이 도구의 핵심 조작이라 그대로 둘 수 없다.
#: 그래서 원본 바이트를 들고 있다가 **새 크기의 화면에 다시 먹인다.** 같은 스트림을 같은
#: 순서로 재생하므로 결과는 그 크기에서 원래 보였을 화면과 같다.
#:
#: 256KB 면 보통 터미널 수천 줄이다. 무한정 쌓으면 오래 띄워둔 세션이 메모리를 먹는다.
REPLAY_LIMIT = 256 * 1024


@dataclass
class Geometry:
    """캔버스 위의 자리와 크기. **이게 세션의 정체성이다** — 저장하고 복원할 값."""

    x: int
    y: int
    width: int
    height: int

    def inner(self) -> tuple[int, int]:
        """테두리를 뺀 실제 표시 칸 수 (열, 행)."""
        return max(self.width - BORDER, 1), max(self.height - BORDER, 1)


class TerminalPanel(Widget, can_focus=True):
    """PTY 하나를 물고 캔버스 위에 자유롭게 놓이는 패널."""

    DEFAULT_CSS = """
    TerminalPanel {
        position: absolute;
        border: round $primary-darken-2;
        background: $surface;
    }
    TerminalPanel:focus { border: round $accent; }
    """

    def __init__(self, command: list[str], geometry: Geometry, title: str, cwd: str | None = None):
        super().__init__()
        self.command = command
        self.geometry_ = geometry
        self.border_title = title
        self.cwd = cwd
        cols, rows = geometry.inner()
        #: pyte 화면. Textual 위젯의 `.screen` 은 소속 Screen 이라 이름을 겹칠 수 없다.
        self.vt = pyte.Screen(cols, rows)
        self.stream = pyte.ByteStream(self.vt)
        #: 재생용 원본 바이트. 크기가 바뀌면 이걸 새 화면에 다시 먹인다.
        self._replay = bytearray()
        self.fd: int | None = None
        self.pid: int | None = None
        self._drag_origin: tuple[int, int] | None = None
        self._resizing = False

    # ── 수명 ────────────────────────────────────────────────────────────────
    def on_mount(self) -> None:
        self._apply_geometry()
        self.spawn()

    def spawn(self) -> None:
        """PTY 를 열고 명령을 띄운다."""
        pid, fd = pty.fork()
        if pid == 0:
            if self.cwd:
                with contextlib.suppress(OSError):
                    os.chdir(self.cwd)
            os.environ["TERM"] = "xterm-256color"
            os.execvp(self.command[0], self.command)
        self.pid, self.fd = pid, fd
        self._sync_pty_size()
        os.set_blocking(fd, False)
        # 20fps. 더 자주 읽어도 사람 눈에는 같고 CPU 만 먹는다.
        self.set_interval(0.05, self._pump)

    def close(self) -> None:
        """자식 프로세스를 정리한다. 남겨두면 좀비가 쌓인다."""
        if self.pid:
            with contextlib.suppress(ProcessLookupError, ChildProcessError):
                os.kill(self.pid, signal.SIGHUP)
                os.waitpid(self.pid, os.WNOHANG)
        if self.fd is not None:
            with contextlib.suppress(OSError):
                os.close(self.fd)
            self.fd = None

    # ── 입출력 ──────────────────────────────────────────────────────────────
    def _pump(self) -> None:
        if self.fd is None:
            return
        try:
            data = os.read(self.fd, 65536)
        except BlockingIOError:
            return
        except OSError:
            # 자식이 죽었다. 화면은 마지막 상태로 남겨둔다 —
            # 갑자기 비면 사용자는 무슨 일이 있었는지 알 수 없다.
            self.fd = None
            return
        if data:
            self.stream.feed(data)
            self._remember(data)
            self.refresh()

    def _remember(self, data: bytes) -> None:
        """재생용으로 보관한다. 한도를 넘으면 앞에서 버린다 — 최근 것이 더 쓸모 있다."""
        self._replay += data
        if len(self._replay) > REPLAY_LIMIT:
            del self._replay[: len(self._replay) - REPLAY_LIMIT]

    def send(self, data: str) -> None:
        if self.fd is not None:
            try:
                os.write(self.fd, data.encode())
            except OSError:
                self.fd = None

    def on_key(self, event) -> None:
        if self.fd is None:
            return
        text = event.character
        if text is not None:
            self.send(text)
            event.stop()

    # ── 자리와 크기 ─────────────────────────────────────────────────────────
    def _apply_geometry(self) -> None:
        g = self.geometry_
        self.styles.offset = (g.x, g.y)
        self.styles.width = g.width
        self.styles.height = g.height

    def _sync_pty_size(self) -> None:
        """PTY 에 새 크기를 알린다. 이걸 빼먹으면 안에서 도는 vim 같은 게 깨진다."""
        if self.fd is None:
            return
        cols, rows = self.geometry_.inner()
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))

    def move_to(self, x: int, y: int) -> None:
        self.geometry_.x = max(0, x)
        self.geometry_.y = max(0, y)
        self._apply_geometry()

    def resize_to(self, width: int, height: int) -> None:
        self.geometry_.width = max(MIN_WIDTH, width)
        self.geometry_.height = max(MIN_HEIGHT, height)
        cols, rows = self.geometry_.inner()

        # ★ `vt.resize()` 를 쓰지 않는다. 줄이 줄어들면 위쪽을 버려서 화면이 비어버린다(실측).
        #   새 화면을 만들고 보관해둔 바이트를 다시 먹여 그 크기에서의 화면을 얻는다.
        self.vt = pyte.Screen(cols, rows)
        self.stream = pyte.ByteStream(self.vt)
        if self._replay:
            self.stream.feed(bytes(self._replay))

        self._apply_geometry()
        self._sync_pty_size()

    # ── 마우스: 본체를 끌면 이동, 오른쪽 아래 모서리를 끌면 크기 조절 ────────
    def on_mouse_down(self, event: MouseDown) -> None:
        self.focus()
        g = self.geometry_
        self._resizing = event.x >= g.width - GRIP and event.y >= g.height - GRIP
        self._drag_origin = (event.screen_x, event.screen_y)
        self.capture_mouse()
        event.stop()

    def on_mouse_move(self, event: MouseMove) -> None:
        if self._drag_origin is None:
            return
        ox, oy = self._drag_origin
        dx, dy = event.screen_x - ox, event.screen_y - oy
        g = self.geometry_
        if self._resizing:
            self.resize_to(g.width + dx, g.height + dy)
        else:
            self.move_to(g.x + dx, g.y + dy)
        self._drag_origin = (event.screen_x, event.screen_y)
        event.stop()

    def on_mouse_up(self, event: MouseUp) -> None:
        self._drag_origin = None
        self._resizing = False
        self.release_mouse()
        event.stop()

    # ── 렌더 ────────────────────────────────────────────────────────────────
    def render(self) -> Text:
        return Text("\n".join(line.rstrip() for line in self.vt.display))
