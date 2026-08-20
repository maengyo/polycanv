"""`polycanv --hook` — CLI 훅이 부르는 쪽.

**무슨 일이 있어도 0 으로 끝난다.** 훅이 실패하면 CLI 가 그것을 오류로 받아들여
사용자의 턴을 망친다. 신호등 하나 때문에 작업을 세우지 않는다.
"""

from __future__ import annotations

import json
import os
import sys

from .bridge import PANE_ENV, send
from .hooks import state_from_payload


def run() -> int:
    try:
        raw = sys.stdin.read()
        payload = json.loads(raw) if raw.strip() else {}
        state = state_from_payload(payload)
        pane = os.environ.get(PANE_ENV)
        if state is not None and pane:
            send(state.value, pane)
    except Exception:  # noqa: BLE001 - 훅은 절대 실패해서는 안 된다
        pass
    return 0
