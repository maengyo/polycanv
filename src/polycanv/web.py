"""브라우저로 열기.

WSL 이나 원격 기계처럼 터미널 에뮬레이터가 마땅치 않은 곳에서 쓴다.
브라우저 쪽은 xterm.js 가 그리므로 `Ctrl+-` 로 축소하면 행·열이 늘어 더 많이 보인다.

**여는 순간 그 주소는 셸이다.** polycanv 를 HTTP 로 띄운다는 것은 터미널 접근을 열어
준다는 뜻이고, 거기엔 인증이 없다. 그래서 **루프백에만 붙인다** — 이 파일에 host 를
바꾸는 길을 두지 않은 것은 잊어서가 아니다.
"""

from __future__ import annotations

import sys

#: 이 주소 밖으로는 내보내지 않는다. 인증이 생기기 전까지는 선택지가 아니다.
HOST = "127.0.0.1"


def serve(port: int = 8000) -> int:
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

    # PATH 에 polycanv 가 없어도 되도록 지금 이 파이썬으로 되부른다.
    command = f"{sys.executable} -m polycanv"
    print(f"polycanv → http://{HOST}:{port}  (이 기계에서만 열립니다. Ctrl+C 로 종료)")
    Server(command, host=HOST, port=port, title="polycanv").serve()
    return 0
