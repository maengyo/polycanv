"""테마.

**기본값을 쓰는 것은 선택이 아니다.** 여태 Textual 기본 테마가 그대로 나오고 있었다.
"""

from __future__ import annotations

from pathlib import Path

from polycanv import settings as settings_module
from polycanv import theme as theme_module
from polycanv.app import PolycanvApp


async def test_우리가_고른_테마로_뜬다() -> None:
    app = PolycanvApp()
    async with app.run_test():
        assert app.theme in {t.name for t in theme_module.THEMES}


async def test_밝은_테마로_띄울_수_있다() -> None:
    app = PolycanvApp(theme=theme_module.LIGHT.name)
    async with app.run_test():
        assert app.theme == theme_module.LIGHT.name
        assert app.current_theme.dark is False


async def test_토글하면_반대쪽으로_간다(monkeypatch, tmp_path: Path) -> None:
    monkeypatch.setattr(settings_module, "settings_path", lambda: tmp_path / "settings.toml")
    app = PolycanvApp(theme=theme_module.DARK.name)
    async with app.run_test() as pilot:
        await pilot.press("ctrl+b")
        await pilot.press("d")
        await pilot.pause()

        assert app.theme == theme_module.LIGHT.name


async def test_고른_테마는_다음에도_남는다(monkeypatch, tmp_path: Path) -> None:
    """고른 것이 남지 않으면 고른 보람이 없다."""
    path = tmp_path / "settings.toml"
    monkeypatch.setattr(settings_module, "settings_path", lambda: path)

    app = PolycanvApp(theme=theme_module.DARK.name)
    async with app.run_test() as pilot:
        await pilot.press("ctrl+b")
        await pilot.press("d")
        await pilot.pause()

    assert settings_module.load(path).theme == theme_module.LIGHT.name


def test_망가진_설정은_기본값으로_버틴다(tmp_path: Path) -> None:
    path = tmp_path / "settings.toml"
    for text in ("theme = ", 'theme = "없는테마"', "쓰레기"):
        path.write_text(text, encoding="utf-8")

        assert settings_module.load(path).theme == theme_module.DARK.name


def test_설정을_못_적어도_죽지_않는다(tmp_path: Path) -> None:
    """테마를 못 적었다고 작업을 막을 이유가 없다."""
    settings_module.Settings(theme="x").save(tmp_path / "없는곳" / "깊은곳" / "s.toml")


def test_강조색은_신호등_색을_쓰지_않는다() -> None:
    """빨강·노랑·초록은 🟢🟡🔴 의 자리다. 테두리가 그 색이면 켜져도 구분이 안 된다."""
    for t in theme_module.THEMES:
        for slot in (t.primary, t.accent):
            r, g, b = (int(slot[i : i + 2], 16) for i in (1, 3, 5))
            assert not (r > g + 40 and r > b + 40), f"{t.name}: {slot} 이 빨강 쪽이다"
            assert not (g > r + 40 and g > b + 40), f"{t.name}: {slot} 이 초록 쪽이다"
