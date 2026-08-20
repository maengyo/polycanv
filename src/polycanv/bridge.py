"""훅에서 오는 상태를 받는 통로.

CLI 훅은 **다른 프로세스**에서, 우리와 무관한 순간에 실행된다. 그것이 polycanv 에게
말을 걸 길이 있어야 한다. zellij 판에서는 `zellij pipe` 였고, 여기서는 유닉스 소켓이다.

터미널을 띄울 때 두 가지를 환경변수로 물려준다:

    POLYCANV_SOCKET   어디로 보낼지
    POLYCANV_PANE     누구의 상태인지

훅은 `polycanv --hook` 을 부르고, 그것이 stdin 의 JSON 을 읽어 이 소켓에 한 줄 보낸다.

**훅은 절대 기다리면 안 된다.** 훅이 매달리면 CLI 의 턴이 통째로 멈춘다 — 신호등 하나
때문에 사용자의 작업을 세우는 것은 최악의 거래다. 그래서 보내는 쪽에 짧은 시한을 걸고,
실패하면 조용히 포기한다.
"""

from __future__ import annotations

import asyncio
import contextlib
import json
import os
import socket
import tempfile
from collections.abc import Callable
from pathlib import Path

from .status import AgentState, Source, StatusEvent, now_ms

SOCKET_ENV = "POLYCANV_SOCKET"
PANE_ENV = "POLYCANV_PANE"

#: 훅이 소켓에 매달려 있을 수 있는 최대 시간. 넘으면 포기한다.
SEND_TIMEOUT = 0.5

#: 유닉스 소켓 경로 한도. 넘으면 조용히가 아니라 요란하게 실패한다.
PATH_LIMIT = 100


def socket_path() -> Path:
    """이 실행에 쓸 소켓 자리.

    유닉스 소켓 경로는 100자 남짓에서 잘린다. 긴 임시 디렉터리 이름을 쓰면 조용히 실패하므로
    짧게 잡는다.
    """
    name = f"polycanv-{os.getpid()}.sock"
    base = Path(os.environ.get("TMPDIR", tempfile.gettempdir()))
    path = base / name
    if len(str(path)) > PATH_LIMIT:
        # 길면 `AF_UNIX path too long` 으로 **묶는 순간** 터진다(실측).
        # 짧은 자리로 물러난다 — 신호등 하나 때문에 앱이 안 뜨면 안 된다.
        path = Path("/tmp") / name
    return path


def send(state: str, pane: str, source: str = "hook") -> bool:
    """훅 쪽에서 부른다. 실패해도 **예외를 내지 않는다.**"""
    path = os.environ.get(SOCKET_ENV)
    if not path:
        return False
    line = json.dumps({"pane": pane, "state": state, "source": source, "at_ms": now_ms()})
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
            s.settimeout(SEND_TIMEOUT)
            s.connect(path)
            s.sendall(line.encode() + b"\n")
    except OSError:
        # polycanv 가 이미 닫혔거나 소켓이 없다. 훅이 그것 때문에 실패하면 안 된다.
        return False
    return True


def sweep(directory: Path) -> int:
    """죽은 세션이 남긴 소켓 파일을 치운다.

    polycanv 가 곱게 끝나면 스스로 지우지만, 강제로 종료되면 남는다.
    쌓이면 사용자의 임시 디렉터리가 지저분해지고, 무엇보다 **연결을 거부하는 파일**이
    남아서 다음 사람을 헷갈리게 한다(실측: 죽은 소켓에 붙어 `Connection refused` 를 봤다).
    """
    removed = 0
    for path in directory.glob("polycanv-*.sock"):
        pid = path.stem.removeprefix("polycanv-")
        if not pid.isdigit():
            continue
        try:
            os.kill(int(pid), 0)  # 살아 있는지만 묻는다
        except ProcessLookupError:
            with contextlib.suppress(OSError):
                path.unlink()
                removed += 1
        except OSError:
            pass  # 남의 것이다. 건드리지 않는다
    return removed


class Bridge:
    """상태 줄을 받아 넘겨 주는 서버."""

    def __init__(self, on_event: Callable[[str, StatusEvent], None]) -> None:
        self.on_event = on_event
        self.path = socket_path()
        self._server: asyncio.AbstractServer | None = None

    async def start(self) -> None:
        sweep(self.path.parent)
        with contextlib.suppress(OSError):
            self.path.unlink()
        self._server = await asyncio.start_unix_server(self._serve, str(self.path))
        # 남이 남의 세션에 상태를 밀어 넣을 이유가 없다.
        with contextlib.suppress(OSError):
            self.path.chmod(0o600)

    async def _serve(self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
        try:
            async for raw in reader:
                self._handle(raw)
        finally:
            writer.close()

    def _handle(self, raw: bytes) -> None:
        # 훅이 보내는 것은 남의 프로그램이 만든 문자열이다. 무엇이 와도 죽지 않는다.
        try:
            data = json.loads(raw)
            pane = str(data["pane"])
            state = AgentState(data["state"])
            source = Source(data.get("source", "hook"))
            at_ms = int(data.get("at_ms") or now_ms())
        except (ValueError, KeyError, TypeError):
            return
        self.on_event(pane, StatusEvent(state, source, at_ms))

    async def stop(self) -> None:
        if self._server is not None:
            self._server.close()
            with contextlib.suppress(Exception):
                await self._server.wait_closed()
        with contextlib.suppress(OSError):
            self.path.unlink()
