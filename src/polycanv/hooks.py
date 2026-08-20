"""CLI 에 훅을 얹는다.

**사용자의 설정 파일을 고치지 않는다.** 되돌리기 어렵고, 다른 도구와 부딪히며,
polycanv 를 지워도 흔적이 남는다. 대신 얹는 수단이 CLI 마다 있다:

    claude / qwen   `--settings <파일>` 로 설정을 **덧씌운다**. 인증은 그대로 쓴다
    codex           `CODEX_HOME` 을 따로 준다

여기서 만드는 파일은 임시 디렉터리에 있고 polycanv 가 끝나면 사라진다.

훅 페이로드에서 상태로 가는 대응은 실측으로 확인된 것이다
(`docs/research/cli-status-hooks.md`).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

from .status import AgentState

#: claude·qwen 의 훅 이름 → 신호등.
#:
#: `Notification` 이 왜 waiting 인가: claude 는 사용자 입력이 필요할 때 이걸 쏜다
#: (권한 승인 등). `Stop` 은 턴이 끝났을 때다.
CLAUDE_EVENTS = {
    "UserPromptSubmit": AgentState.RUNNING,
    "Notification": AgentState.WAITING,
    "PermissionRequest": AgentState.WAITING,
    "Stop": AgentState.FINISHED,
    "SessionStart": AgentState.IDLE,
}


def hook_command() -> str:
    """훅이 실행할 명령.

    `polycanv` 라는 이름 대신 지금 이 파이썬을 쓴다 — 훅은 CLI 가 만든 환경에서 돌고,
    거기에 우리 `PATH` 가 있으리라는 보장이 없다.
    """
    return f"{sys.executable} -m polycanv --hook"


def claude_settings() -> dict:
    """`--settings` 로 넘길 설정. 훅만 얹고 나머지는 건드리지 않는다."""
    command = hook_command()
    return {
        "hooks": {
            event: [{"hooks": [{"type": "command", "command": command, "timeout": 5}]}]
            for event in CLAUDE_EVENTS
        }
    }


def write_claude_settings(directory: Path, pane: str) -> Path:
    path = directory / f"claude-{pane}.json"
    path.write_text(json.dumps(claude_settings(), indent=2), encoding="utf-8")
    return path


def state_from_payload(payload: dict) -> AgentState | None:
    """훅이 stdin 으로 준 JSON 에서 상태를 읽는다.

    이름은 `hook_event_name` 으로 온다(실측). 모르는 이름이면 아무것도 하지 않는다 —
    **추측해서 신호등을 켜지 않는다.**
    """
    name = payload.get("hook_event_name") or payload.get("hookEventName")
    if not isinstance(name, str):
        return None
    return CLAUDE_EVENTS.get(name)
