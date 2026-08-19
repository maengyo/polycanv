"""진입점."""

from __future__ import annotations

import sys


def main() -> int:
    from .app import PolycanvApp

    PolycanvApp().run()
    return 0


if __name__ == "__main__":
    sys.exit(main())
