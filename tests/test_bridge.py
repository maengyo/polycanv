"""훅 → 소켓 → 신호등.

**셸과 파이썬이 어긋나면 신호등이 조용히 안 켜진다.** 오류가 아니라 무반응으로 나타나므로
와이어 포맷을 여기서 못박는다.
"""

from __future__ import annotations

import asyncio
import json
import os
import socket

from polycanv.bridge import PANE_ENV, SOCKET_ENV, Bridge, send
from polycanv.hooks import state_from_payload
from polycanv.status import AgentState, Source


def short_path():
    """유닉스 소켓 경로는 100자쯤에서 잘린다. pytest 의 tmp_path 는 그보다 길다."""
    import os
    import tempfile

    fd, name = tempfile.mkstemp(prefix="pc-", suffix=".sock", dir="/tmp")
    os.close(fd)
    os.unlink(name)
    from pathlib import Path

    return Path(name)


async def test_보낸_상태가_그대로_도착한다(monkeypatch, tmp_path) -> None:
    got: list = []
    bridge = Bridge(lambda pane, ev: got.append((pane, ev)))
    monkeypatch.setattr(bridge, "path", short_path())
    await bridge.start()
    monkeypatch.setenv(SOCKET_ENV, str(bridge.path))

    assert send("finished", "3")
    await asyncio.sleep(0.05)
    await bridge.stop()

    assert got
    pane, event = got[0]
    assert pane == "3"
    assert event.state is AgentState.FINISHED
    assert event.source is Source.HOOK


async def test_이상한_줄에_죽지_않는다(monkeypatch, tmp_path) -> None:
    """훅이 보내는 것은 남의 프로그램이 만든 문자열이다."""
    got: list = []
    bridge = Bridge(lambda pane, ev: got.append((pane, ev)))
    monkeypatch.setattr(bridge, "path", short_path())
    await bridge.start()

    def raw(line: bytes) -> None:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
            s.connect(str(bridge.path))
            s.sendall(line + b"\n")

    for line in (b"{", b"[]", b'{"pane":"1"}', '{"pane":"1","state":"없음"}'.encode(), b"\xff\xfe"):
        raw(line)
    await asyncio.sleep(0.05)
    raw(json.dumps({"pane": "1", "state": "running"}).encode())
    await asyncio.sleep(0.05)
    await bridge.stop()

    assert len(got) == 1, "쓰레기는 버리고 멀쩡한 것은 받는다"


def test_소켓이_없어도_훅은_실패하지_않는다(monkeypatch) -> None:
    """훅이 매달리거나 실패하면 CLI 의 턴이 통째로 멈춘다."""
    monkeypatch.setenv(SOCKET_ENV, "/tmp/polycanv-없는소켓.sock")
    assert send("running", "1") is False

    monkeypatch.delenv(SOCKET_ENV, raising=False)
    assert send("running", "1") is False


def test_훅_페이로드에서_상태를_읽는다() -> None:
    assert state_from_payload({"hook_event_name": "Stop"}) is AgentState.FINISHED
    assert state_from_payload({"hook_event_name": "UserPromptSubmit"}) is AgentState.RUNNING
    assert state_from_payload({"hook_event_name": "Notification"}) is AgentState.WAITING
    # 모르는 이름이면 **추측해서 켜지 않는다**
    assert state_from_payload({"hook_event_name": "무엇인가"}) is None
    assert state_from_payload({}) is None


def test_훅_진입점은_무엇이_와도_0으로_끝난다(monkeypatch, capsys) -> None:
    from polycanv.hook_entry import run

    monkeypatch.setenv(PANE_ENV, "1")
    monkeypatch.setattr("sys.stdin", os.fdopen(os.open(os.devnull, os.O_RDONLY)))
    assert run() == 0


async def test_앱이_받은_상태를_그_터미널에_붙인다() -> None:
    from polycanv.app import PolycanvApp
    from polycanv.status import StatusEvent, now_ms

    app = PolycanvApp()
    async with app.run_test() as pilot:
        pane, panel = next(iter(app.panes.items()))

        app._on_status(pane, StatusEvent(AgentState.FINISHED, Source.HOOK, now_ms()))
        await pilot.pause()

        assert panel.status.state is AgentState.FINISHED
        assert "●" in panel.render().plain


async def test_들여다보면_완료가_풀린다() -> None:
    """🔴 은 확인하면 꺼진다 — 그게 이 색의 뜻이다."""
    from polycanv.app import PolycanvApp
    from polycanv.status import StatusEvent, now_ms

    app = PolycanvApp()
    async with app.run_test() as pilot:
        app.action_new_shell()
        await pilot.pause()
        pane, panel = list(app.panes.items())[0]
        other = list(app.panes.values())[1]
        other.focus()
        await pilot.pause()

        app._on_status(pane, StatusEvent(AgentState.FINISHED, Source.HOOK, now_ms()))
        await pilot.pause()
        assert panel.status.state is AgentState.FINISHED

        panel.focus()
        await pilot.pause()

        assert panel.status.state is AgentState.IDLE


async def test_승인_대기는_들여다봐도_남는다() -> None:
    from polycanv.app import PolycanvApp
    from polycanv.status import StatusEvent, now_ms

    app = PolycanvApp()
    async with app.run_test() as pilot:
        pane, panel = next(iter(app.panes.items()))
        app._on_status(pane, StatusEvent(AgentState.WAITING, Source.HOOK, now_ms()))
        panel.focus()
        await pilot.pause()

        assert panel.status.state is AgentState.WAITING


def test_훅을_얹어도_사용자_설정은_건드리지_않는다() -> None:
    """되돌리기 어렵고 다른 도구와 부딪힌다. 덧씌우는 길로만 간다."""
    from polycanv.app import PolycanvApp

    app = PolycanvApp()
    command = app._with_hooks(["claude"], "1")

    assert command[:1] == ["claude"]
    assert "--settings" in command
    # 이름이 아니라 실행 파일로 판정한다
    assert app._with_hooks(["/usr/local/bin/claude"], "2")[-2] == "--settings"
    assert app._with_hooks(["sh"], "3") == ["sh"]


def test_죽은_세션의_소켓을_치운다(tmp_path) -> None:
    """강제 종료되면 남는다. 쌓이면 다음 사람이 죽은 소켓에 붙어 헷갈린다."""
    from polycanv.bridge import sweep

    dead = tmp_path / "polycanv-999999.sock"
    alive = tmp_path / f"polycanv-{os.getpid()}.sock"
    other = tmp_path / "다른것.sock"
    for f in (dead, alive, other):
        f.touch()

    sweep(tmp_path)

    assert not dead.exists(), "죽은 것은 치운다"
    assert alive.exists(), "살아 있는 세션의 것은 두어야 한다"
    assert other.exists(), "우리 것이 아니면 건드리지 않는다"
