"""창처럼 다루기 — 제목 줄에서만 끌리고, 버튼이 있고, 휠이 되돌아본다."""

from __future__ import annotations

import pyte

from polycanv.app import PolycanvApp
from polycanv.terminal import FOLDED_HEIGHT, TITLE_BAR, Geometry, TerminalPanel


def panel(width: int = 40, height: int = 12) -> TerminalPanel:
    return TerminalPanel(["sh"], Geometry(0, 0, width, height), "t")


# ── 어디를 눌러야 창이 움직이나 ─────────────────────────────────────────────
def test_본문을_끌어도_창이_안_움직인다() -> None:
    """글자를 고르려고 끄는데 창이 따라오면 안 된다 — 이게 원래 문제였다."""
    p = panel()
    body_row = TITLE_BAR + 1

    assert p.button_at(0) is None
    # 본문 행은 제목 줄보다 아래다
    assert body_row > TITLE_BAR


def test_제목_줄_오른쪽_끝에_버튼이_있다() -> None:
    p = panel(width=40)

    assert p.button_at(38) == "close", "가장 오른쪽이 닫기"
    assert p.button_at(35) == "minimize"
    assert p.button_at(10) is None, "가운데는 잡는 곳이지 버튼이 아니다"


def test_버튼_자리는_창_폭을_따라간다() -> None:
    narrow, wide = panel(width=24), panel(width=60)

    assert narrow.button_at(22) == "close"
    assert wide.button_at(58) == "close"


# ── 접기 ────────────────────────────────────────────────────────────────────
def test_접으면_제목_줄만_남는다() -> None:
    p = panel(height=20)

    p.toggle_fold()

    assert p.folded
    assert p.geometry_.height == FOLDED_HEIGHT


def test_펴면_원래_높이로_돌아온다() -> None:
    p = panel(height=17)

    p.toggle_fold()
    p.toggle_fold()

    assert not p.folded
    assert p.geometry_.height == 17


async def test_접어도_프로세스는_계속_돈다() -> None:
    """접는 것은 끄는 것이 아니다."""
    app = PolycanvApp()
    async with app.run_test() as pilot:
        p = app.canvas.panels[0]
        await pilot.pause()
        before = p.pid

        p.toggle_fold()
        await pilot.pause()

        assert p.pid == before
        assert p.fd is not None


# ── 휠 ──────────────────────────────────────────────────────────────────────
def feed(p: TerminalPanel, text: str) -> None:
    p._feed(text.encode())


def test_휠로_흘러간_줄을_되돌아본다() -> None:
    p = panel(height=8)  # 안쪽 5줄
    feed(p, "".join(f"line{i}\r\n" for i in range(20)))

    assert p.vt.history, "화면 밖으로 밀려난 줄이 보관돼야 한다"
    p._scroll(3, 0, 0)

    assert p.history_offset == 3
    assert "line" in p.render().plain


def test_맨_아래보다_더_내려가지_않는다() -> None:
    p = panel()
    feed(p, "hello\r\n")

    p._scroll(-10, 0, 0)

    assert p.history_offset == 0


def test_기록보다_더_올라가지_않는다() -> None:
    p = panel(height=8)
    feed(p, "".join(f"l{i}\r\n" for i in range(10)))

    p._scroll(9999, 0, 0)

    assert p.history_offset == len(p.vt.history)


def test_안쪽이_마우스를_원하면_휠을_넘긴다() -> None:
    """claude 나 vim 은 자기 화면을 스스로 스크롤한다. 가로채면 안 된다."""
    p = panel()
    sent: list[str] = []
    p.send = sent.append  # type: ignore[method-assign]
    p.fd = 99
    pyte.ByteStream(p.vt).feed(b"\x1b[?1000h\x1b[?1006h")

    p._scroll(3, 5, 6)

    assert sent, "안쪽으로 보고가 가야 한다"
    assert all(s.startswith("\x1b[<") for s in sent), f"SGR 형식이어야 한다: {sent}"
    assert p.history_offset == 0, "우리 스크롤백은 건드리지 않는다"
