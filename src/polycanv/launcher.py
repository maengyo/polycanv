"""도구 고르기 — `ctrl+n` 이 여는 목록.

**빠른 길을 막지 않는 것이 조건이다.** 첫 항목이 셸이므로 `ctrl+n`, `enter` 두 번이면
전과 똑같이 셸이 열린다. 목록은 그 위에 얹히는 것이지 관문이 아니다.
"""

from __future__ import annotations

from textual.app import ComposeResult
from textual.binding import Binding
from textual.containers import Vertical
from textual.screen import ModalScreen
from textual.widgets import Label, ListItem, ListView

from .tools import Tool


class ToolPicker(ModalScreen[Tool | None]):
    """도구 목록. 고르면 그 도구를, 물리면 `None` 을 돌려준다."""

    DEFAULT_CSS = """
    ToolPicker {
        align: center middle;
        background: $background 60%;
    }
    ToolPicker > Vertical {
        width: 46;
        height: auto;
        max-height: 80%;
        border: round $accent;
        background: $surface;
    }
    ToolPicker Label {
        padding: 0 1;
        color: $text-muted;
    }
    ToolPicker ListView { height: auto; max-height: 20; }
    ToolPicker .missing { color: $text-disabled; }
    """

    BINDINGS = [Binding("escape", "dismiss_picker", "취소")]

    def __init__(self, tools: list[Tool]) -> None:
        super().__init__()
        self.tools = tools

    def compose(self) -> ComposeResult:
        with Vertical() as box:
            box.border_title = "무엇을 띄울까요"
            yield ListView(*[self._row(tool) for tool in self.tools])
            yield Label("enter 선택 · esc 취소")

    def _row(self, tool: Tool) -> ListItem:
        if tool.available():
            return ListItem(Label(tool.name))
        # 없는 도구도 **보여준다.** 감추면 왜 목록에 없는지 알 수 없고,
        # 사용자는 polycanv 가 도구를 못 찾는 건지 지원을 안 하는 건지 구분하지 못한다.
        item = ListItem(Label(f"{tool.name}  — 설치되어 있지 않음"))
        item.add_class("missing")
        return item

    def on_mount(self) -> None:
        self.query_one(ListView).focus()

    def action_dismiss_picker(self) -> None:
        self.dismiss(None)

    def on_list_view_selected(self, event: ListView.Selected) -> None:
        event.stop()
        tool = self.tools[event.list_view.index or 0]
        if not tool.available():
            # 열자마자 죽는 패널을 만들지 않는다. 그건 고장으로 보인다.
            self.notify(
                f"{tool.name}: `{tool.executable}` 을 찾지 못했습니다",
                severity="warning",
            )
            return
        self.dismiss(tool)
