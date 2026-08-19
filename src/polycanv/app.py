"""polycanv 본체."""

from __future__ import annotations

import os

from textual.app import App, ComposeResult
from textual.binding import Binding
from textual.css.query import NoMatches

from .canvas import Canvas
from .terminal import TerminalPanel


class PolycanvApp(App):
    """흩어진 세션을 한 캔버스 위에 모은다."""

    TITLE = "polycanv"

    CSS = """
    Screen { background: $panel; }
    """

    BINDINGS = [
        Binding("ctrl+n", "new_shell", "새 터미널"),
        Binding("ctrl+w", "close_focused", "닫기"),
        Binding("ctrl+q", "quit", "종료"),
    ]

    def compose(self) -> ComposeResult:
        yield Canvas()

    def on_mount(self) -> None:
        # 빈 화면으로 시작하면 무엇을 해야 할지 알 수 없다. 하나는 띄워둔다.
        self.action_new_shell()

    @property
    def canvas(self) -> Canvas:
        return self.query_one(Canvas)

    def _panels(self) -> list[TerminalPanel]:
        """지금 살아 있는 패널들. **종료 중에는 캔버스가 이미 없을 수 있다.**"""
        try:
            return self.canvas.panels
        except NoMatches:
            return list(self.query(TerminalPanel))

    def action_new_shell(self) -> None:
        shell = os.environ.get("SHELL", "/bin/sh")
        panel = self.canvas.open_terminal([shell], title=os.path.basename(shell))
        self.call_after_refresh(panel.focus)

    def action_close_focused(self) -> None:
        focused = self.focused
        if isinstance(focused, TerminalPanel):
            focused.close()
            focused.remove()

    def on_unmount(self) -> None:
        # 앱이 죽을 때 자식들을 정리한다. 안 그러면 PTY 와 프로세스가 남는다.
        #
        # 이 시점에는 위젯 트리가 이미 헐린 뒤일 수 있어 캔버스를 못 찾는다.
        # 정리는 실패하면 안 되는 일이라 조회에 기대지 않는다.
        for panel in self._panels():
            panel.close()
