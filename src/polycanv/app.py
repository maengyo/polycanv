"""polycanv 본체."""

from __future__ import annotations

import contextlib
import os
import tempfile
from pathlib import Path

from textual.app import App, ComposeResult
from textual.binding import Binding
from textual.css.query import NoMatches
from textual.widgets import Static

from . import settings as settings_module
from . import theme as theme_module
from . import tools as tools_module
from .bridge import PANE_ENV, SOCKET_ENV, Bridge
from .canvas import Canvas
from .hooks import write_claude_settings
from .keymap import sequence
from .keys import COMMANDS, HINT, PREFIX, PREFIX_BYTE
from .launcher import ToolPicker
from .terminal import TerminalPanel
from .tools import Tool


class PolycanvApp(App):
    """흩어진 세션을 한 캔버스 위에 모은다."""

    TITLE = "polycanv"

    CSS = """
    Screen { background: $background; }
    #hint {
        dock: bottom;
        height: 1;
        padding: 0 1;
        color: $text-muted;
        background: $surface;
    }
    #hint.armed { color: $text; background: $accent; }
    """

    #: 앱 단축키는 BINDINGS 가 아니라 접두키로 받는다 — `keys.py` 참고.
    #:
    #: 여기 남은 하나는 **되찾아 오기 위한 것**이다. Textual 은 `ctrl+q` 를 priority
    #: 바인딩으로 잡아 위젯보다 먼저 가져간다(실측). 그러면 안에서 도는 프로그램에
    #: `ctrl+q` 를 보낼 방법이 없다. 같은 키를 덮어써서 안쪽으로 돌려보낸다.
    BINDINGS = [Binding("ctrl+q", "pass_through('ctrl+q')", show=False, priority=True)]

    #: 명령 팔레트가 `ctrl+p` 를 가져간다. readline 의 이전 줄이라 뺏을 수 없다.
    ENABLE_COMMAND_PALETTE = False

    def __init__(self, theme: str | None = None) -> None:
        super().__init__()
        #: 이번 실행에만 쓰는 테마. 없으면 저장된 것을 따른다.
        self._theme_override = theme
        self.settings = settings_module.Settings()
        #: 패인 식별자 → 패널. 훅이 보내오는 상태를 어디에 붙일지 찾는 표.
        self.panes: dict[str, TerminalPanel] = {}
        self._next_pane = 0
        self.bridge = Bridge(self._on_status)
        #: 훅 설정 파일을 두는 곳. polycanv 가 끝나면 사라진다 —
        #: **사용자의 설정 파일은 건드리지 않는다.**
        self._workdir = tempfile.TemporaryDirectory(prefix="polycanv-", ignore_cleanup_errors=True)
        #: 접두키를 눌러 다음 한 글자를 기다리는 중인가. 패널이 이 값을 보고 키를 양보한다.
        self.prefix_armed = False
        self.config = tools_module.ToolConfig(tools_module.defaults(), tools_module.config_path())

    def compose(self) -> ComposeResult:
        yield Canvas()
        yield Static(HINT, id="hint")

    async def on_mount(self) -> None:
        for t in theme_module.THEMES:
            self.register_theme(t)
        self.settings = settings_module.load()
        # 깃발로 준 것은 이번만이다 — 저장된 선택을 조용히 덮어쓰지 않는다.
        self.theme = self._theme_override or self.settings.theme

        await self.bridge.start()
        self.config = tools_module.load()
        if self.config.problem:
            self.notify(self.config.problem, severity="warning")
        # 빈 화면으로 시작하면 무엇을 해야 할지 알 수 없다. 하나는 띄워둔다.
        #
        # 여기서 목록을 띄우지 않는 건 의도다. 첫 화면이 질문이면 도구를 고를 때까지
        # 아무것도 못 보고, 그건 지금까지 있던 빠른 시작을 없애는 일이다.
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

    # ── 접두키 ──────────────────────────────────────────────────────────────
    def _arm(self, armed: bool) -> None:
        self.prefix_armed = armed
        # 눌린 걸 화면이 알려줘야 한다. 아무 표시가 없으면 먹었는지 알 수 없다.
        with contextlib.suppress(NoMatches):
            self.query_one("#hint", Static).set_class(armed, "armed")

    async def on_key(self, event) -> None:
        """접두키와 그 다음 한 글자를 받는다.

        패널이 키를 먼저 보지만 이 둘만은 넘겨 준다(`TerminalPanel.on_key`).
        """
        if self.prefix_armed:
            self._arm(False)
            event.stop()
            event.prevent_default()
            if event.key == PREFIX:
                # 두 번 누르면 안쪽으로 그대로 보낸다. 안 그러면 접두키를 쓰는
                # 프로그램(중첩된 tmux 등)에 이 키를 전달할 방법이 없다.
                focused = self.focused
                if isinstance(focused, TerminalPanel):
                    focused.send(PREFIX_BYTE)
                return
            action = COMMANDS.get((event.character or "").lower())
            if action:
                await self.run_action(action)
            return

        if event.key == PREFIX:
            self._arm(True)
            event.stop()
            event.prevent_default()

    # ── 동작 ────────────────────────────────────────────────────────────────
    def _on_status(self, pane: str, event) -> None:
        panel = self.panes.get(pane)
        if panel is not None:
            panel.apply_status(event)

    def _open(self, command: list[str], title: str, cwd: str | None = None) -> None:
        pane = str(self._next_pane)
        self._next_pane += 1
        env = {SOCKET_ENV: str(self.bridge.path), PANE_ENV: pane}
        command = self._with_hooks(command, pane)

        panel = self.canvas.open_terminal(command, title=title, cwd=cwd, env=env)
        self.panes[pane] = panel
        self.call_after_refresh(panel.focus)

    def _with_hooks(self, command: list[str], pane: str) -> list[str]:
        """훅을 얹은 명령.

        **사용자 설정 파일을 고치지 않는다.** claude 는 `--settings <파일>` 로 설정을
        덧씌울 수 있고(실측), 인증은 그대로 쓴다. 그 길로만 간다.

        도구를 **이름이 아니라 실행 파일로** 판정한다 — 목록에서 이름을 바꿔도
        훅이 따라와야 한다.
        """
        exe = Path(command[0]).name
        if exe in ("claude", "qwen"):
            path = write_claude_settings(Path(self._workdir.name), pane)
            return [*command, "--settings", str(path)]
        return command

    def action_pass_through(self, key: str) -> None:
        """앱이 가로챈 키를 안쪽 터미널로 돌려보낸다."""
        focused = self.focused
        text = sequence(key, None)
        if isinstance(focused, TerminalPanel) and text is not None:
            focused.send(text)

    def action_new_shell(self) -> None:
        shell = os.environ.get("SHELL", "/bin/sh")
        self._open([shell], title=os.path.basename(shell))

    def action_launch(self) -> None:
        def opened(tool: Tool | None) -> None:
            if tool is not None:
                self._open(tool.resolved(), title=tool.name, cwd=tool.resolved_cwd())

        self.push_screen(ToolPicker(self.config.tools), opened)

    def action_toggle_theme(self) -> None:
        """밝은 쪽 ↔ 어두운 쪽. **고른 것은 다음에도 남는다.**"""
        self.theme = theme_module.other(self.theme)
        self.settings.theme = self.theme
        self.settings.save()
        self.notify("밝은 테마" if self.theme == theme_module.LIGHT.name else "어두운 테마")

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
        with contextlib.suppress(Exception):
            self._workdir.cleanup()
