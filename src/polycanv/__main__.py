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
    parser.add_argument("--port", type=int, default=8000, help="--web 일 때 쓸 포트 (기본 8000)")
    args = parser.parse_args()

    if args.web:
        from .web import serve

        return serve(args.port)

    from .app import PolycanvApp

    PolycanvApp().run()
    return 0


if __name__ == "__main__":
    sys.exit(main())
