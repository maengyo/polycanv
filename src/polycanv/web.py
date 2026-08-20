"""브라우저로 열기.

WSL 이나 원격 기계처럼 터미널 에뮬레이터가 마땅치 않은 곳에서 쓴다.
브라우저 쪽은 xterm.js 가 그리므로 `Ctrl+-` 로 축소하면 행·열이 늘어 더 많이 보인다.

**여는 순간 그 주소는 셸이다.** polycanv 를 HTTP 로 띄운다는 것은 터미널 접근을 열어
준다는 뜻이고, 거기엔 인증이 없다. 그래서 **루프백에만 붙인다** — 이 파일에 host 를
바꾸는 길을 두지 않은 것은 잊어서가 아니다.
"""

from __future__ import annotations

import socket
import sys

#: 이 주소 밖으로는 내보내지 않는다. 인증이 생기기 전까지는 선택지가 아니다.
HOST = "127.0.0.1"

#: 아무것도 지정하지 않았을 때 먼저 써 볼 포트.
DEFAULT_PORT = 8000

#: 기본 포트가 막혔을 때 옆으로 몇 번까지 훑어볼지.
NEARBY = 20


def is_free(port: int) -> bool:
    with socket.socket() as probe:
        probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            probe.bind((HOST, port))
        except OSError:
            return False
    return True


def free_port() -> int:
    """비어 있는 포트 하나. 0 으로 묶으면 커널이 골라 준다."""
    with socket.socket() as probe:
        probe.bind((HOST, 0))
        return probe.getsockname()[1]


def choose_port(requested: int | None) -> int | None:
    """쓸 포트를 정한다. 정할 수 없으면 `None`.

    **지정했는지 아닌지로 태도가 갈린다.** 8000 은 흔한 포트라 이미 쓰이는 일이 잦은데,
    지정하지도 않은 기본값 때문에 실행이 실패하면 그건 우리 사정으로 사용자를 막는 것이다.
    반대로 포트를 콕 집어 말했다면 말없이 다른 데로 옮기는 쪽이 더 나쁘다 —
    그 주소로 접속하려던 것이기 때문이다.
    """
    if requested is not None:
        if is_free(requested):
            return requested
        print(
            f"{HOST}:{requested} 는 이미 다른 프로그램이 쓰고 있습니다.\n"
            f"  쓰는 것을 확인하려면:  lsof -nP -iTCP:{requested} -sTCP:LISTEN\n"
            f"  다른 포트로 열려면:    polycanv --web --port {free_port()}",
            file=sys.stderr,
        )
        return None

    # 바로 옆 번호부터 찾는다. 커널이 골라 주는 54850 같은 번호보다
    # 8001 이 외우기 쉽고, 다음에 다시 열 때도 같은 자리일 가능성이 높다.
    for port in range(DEFAULT_PORT, DEFAULT_PORT + NEARBY):
        if is_free(port):
            if port != DEFAULT_PORT:
                print(f"{HOST}:{DEFAULT_PORT} 이 사용 중이라 {port} 로 엽니다.")
            return port

    port = free_port()
    print(f"{DEFAULT_PORT}–{DEFAULT_PORT + NEARBY - 1} 이 모두 사용 중이라 {port} 로 엽니다.")
    return port


def serve(port: int | None = None) -> int:
    try:
        from textual_serve.server import Server
    except ImportError:
        print(
            "브라우저로 열려면 web 추가 구성요소가 필요합니다:\n"
            "  uv tool install --force 'polycanv[web]'\n"
            "  (개발 중이라면)  uv sync --extra web",
            file=sys.stderr,
        )
        return 1

    chosen = choose_port(port)
    if chosen is None:
        return 1

    # PATH 에 polycanv 가 없어도 되도록 지금 이 파이썬으로 되부른다.
    command = f"{sys.executable} -m polycanv"
    print(f"polycanv → http://{HOST}:{chosen}  (이 기계에서만 열립니다. Ctrl+C 로 종료)")
    try:
        Server(command, host=HOST, port=chosen, title="polycanv").serve()
    except OSError as exc:
        # 확인과 실제 묶기 사이에 남이 채 갈 수 있다. 그때도 트레이스백을 보이지 않는다.
        print(f"{HOST}:{chosen} 을 열지 못했습니다: {exc}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        pass
    return 0
