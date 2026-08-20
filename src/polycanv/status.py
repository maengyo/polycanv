"""신호등 — 상태와 그 병합 규칙.

상태는 여러 경로로 들어온다. claude·qwen 은 훅, opencode 는 HTTP SSE, codex 는 훅.
**쓰는 쪽(패널)이 출처를 신경 쓰지 않도록** 여기서 하나로 모으고, 출처가 다른 이벤트가
부딪힐 때 누가 이기는지도 여기서 정한다.

규칙은 전부 **"놓치는 것이 헛보는 것보다 나쁘다"** 에서 나온다. 끝났는데 신호가 안 오면
이 도구는 존재 이유를 잃고, 잘못 켜진 🔴 은 한 번 보면 그만이다.

zellij 판에서 검증된 계약을 그대로 옮긴 것이다 (`legacy-zellij` 브랜치의
`crates/protocol/src/{state,event}.rs`).
"""

from __future__ import annotations

import time
from dataclasses import dataclass
from enum import Enum


class AgentState(str, Enum):
    """터미널 하나의 신호등.

    **`waiting` 과 `finished` 를 뭉개지 않는 것이 이 타입의 존재 이유다.**
    둘 다 멈춰 보이지만 사용자가 할 일이 다르다 — 하나는 답을 해줘야 하고,
    하나는 결과를 보면 된다.
    """

    IDLE = "idle"
    RUNNING = "running"
    WAITING = "waiting"
    FINISHED = "finished"

    def needs_attention(self) -> bool:
        """사용자를 불러야 하는 상태인가."""
        return self in (AgentState.WAITING, AgentState.FINISHED)

    def on_focus(self) -> AgentState:
        """그 터미널을 실제로 들여다봤을 때의 다음 상태.

        **`finished` 만 확인으로 풀린다.** 승인 프롬프트는 쳐다본다고 사라지지 않는다 —
        답을 해야 CLI 가 다음 이벤트를 보낸다. 여기서 `waiting` 을 같이 지우면
        사용자가 프롬프트를 놓친다.
        """
        return AgentState.IDLE if self is AgentState.FINISHED else self


class Source(str, Enum):
    """이 이벤트가 어디서 왔는가. 이름 자체가 신뢰도다."""

    HOOK = "hook"  # CLI 훅. 가장 믿을 만하다
    SSE = "sse"  # opencode 이벤트 스트림. 훅과 동급
    NOTIFY = "notify"  # 알림. 믿을 만하지만 끝났다는 것만 온다
    PATTERN = "pattern"  # 출력 정규식. 추론이다
    BELL = "bell"  # 벨 문자. 났다는 것만 알고 무엇인지는 모른다
    IDLE = "idle"  # 한동안 출력이 없음. 가장 약하다

    def rank(self) -> int:
        if self in (Source.HOOK, Source.SSE):
            return 3
        if self is Source.NOTIFY:
            return 2
        if self is Source.PATTERN:
            return 1
        return 0


def now_ms() -> int:
    return int(time.time() * 1000)


@dataclass(frozen=True)
class StatusEvent:
    """터미널 하나의 상태가 바뀌었다는 통지."""

    state: AgentState
    source: Source
    at_ms: int


@dataclass
class Status:
    """마지막으로 채택된 상태."""

    state: AgentState = AgentState.IDLE
    source: Source = Source.IDLE
    at_ms: int = 0

    def apply(self, event: StatusEvent) -> bool:
        """새 이벤트를 반영한다. 채택했으면 참.

        1. **지나간 이벤트는 버린다.** 같은 시각이면 새 것을 받는다(같은 순간의 순서 보존).
        2. **등급이 같거나 높으면 받는다.**
        3. **등급이 낮아도 주의 상태로 *올릴* 때는 받는다.** 약한 근거가 훅이 세운 🔴 을
           지우면 사용자가 완료를 놓친다. 반대로 훅이 놓친 승인 프롬프트를 출력 패턴이
           잡아 🟡 을 켜는 것은 이득이다.
        """
        if event.at_ms < self.at_ms:
            return False
        accept = event.source.rank() >= self.source.rank() or (
            event.state.needs_attention() and not self.state.needs_attention()
        )
        if accept:
            self.state, self.source, self.at_ms = event.state, event.source, event.at_ms
        return accept

    def acknowledge(self, at_ms: int | None = None) -> bool:
        """사용자가 이 터미널을 실제로 들여다봤다.

        확인은 어떤 이벤트보다 세다 — 눈으로 봤다는 것은 추론이 아니다. 그래서 출처를
        가장 높은 등급으로 올려, **뒤늦게 도착한 약한 이벤트가 방금 확인한 🔴 을
        되살리지 못하게** 한다.
        """
        nxt = self.state.on_focus()
        if nxt is self.state:
            return False
        self.state = nxt
        self.source = Source.HOOK
        self.at_ms = at_ms if at_ms is not None else now_ms()
        return True
