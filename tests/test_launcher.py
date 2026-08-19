"""도구 목록이 실제로 터미널을 여는가."""

from __future__ import annotations

import pytest

from polycanv.app import PolycanvApp
from polycanv.launcher import ToolPicker
from polycanv.terminal import TerminalPanel
from polycanv.tools import Tool, ToolConfig

CONFIG = ToolConfig(
    tools=[
        Tool(name="shell", command=("sh",)),
        Tool(name="없는것", command=("polycanv-does-not-exist",)),
    ],
    path=None,  # type: ignore[arg-type]
)


@pytest.fixture
def app() -> PolycanvApp:
    app = PolycanvApp()
    return app


async def test_고르면_그_도구로_터미널이_열린다(app: PolycanvApp) -> None:
    async with app.run_test() as pilot:
        app.config = CONFIG
        before = len(app.canvas.panels)

        await pilot.press("ctrl+b")
        await pilot.press("n")
        await pilot.press("enter")
        await pilot.pause()

        panels = app.canvas.panels
        assert len(panels) == before + 1
        assert panels[-1].command == ["sh"]


async def test_없는_도구는_패널을_만들지_않는다(app: PolycanvApp) -> None:
    """열자마자 죽는 패널은 고장으로 보인다. 아예 만들지 않는다."""
    async with app.run_test() as pilot:
        app.config = CONFIG
        before = len(app.canvas.panels)

        await pilot.press("ctrl+b")
        await pilot.press("n")
        await pilot.press("down")
        await pilot.press("enter")
        await pilot.pause()

        assert len(app.canvas.panels) == before
        assert isinstance(app.screen, ToolPicker), "고를 기회는 남아 있어야 한다"


async def test_취소하면_아무_일도_없다(app: PolycanvApp) -> None:
    async with app.run_test() as pilot:
        app.config = CONFIG
        before = len(app.canvas.panels)

        await pilot.press("ctrl+b")
        await pilot.press("n")
        await pilot.press("escape")
        await pilot.pause()

        assert len(app.canvas.panels) == before
        assert not isinstance(app.screen, ToolPicker)


async def test_시작하면_묻지_않고_셸이_뜬다(app: PolycanvApp) -> None:
    """첫 화면이 질문이면 빠른 시작이 사라진다."""
    async with app.run_test():
        assert len(app.canvas.panels) == 1
        assert isinstance(app.canvas.panels[0], TerminalPanel)
