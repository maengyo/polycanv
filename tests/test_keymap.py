"""키 → PTY 바이트.

이 표가 틀리면 안에서 도는 프로그램이 **조용히** 이상하게 굴기 때문에 눈으로는 못 찾는다.
"""

from __future__ import annotations

import pytest

from polycanv.keymap import sequence


@pytest.mark.parametrize(
    ("key", "expected"),
    [
        ("enter", "\r"),  # 이게 없으면 명령을 실행할 수 없다
        ("ctrl+c", "\x03"),  # 이게 없으면 도는 에이전트를 멈출 수 없다
        ("ctrl+d", "\x04"),
        ("ctrl+a", "\x01"),
        ("ctrl+z", "\x1a"),
        ("backspace", "\x7f"),  # \x08 을 보내면 유닉스에서 안 지워진다
        ("delete", "\x1b[3~"),
        ("up", "\x1b[A"),
        ("down", "\x1b[B"),
        ("right", "\x1b[C"),
        ("left", "\x1b[D"),
        ("tab", "\t"),
        ("shift+tab", "\x1b[Z"),
        ("escape", "\x1b"),
        ("f1", "\x1bOP"),
        ("f5", "\x1b[15~"),
        ("alt+b", "\x1b\x62"),  # readline 의 한 단어 뒤로
        ("alt+left", "\x1b\x1b[D"),
    ],
)
def test_아는_키는_xterm_과_같이_보낸다(key: str, expected: str) -> None:
    assert sequence(key, None) == expected


def test_글자는_그대로_간다() -> None:
    assert sequence("a", "a") == "a"
    assert sequence("space", None) == " "
    assert sequence("가", "가") == "가"


def test_보낼_것이_없으면_보내지_않는다() -> None:
    """모르는 키를 아무 바이트로나 보내면 화면이 깨진다."""
    assert sequence("f24", None) is None
    assert sequence("ctrl+shift+alt+f7", None) is None
    assert sequence("unknown", None) is None
