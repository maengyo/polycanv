"""캔버스 위에 놓이는 터미널 하나.

PTY 를 물고, 그 출력을 화면 상태로 유지하고, 키 입력을 되돌려 보낸다.
**자리와 크기를 스스로 안다** — 타일링이 아니라 자유 배치이기 때문이다.
"""

from __future__ import annotations

import asyncio
import contextlib
import fcntl
import os
import pty
import signal
import struct
import termios
from collections import deque
from dataclasses import dataclass

import pyte
from rich.text import Text
from textual.events import MouseDown, MouseMove, MouseScrollDown, MouseScrollUp, MouseUp
from textual.widget import Widget

from .keymap import sequence
from .keys import PREFIX

#: 테두리가 먹는 칸 수. 안쪽 크기를 계산할 때 빼야 PTY 가 실제 표시 영역을 안다.
BORDER = 2

#: 이보다 작아지면 안에서 도는 프로그램이 화면을 못 그린다.
MIN_WIDTH = 16
MIN_HEIGHT = 5

#: 모서리에서 이 범위 안을 누르면 이동이 아니라 크기 조절로 본다.
GRIP = 2

#: 제목 표시줄이 먹는 줄 수. **테두리가 아니라 내용의 일부다** — 그래야 어디를 눌렀는지
#: 정확히 알 수 있고, 창처럼 버튼을 놓을 자리가 생긴다.
TITLE_BAR = 1

#: 제목 줄 오른쪽 끝의 버튼들. 오른쪽부터 차례로 놓인다.
#: 각 항목은 (글자, 동작, 폭). 폭은 글자 폭이 아니라 **누를 수 있는 칸 수**다.
BUTTONS = (("✕", "close", 3), ("─", "minimize", 3))

#: 접었을 때의 높이 — 테두리 둘 + 제목 줄 하나.
FOLDED_HEIGHT = BORDER + TITLE_BAR

#: PTY 가 뱉은 바이트를 이만큼 보관한다.
#:
#: **왜 보관하는가** — `pyte` 의 `resize` 는 줄이 줄어들 때 위쪽을 버린다. 크기를 조금만
#: 줄여도 화면이 통째로 비는데(실측), 크기 조절이 이 도구의 핵심 조작이라 그대로 둘 수 없다.
#: 그래서 원본 바이트를 들고 있다가 **새 크기의 화면에 다시 먹인다.** 같은 스트림을 같은
#: 순서로 재생하므로 결과는 그 크기에서 원래 보였을 화면과 같다.
#:
#: 256KB 면 보통 터미널 수천 줄이다. 무한정 쌓으면 오래 띄워둔 세션이 메모리를 먹는다.
REPLAY_LIMIT = 256 * 1024

#: 되돌아볼 수 있는 줄 수. 화면 밖으로 밀려난 줄을 이만큼 보관한다.
SCROLLBACK = 2000

#: 한 번 깨어났을 때 삼킬 최대 바이트. 이보다 많으면 다음 차례로 넘긴다 —
#: 한 터미널이 화면을 독차지하지 않게 하는 몫이다.
PUMP_BUDGET = 512 * 1024

#: 휠 한 번에 움직이는 줄 수. 터미널 에뮬레이터들이 대체로 3줄이다.
WHEEL_LINES = 3

#: 안쪽 프로그램이 요청한 모드. pyte 는 private 모드를 `번호 << 5` 로 기록한다(실측).
MOUSE_MODES = frozenset({1000 << 5, 1002 << 5, 1003 << 5})
SGR_MOUSE = 1006 << 5


@dataclass
class Geometry:
    """캔버스 위의 자리와 크기. **이게 세션의 정체성이다** — 저장하고 복원할 값."""

    x: int
    y: int
    width: int
    height: int

    def inner(self) -> tuple[int, int]:
        """테두리와 제목 줄을 뺀 실제 표시 칸 수 (열, 행)."""
        return (
            max(self.width - BORDER, 1),
            max(self.height - BORDER - TITLE_BAR, 1),
        )


class Vt(pyte.Screen):
    """안에서 도는 프로그램이 무엇을 뱉든 **앱을 죽이지 않는** 화면.

    두 가지를 한다.

    **질의에 답한다.** 프로그램은 커서 위치 같은 것을 터미널에 묻고(`ESC[6n`) 답을
    기다린다. 답하지 않으면 기다리다 멈추거나 이상하게 그린다. pyte 는 답할 내용을
    `write_process_input` 으로 넘겨 주므로 그대로 PTY 에 돌려보낸다.

    **모르는 것에 죽지 않는다.** pyte 0.8 의 `report_device_status` 는 `private`
    인자를 모른다. claude code 가 보내는 `ESC[?6n` 이 정확히 그 경우이고, 그대로 두면
    **터미널 하나가 앱 전체를 넘어뜨린다**(실측).
    """

    def __init__(self, columns: int, lines: int, history: int = SCROLLBACK) -> None:
        super().__init__(columns, lines)
        #: 답을 돌려보낼 곳. 붙기 전에는 없다.
        self.reply = None
        #: 위로 흘러간 줄들. **pyte 는 이걸 안 준다** — `Screen` 은 화면 밖 줄을 그냥 버린다.
        self.history: deque[str] = deque(maxlen=history)

    def index(self) -> None:
        """한 줄 내려간다. 맨 아래였다면 **맨 윗줄이 화면 밖으로 밀려난다.**

        pyte 는 그 줄을 버린다. 여기서 가로채 보관해야 사용자가 되돌아볼 수 있다.
        `index` 가 실제로 밀어내는 지점이라 여기가 유일하게 맞는 자리다.
        """
        top, bottom = self.margins or (0, self.lines - 1)
        if self.cursor.y == bottom:
            self.history.append(self.line(top))
        super().index()

    def line(self, y: int) -> str:
        """한 줄만 글자로 만든다.

        **`display` 를 쓰면 안 된다.** 그건 화면 **전체**를 매번 새로 만드는 속성이라,
        줄 하나 밀릴 때마다 부르면 처리량이 12배 떨어진다 (1.59 → 0.13 MB/s, 실측).
        넓은 글자의 뒷칸은 `data` 가 빈 문자열이라 그냥 이어 붙여도 결과가 같다.
        """
        row = self.buffer[y]
        if not row:
            return ""
        # **쓰인 칸까지만 본다.** pyte 의 행은 성긴 dict 라 안 쓴 칸은 아예 없다.
        # 폭 전체를 훑으면 `seq` 처럼 짧은 줄이 쏟아질 때 스무 배를 헛일한다
        # (140칸 훑기 × 줄마다).
        width = min(max(row) + 1, self.columns)
        # 오른쪽 공백은 보관하지 않는다 — 화면에 그릴 때 어차피 잘라내고,
        # 줄마다 폭만큼의 공백을 이천 줄 쌓으면 그게 곧 메모리다.
        return "".join(row[x].data for x in range(width)).rstrip()

    def reset(self) -> None:
        super().reset()
        # `clear` 등으로 화면이 초기화돼도 지나간 것은 지나간 것이다. 기록은 남긴다.

    def write_process_input(self, data: str) -> None:
        if self.reply is not None:
            self.reply(data)

    def report_device_status(self, mode: int, private: bool = False) -> None:
        # private 여부는 구분하지 않는다 — 어느 쪽이든 커서 위치를 묻는 것이다.
        super().report_device_status(mode)


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
        #: 이름. **테두리 제목으로 쓰지 않는다** — 제목 줄을 우리가 직접 그리고,
        #: 둘 다 쓰면 같은 이름이 두 번 나온다(실측).
        self.name_ = title
        self.cwd = cwd
        cols, rows = geometry.inner()
        #: pyte 화면. Textual 위젯의 `.screen` 은 소속 Screen 이라 이름을 겹칠 수 없다.
        self.vt = Vt(cols, rows)
        self.vt.reply = self.send
        self.stream = pyte.ByteStream(self.vt)
        #: 화면에 먹이다 삼킨 예외 수. 조용히 이상해지는 것을 알아채기 위한 것이다.
        self.glitches = 0
        #: 접혔는가. 접히면 제목 줄만 남고 **프로세스는 그대로 돈다.**
        self.folded = False
        #: 펼쳤을 때 돌아갈 높이.
        self._height_before_fold = geometry.height
        #: 위로 되돌아간 줄 수. 0 이면 맨 아래(지금)를 보고 있다.
        #: **`scroll_offset` 이라 부르면 안 된다** — Widget 의 읽기 전용 속성과 겹친다.
        self.history_offset = 0
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
        # **타이머로 훑지 않는다.** 패널마다 초당 20번을 깨우면 아무 일이 없어도
        # 터미널 수만큼 CPU 를 쓴다(실측: 유휴 0.3%). 읽을 게 생겼을 때만 깨운다.
        asyncio.get_running_loop().add_reader(fd, self._pump)

    def _stop_reading(self) -> None:
        if self.fd is None:
            return
        with contextlib.suppress(RuntimeError, ValueError, OSError):
            asyncio.get_running_loop().remove_reader(self.fd)

    def close(self) -> None:
        """자식 프로세스를 정리한다. 남겨두면 좀비가 쌓인다."""
        self._stop_reading()
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
        """읽을 게 생겼다. **있는 만큼 몰아 읽는다.**

        한 번 읽고 바로 그리면, 쏟아지는 출력에서는 4KB 마다 화면을 다시 그리게 된다.
        모아서 한 번에 해석하고 한 번만 그리는 편이 훨씬 싸다
        (실측: `seq 1 200000` 이 6.2초 → 1.6초).

        그래도 **한 번에 삼키는 양에는 한도를 둔다.** 끝없이 뱉는 프로그램이 있으면
        이 함수가 안 돌아와서 앱 전체가 멈춘다 — 다른 터미널도, 키 입력도.
        """
        if self.fd is None:
            return
        chunks: list[bytes] = []
        total = 0
        while total < PUMP_BUDGET:
            try:
                data = os.read(self.fd, 65536)
            except BlockingIOError:
                break  # 지금은 더 없다
            except OSError:
                # 자식이 죽었다. 화면은 마지막 상태로 남겨둔다 —
                # 갑자기 비면 사용자는 무슨 일이 있었는지 알 수 없다.
                self._stop_reading()
                self.fd = None
                break
            if not data:  # EOF — 자식이 끝났다
                self._stop_reading()
                self.fd = None
                break
            chunks.append(data)
            total += len(data)

        if not chunks:
            return
        data = b"".join(chunks)
        self._feed(data)
        self._remember(data)
        self.refresh()

    def _feed(self, data: bytes) -> None:
        """화면에 먹인다. **여기서 나는 예외가 앱을 죽여서는 안 된다.**

        안에서 도는 것은 우리가 고를 수 없는 남의 프로그램이고, 터미널 에뮬레이션은
        완전하지 않다. 한 터미널의 출력이 다른 터미널까지 끌고 내려가는 건 최악이다.
        """
        try:
            self.stream.feed(data)
        except Exception:  # noqa: BLE001 - 무엇이 오든 화면 하나가 앱을 넘어뜨리면 안 된다
            self.glitches += 1

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
        # 접두키와 그 다음 한 글자는 앱의 것이다. 여기서 멈추면 단축키가 영영 닿지 않는다.
        if event.key == PREFIX or getattr(self.app, "prefix_armed", False):
            return
        # `event.character` 만 보면 ctrl 키도 enter 도 화살표도 전달되지 않는다(실측).
        text = sequence(event.key, event.character)
        if text is not None:
            self.send(text)
            event.stop()
            event.prevent_default()

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

    def _room(self) -> tuple[int, int]:
        """캔버스가 내주는 칸. 아직 붙기 전이면 알 수 없다."""
        parent = self.parent
        size = getattr(parent, "size", None)
        if size is None or not size.width or not size.height:
            return 0, 0
        return size.width, size.height

    def move_to(self, x: int, y: int) -> None:
        """자리를 옮긴다. **캔버스 밖으로는 못 나간다.**

        나갈 수 있게 두면 끌다가 놓친 터미널을 되찾을 방법이 없다. 자유 배치는
        아무 데나 둘 수 있다는 뜻이지 잃어버릴 수 있다는 뜻이 아니다.
        """
        room_w, room_h = self._room()
        g = self.geometry_
        self.geometry_.x = max(0, min(x, room_w - g.width) if room_w else x)
        self.geometry_.y = max(0, min(y, room_h - g.height) if room_h else y)
        self._apply_geometry()

    def resize_to(self, width: int, height: int) -> None:
        room_w, room_h = self._room()
        g = self.geometry_
        if room_w:
            width = min(width, room_w - g.x)
            height = min(height, room_h - g.y)
        self.geometry_.width = max(MIN_WIDTH, width)
        self.geometry_.height = max(MIN_HEIGHT, height)
        cols, rows = self.geometry_.inner()

        # ★ `vt.resize()` 를 쓰지 않는다. 줄이 줄어들면 위쪽을 버려서 화면이 비어버린다(실측).
        #   새 화면을 만들고 보관해둔 바이트를 다시 먹여 그 크기에서의 화면을 얻는다.
        self.vt = Vt(cols, rows)
        self.vt.reply = self.send
        self.stream = pyte.ByteStream(self.vt)
        if self._replay:
            self._feed(bytes(self._replay))

        self._apply_geometry()
        self._sync_pty_size()

    # ── 마우스 ──────────────────────────────────────────────────────────────
    #
    # **아무 데나 끌면 창이 움직이던 것이 문제였다.** 안쪽 글자를 고르려고 끌어도 창이
    # 따라왔다. 창을 옮기는 곳은 제목 줄뿐이다 — 윈도우·맥의 창과 같다.
    def button_at(self, x: int) -> str | None:
        """제목 줄의 이 칸에 버튼이 있는가. 없으면 `None`."""
        right = self.geometry_.width - 1  # 오른쪽 테두리
        for _glyph, action, width in BUTTONS:
            right -= width
            if right <= x < right + width:
                return action
        return None

    def _title_row(self) -> int:
        """제목 줄의 위젯 안 y 좌표. 0 은 테두리다."""
        return TITLE_BAR

    def on_mouse_down(self, event: MouseDown) -> None:
        self.focus()
        g = self.geometry_

        # 오른쪽 아래 모서리 — 크기 조절. 접혀 있으면 잡히지 않는다.
        if not self.folded and event.x >= g.width - GRIP and event.y >= g.height - GRIP:
            self._resizing = True
            self._drag_origin = (event.screen_x, event.screen_y)
            self.capture_mouse()
            event.stop()
            return

        # 위쪽 테두리와 제목 줄 — 여기서만 창이 끌린다
        if event.y <= self._title_row():
            action = self.button_at(event.x) if event.y == self._title_row() else None
            if action == "close":
                self.close()
                self.remove()
            elif action == "minimize":
                self.toggle_fold()
            else:
                self._drag_origin = (event.screen_x, event.screen_y)
                self.capture_mouse()
            event.stop()
            return

        # 그 밖은 **건드리지 않는다.** Textual 이 글자 선택을 맡고 있어서
        # 여기서 event.stop() 을 하면 끌어서 고르는 것이 막힌다(실측).
        # 고른 글자는 Cmd+C 로 복사된다 — Ctrl+C 는 안쪽 프로그램의 것이므로 겹치지 않는다.

    def toggle_fold(self) -> None:
        """접거나 편다. **프로세스는 건드리지 않는다** — 접는 것은 끄는 것이 아니다."""
        if self.folded:
            self.folded = False
            self.resize_to(self.geometry_.width, self._height_before_fold)
        else:
            self._height_before_fold = self.geometry_.height
            self.folded = True
            self.geometry_.height = FOLDED_HEIGHT
            self._apply_geometry()
        self.refresh()

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
    # ── 휠 ──────────────────────────────────────────────────────────────────
    def _wants_mouse(self) -> bool:
        """안쪽 프로그램이 마우스를 직접 받겠다고 했는가.

        claude code·vim 처럼 자기 화면을 스스로 스크롤하는 것들이 이걸 켠다.
        **그럴 때 우리가 대신 스크롤하면 안 된다** — 사용자는 그 프로그램의 내용을
        움직이려던 것이고, 우리 스크롤백은 그 프로그램이 그린 그림일 뿐이다.
        """
        return bool(self.vt.mode & MOUSE_MODES)

    def _report_wheel(self, up: bool, x: int, y: int) -> None:
        """휠을 마우스 보고로 안쪽에 전달한다."""
        button = 64 if up else 65
        if SGR_MOUSE in self.vt.mode:
            self.send(f"\x1b[<{button};{x + 1};{y + 1}M")
        else:
            # 옛 방식은 좌표에 32 를 더해 글자로 싣는다. 223칸을 넘으면 표현할 수 없다.
            self.send(f"\x1b[M{chr(32 + button)}{chr(33 + x)}{chr(33 + y)}")

    def _scroll(self, lines: int, x: int, y: int) -> None:
        # 프로세스가 끝났어도 되돌아볼 수 있어야 한다 — **끝난 세션이야말로 다시 볼 것이다.**
        # 넘길 곳이 있을 때만 넘기고, 나머지는 우리가 스크롤한다.
        if self.fd is not None and self._wants_mouse():
            for _ in range(abs(lines)):
                self._report_wheel(lines > 0, x, y)
            return
        limit = len(self.vt.history)
        self.history_offset = max(0, min(self.history_offset + lines, limit))
        self.refresh()

    def on_mouse_scroll_up(self, event: MouseScrollUp) -> None:
        event.stop()
        self._scroll(WHEEL_LINES, event.x, event.y)

    def on_mouse_scroll_down(self, event: MouseScrollDown) -> None:
        event.stop()
        self._scroll(-WHEEL_LINES, event.x, event.y)

    def _title_line(self) -> Text:
        """제목 줄. 왼쪽에 이름, 오른쪽에 버튼.

        창처럼 보여야 창처럼 다뤄진다 — 여기가 잡는 곳이라는 걸 생김새로 알려야 한다.
        """
        width = max(self.geometry_.width - BORDER, 1)
        buttons = "".join(glyph.center(w) for glyph, _, w in reversed(BUTTONS))
        room = max(width - len(buttons), 0)

        name = f" {self.name_}"
        if self.history_offset:
            # 지금 보고 있는 곳이 맨 아래가 아니면 그렇다고 알려야 한다.
            name += f"  ↑{self.history_offset}"
        # 잡는 곳이라는 걸 생김새로 알려야 한다. 색을 하드코딩하지 않으려고 반전을 쓴다 —
        # 어떤 테마에서도 배경과 구분된다.
        style = "reverse" if self.has_focus else "dim"
        line = Text(name[:room].ljust(room), style=style)
        line.append(buttons[:width], style=style)
        return line

    def render(self) -> Text:
        # `display` 는 줄마다 wcwidth 를 돌리고 assert 를 건다. 우리 것이 더 싸다.
        lines = [self.vt.line(y) for y in range(self.vt.lines)]
        if self.history_offset:
            # 되돌아간 만큼 기록에서 끌어와 위에 붙이고, 그만큼 아래를 잘라낸다.
            back = list(self.vt.history)[-self.history_offset :]
            lines = (back + lines)[: self.vt.lines]

        out = self._title_line()
        if not self.folded:
            for line in lines:
                out.append("\n")
                out.append(line)
        return out
