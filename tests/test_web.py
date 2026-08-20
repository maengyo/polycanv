"""브라우저로 열기.

**포트가 막혔다고 트레이스백을 보이면 안 된다.** 8000 은 흔해서 이미 쓰이는 일이 잦다.
"""

from __future__ import annotations

import socket

from polycanv import web


def held_port() -> tuple[socket.socket, int]:
    s = socket.socket()
    s.bind((web.HOST, 0))
    s.listen(1)
    return s, s.getsockname()[1]


def test_지정한_포트가_비어_있으면_그걸_쓴다() -> None:
    assert web.choose_port(web.free_port()) is not None


def test_지정한_포트가_막혀_있으면_옮기지_않는다(capsys) -> None:
    """콕 집어 말한 주소로 접속하려던 것이다. 말없이 옮기면 더 나쁘다."""
    sock, port = held_port()
    try:
        assert web.choose_port(port) is None
        assert str(port) in capsys.readouterr().err
    finally:
        sock.close()


def test_지정하지_않았고_기본_포트가_막혔으면_비켜준다(monkeypatch) -> None:
    """지정하지도 않은 기본값 때문에 실행이 막히면 그건 우리 사정이다."""
    sock, port = held_port()
    try:
        monkeypatch.setattr(web, "DEFAULT_PORT", port)

        chosen = web.choose_port(None)

        assert chosen is not None
        assert chosen != port
        assert port < chosen <= port + web.NEARBY, "외우기 쉽게 바로 옆 번호를 준다"
    finally:
        sock.close()


def test_루프백_밖으로는_열지_않는다() -> None:
    """host 를 바꾸는 길을 두지 않은 것은 잊어서가 아니다."""
    assert web.HOST == "127.0.0.1"


def test_web_구성요소가_없으면_안내한다(monkeypatch, capsys) -> None:
    import builtins

    real = builtins.__import__

    def fail(name, *args, **kwargs):
        if name.startswith("textual_serve"):
            raise ImportError(name)
        return real(name, *args, **kwargs)

    monkeypatch.setattr(builtins, "__import__", fail)

    assert web.serve() == 1
    assert "web" in capsys.readouterr().err
