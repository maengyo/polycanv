"""신호등의 병합 규칙.

zellij 판에서 검증됐던 것을 그대로 옮겼다. **여기가 무너지면 신호등이 조용히 거짓말한다** —
오류가 아니라 잘못된 색으로 나타나므로 눈으로는 못 잡는다.
"""

from __future__ import annotations

from polycanv.status import AgentState, Source, Status, StatusEvent


def ev(state: AgentState, source: Source, at_ms: int) -> StatusEvent:
    return StatusEvent(state, source, at_ms)


def test_출력_패턴은_훅이_세운_완료를_지우지_못한다() -> None:
    """약한 근거가 🔴 을 지우면 사용자가 완료를 놓친다."""
    s = Status()
    s.apply(ev(AgentState.FINISHED, Source.HOOK, 100))

    assert not s.apply(ev(AgentState.RUNNING, Source.PATTERN, 200))
    assert s.state is AgentState.FINISHED


def test_약한_근거도_주의_상태로는_올릴_수_있다() -> None:
    """훅이 놓친 승인 프롬프트를 출력 패턴이 잡는 경로다."""
    s = Status()
    s.apply(ev(AgentState.RUNNING, Source.HOOK, 100))

    assert s.apply(ev(AgentState.WAITING, Source.PATTERN, 200))
    assert s.state is AgentState.WAITING


def test_훅은_언제나_이긴다() -> None:
    s = Status()
    s.apply(ev(AgentState.WAITING, Source.PATTERN, 100))

    assert s.apply(ev(AgentState.RUNNING, Source.HOOK, 150))
    assert s.state is AgentState.RUNNING


def test_뒤늦게_도착한_과거_이벤트는_버린다() -> None:
    s = Status()
    s.apply(ev(AgentState.RUNNING, Source.HOOK, 500))

    assert not s.apply(ev(AgentState.IDLE, Source.HOOK, 400))
    assert s.state is AgentState.RUNNING


def test_같은_시각이면_새_것을_받는다() -> None:
    """한 순간에 여러 개가 와도 온 순서를 지킨다."""
    s = Status()
    s.apply(ev(AgentState.RUNNING, Source.HOOK, 500))

    assert s.apply(ev(AgentState.FINISHED, Source.HOOK, 500))
    assert s.state is AgentState.FINISHED


def test_확인하면_완료가_풀리고_되살아나지_않는다() -> None:
    s = Status()
    s.apply(ev(AgentState.FINISHED, Source.HOOK, 100))

    assert s.acknowledge(200)
    assert s.state is AgentState.IDLE
    # 뒤늦게 도착한 약한 이벤트가 방금 확인한 것을 되살리면 안 된다
    assert not s.apply(ev(AgentState.FINISHED, Source.PATTERN, 150))
    assert s.state is AgentState.IDLE


def test_확인해도_승인_대기는_남는다() -> None:
    """승인 프롬프트는 쳐다본다고 사라지지 않는다."""
    s = Status()
    s.apply(ev(AgentState.WAITING, Source.HOOK, 100))

    assert not s.acknowledge(200)
    assert s.state is AgentState.WAITING


def test_주의가_필요한_것은_둘뿐이다() -> None:
    assert AgentState.WAITING.needs_attention()
    assert AgentState.FINISHED.needs_attention()
    assert not AgentState.RUNNING.needs_attention()
    assert not AgentState.IDLE.needs_attention()


def test_출처의_서열() -> None:
    assert Source.HOOK.rank() == Source.SSE.rank() > Source.NOTIFY.rank()
    assert Source.NOTIFY.rank() > Source.PATTERN.rank() > Source.BELL.rank()
    assert Source.BELL.rank() == Source.IDLE.rank()
