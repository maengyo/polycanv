"""진입점."""

from __future__ import annotations

import argparse
import sys


def main() -> int:
    parser = argparse.ArgumentParser(prog="polycanv", description="흩어진 세션을 한 캔버스 위에")
    parser.add_argument(
        "--web",
        action="store_true",
        help="터미널 대신 브라우저로 연다 (127.0.0.1 에만 붙는다)",
    )
    # 기본값을 두지 않는다 — **지정했는지 아닌지**로 동작이 갈리기 때문이다.
    parser.add_argument(
        "--port",
        type=int,
        help="--web 일 때 쓸 포트 (기본 8000, 사용 중이면 빈 포트를 찾는다)",
    )
    args = parser.parse_args()

    if args.web:
        from .web import serve

        return serve(args.port)

    from .app import PolycanvApp

    PolycanvApp().run()
    return 0


if __name__ == "__main__":
    sys.exit(main())
