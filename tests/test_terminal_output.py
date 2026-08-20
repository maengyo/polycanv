"""안에서 도는 프로그램의 출력.

**우리가 고를 수 없는 남의 프로그램**이 돌아간다. 무엇을 뱉든 앱이 죽으면 안 된다.
실제로 claude code 를 띄웠더니 `ESC[?6n` 하나에 polycanv 가 통째로 넘어갔다.
"""

from __future__ import annotations

from polycanv.terminal import Geometry, TerminalPanel, Vt


def panel() -> TerminalPanel:
    return TerminalPanel(["sh"], Geometry(0, 0, 40, 12), "t")


def test_커서_위치_질의에_죽지_않는다() -> None:
    """claude code 가 보내는 것. pyte 0.8 의 처리기는 이 인자를 모른다."""
    p = panel()
    replies: list[str] = []
    p.vt.reply = replies.append

    p._feed(b"\x1b[?6n")

    assert p.glitches == 0, "예외를 삼킨 게 아니라 제대로 처리해야 한다"
    assert replies, "질의에 답하지 않으면 프로그램이 기다리다 멈춘다"


def test_평범한_질의에도_답한다() -> None:
    p = panel()
    replies: list[str] = []
    p.vt.reply = replies.append

    p._feed(b"\x1b[6n")

    assert replies


def test_해석할_수_없는_출력이_앱을_넘어뜨리지_않는다() -> None:
    p = panel()

    p._feed(b"\x1b[?99999999999999999999n")
    p._feed(b"hello")

    assert "hello" in p.vt.display[0], "한 번 삐끗해도 그다음 출력은 계속 그려야 한다"


def test_답할_곳이_없어도_괜찮다() -> None:
    """붙기 전이나 닫힌 뒤에도 질의가 올 수 있다."""
    vt = Vt(20, 5)

    vt.write_process_input("\x1b[1;1R")
