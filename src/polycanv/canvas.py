"""캔버스 — 터미널들이 자유롭게 놓이는 바닥.

타일링이 아니다. 각 터미널이 **자기 자리와 크기를 갖는다.** 그래서 사용자가
"그건 왼쪽 위에 있던 거"라고 기억할 수 있다. 탭 목록으로는 안 되는 일이다.
"""

from __future__ import annotations

from textual.containers import Container

from .terminal import Geometry, TerminalPanel

#: 새 터미널을 놓을 때 이만큼씩 어긋나게 둔다. 정확히 겹치면 뒤엣것이 안 보인다.
CASCADE_STEP = 3

#: 처음 열릴 때의 크기. 화면을 다 덮지 않으면서 내용은 읽히는 정도.
DEFAULT_WIDTH = 48
DEFAULT_HEIGHT = 14


class Canvas(Container):
    """터미널들이 놓이는 영역."""

    DEFAULT_CSS = """
    Canvas {
        width: 100%;
        height: 100%;
        /* 바닥은 **가장 뒤로 물러나는 색**이어야 한다. 패널과 같은 색을 쓰면
           패널이 떠 보이지 않고, 눈이 어디를 봐야 할지 알 수 없다. */
        background: $background;
    }
    """

    def __init__(self) -> None:
        super().__init__()
        self._opened = 0

    def next_geometry(self) -> Geometry:
        """새 터미널이 놓일 자리. 계단식으로 어긋나게 둔다.

        화면 밖으로 나가면 처음으로 되돌아온다 — 안 그러면 새 터미널이
        보이지 않는 곳에 생겨서 사용자는 아무 일도 안 일어난 줄 안다.
        """
        step = self._opened * CASCADE_STEP
        limit_x = max(self.size.width - DEFAULT_WIDTH, 1)
        limit_y = max(self.size.height - DEFAULT_HEIGHT, 1)
        self._opened += 1
        return Geometry(
            x=step % limit_x,
            y=step % limit_y,
            width=DEFAULT_WIDTH,
            height=DEFAULT_HEIGHT,
        )

    def open_terminal(
        self,
        command: list[str],
        title: str,
        cwd: str | None = None,
        env: dict[str, str] | None = None,
    ) -> TerminalPanel:
        panel = TerminalPanel(command, self.next_geometry(), title, cwd=cwd, env=env)
        self.mount(panel)
        return panel

    @property
    def panels(self) -> list[TerminalPanel]:
        return list(self.query(TerminalPanel))
