"""접두키.

터미널은 키를 전부 안쪽으로 넘긴다. 그래서 앱 단축키가 닿으려면 **양보가 있어야
한다** — 실제로 `ctrl+n` 이 셸로 흘러가 도구 목록이 안 뜬 적이 있다.
반대로 너무 많이 가져가면 안쪽 CLI 의 조작을 뺏는다. 그 둘을 다 본다.
"""

from __future__ import annotations

from polycanv import keys
from polycanv.app import PolycanvApp
from polycanv.launcher import ToolPicker
from polycanv.terminal import TerminalPanel


async def test_접두키_다음_글자로_목록이_열린다() -> None:
    app = PolycanvApp()
    async with app.run_test() as pilot:
        await pilot.press("ctrl+b")
        await pilot.press("n")
        await pilot.pause()

        assert isinstance(app.screen, ToolPicker)


async def test_평범한_키는_안쪽으로_간다() -> None:
    """앱이 가로채면 안에서 도는 프로그램이 망가진다."""
    app = PolycanvApp()
    async with app.run_test() as pilot:
        panel = app.canvas.panels[0]
        sent: list[str] = []
        panel.send = sent.append  # type: ignore[method-assign]
        panel.focus()
        await pilot.pause()

        for key in ("a", "ctrl+w", "ctrl+n", "ctrl+t", "ctrl+q"):
            await pilot.press(key)
        await pilot.pause()

        assert len(sent) == 5, f"안쪽으로 가야 할 키가 사라졌다: {sent}"


async def test_접두키를_두_번_누르면_안쪽으로_보낸다() -> None:
    """중첩된 tmux 처럼 이 키를 쓰는 프로그램에 전달할 길이 있어야 한다."""
    app = PolycanvApp()
    async with app.run_test() as pilot:
        panel = app.canvas.panels[0]
        sent: list[str] = []
        panel.send = sent.append  # type: ignore[method-assign]
        panel.focus()
        await pilot.pause()

        await pilot.press("ctrl+b")
        assert sent == [], "첫 번째는 앱이 삼킨다"
        await pilot.press("ctrl+b")
        await pilot.pause()

        assert sent == [keys.PREFIX_BYTE]
        assert app.prefix_armed is False


async def test_모르는_글자는_대기를_푼다() -> None:
    """붙잡아 두면 그다음 입력까지 먹는다."""
    app = PolycanvApp()
    async with app.run_test() as pilot:
        await pilot.press("ctrl+b")
        await pilot.press("z")
        await pilot.pause()

        assert app.prefix_armed is False
        assert isinstance(app.screen.focused or app.canvas.panels[0], TerminalPanel)


async def test_안내에_적힌_키가_실제로_동작한다() -> None:
    """안내와 구현이 어긋나면 안내가 거짓말이 된다."""
    for char in keys.COMMANDS:
        assert char in keys.HINT, f"{char} 가 안내에 없다"
    app = PolycanvApp()
    for action in keys.COMMANDS.values():
        assert hasattr(app, f"action_{action}"), f"{action} 동작이 없다"
