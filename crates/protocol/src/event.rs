//! 상태 이벤트와 병합 규칙.
//!
//! 상태는 여러 경로로 들어온다. claude 는 훅, opencode 는 SSE, codex 는 finished 만 notify 로 오고
//! waiting 은 출력 패턴으로 추론한다. **소비하는 쪽(사이드바)이 출처를 신경 쓰지 않도록**
//! 여기서 하나의 타입으로 통일하고, 출처가 다른 이벤트가 충돌할 때의 우선순위도 여기서 정한다.

use serde::{Deserialize, Serialize};

use crate::pane::PaneKey;
use crate::state::AgentState;

/// 이 이벤트가 어디서 왔는가. 신뢰도 순서를 담고 있다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusSource {
    /// CLI 훅 (claude/qwen `Stop`·`Notification` 등). 가장 신뢰할 수 있다.
    Hook,
    /// opencode HTTP SSE 이벤트. 훅과 동급.
    Sse,
    /// codex `config.toml` 의 `notify`. 신뢰할 수 있으나 finished 만 온다.
    Notify,
    /// 터미널 출력 정규식 매칭. 추론이다.
    Pattern,
    /// 벨 문자(`\a`). 무슨 일이 났는지는 모르고 났다는 것만 안다.
    Bell,
    /// 일정 시간 출력이 없음. 가장 약한 근거.
    IdleHeuristic,
}

impl StatusSource {
    /// 신뢰도 등급. 클수록 세다.
    pub fn rank(self) -> u8 {
        match self {
            StatusSource::Hook | StatusSource::Sse => 3,
            StatusSource::Notify => 2,
            StatusSource::Pattern => 1,
            StatusSource::Bell | StatusSource::IdleHeuristic => 0,
        }
    }
}

/// 패인 하나의 상태가 바뀌었다는 통지.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusEvent {
    pub pane: PaneKey,
    pub state: AgentState,
    pub source: StatusSource,
    /// 유닉스 epoch 밀리초. 순서가 뒤집힌 이벤트를 버리는 데 쓴다.
    pub at_ms: u64,
}

/// 패인 하나에 대해 마지막으로 채택된 이벤트.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusRecord {
    pub state: AgentState,
    pub source: StatusSource,
    pub at_ms: u64,
}

impl StatusRecord {
    pub fn new(state: AgentState, source: StatusSource, at_ms: u64) -> Self {
        Self {
            state,
            source,
            at_ms,
        }
    }

    /// 새 이벤트를 반영한다. 채택했으면 `true`.
    ///
    /// 규칙은 "미탐이 오탐보다 나쁘다"에서 나온다 — 끝났는데 신호가 안 오면 제품이 실패하고,
    /// 잘못된 🔴은 사용자가 한 번 보면 그만이다. 그래서:
    ///
    /// 1. **과거 이벤트는 버린다.** 같은 시각이면 새 것을 채택한다(같은 틱 내 순서 보존).
    /// 2. **등급이 같거나 높으면 채택한다.**
    /// 3. **등급이 낮으면 주의 상태로 올릴 때만 채택한다.** 패턴 매칭이 훅이 세운 🔴을
    ///    지워버리면 사용자가 완료를 놓친다. 반대로 훅이 놓친 승인 프롬프트를 패턴이
    ///    잡아서 🟡을 켜는 것은 허용한다 — codex 의 waiting 이 정확히 이 경로다.
    pub fn apply(&mut self, ev: &StatusEvent) -> bool {
        if ev.at_ms < self.at_ms {
            return false;
        }
        let accept = ev.source.rank() >= self.source.rank()
            || (ev.state.needs_attention() && !self.state.needs_attention());
        if accept {
            self.state = ev.state;
            self.source = ev.source;
            self.at_ms = ev.at_ms;
        }
        accept
    }

    /// 사용자가 이 패인을 실제로 포커스했다. 🔴을 해제한다.
    ///
    /// 확인은 어떤 이벤트보다 세다 — 사용자가 눈으로 봤다는 사실은 추론이 아니다.
    /// 그래서 source 를 `Hook` 등급으로 승격시켜, 뒤늦게 도착한 약한 이벤트가
    /// 방금 확인한 🔴을 되살리지 못하게 한다.
    pub fn acknowledge(&mut self, at_ms: u64) {
        let next = self.state.on_focus();
        if next != self.state {
            self.state = next;
            self.source = StatusSource::Hook;
            self.at_ms = at_ms;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(state: AgentState, source: StatusSource, at_ms: u64) -> StatusEvent {
        StatusEvent {
            pane: PaneKey::Terminal(1),
            state,
            source,
            at_ms,
        }
    }

    #[test]
    fn 패턴매칭은_훅이_세운_완료를_지우지_못한다() {
        let mut rec = StatusRecord::new(AgentState::Finished, StatusSource::Hook, 100);
        let accepted = rec.apply(&ev(AgentState::Idle, StatusSource::Pattern, 200));
        assert!(!accepted, "약한 출처가 🔴을 지우면 사용자가 완료를 놓친다");
        assert_eq!(rec.state, AgentState::Finished);
    }

    #[test]
    fn 패턴매칭도_주의_상태로는_올릴_수_있다() {
        // codex 의 waiting 이 정확히 이 경로다 — 훅이 없어 출력 패턴으로만 잡힌다.
        let mut rec = StatusRecord::new(AgentState::Running, StatusSource::Notify, 100);
        assert!(rec.apply(&ev(AgentState::Waiting, StatusSource::Pattern, 200)));
        assert_eq!(rec.state, AgentState::Waiting);
    }

    #[test]
    fn 훅은_언제나_이긴다() {
        let mut rec = StatusRecord::new(AgentState::Waiting, StatusSource::Pattern, 100);
        assert!(rec.apply(&ev(AgentState::Idle, StatusSource::Hook, 200)));
        assert_eq!(rec.state, AgentState::Idle);
    }

    #[test]
    fn 뒤늦게_도착한_과거_이벤트는_버린다() {
        let mut rec = StatusRecord::new(AgentState::Finished, StatusSource::Hook, 500);
        assert!(!rec.apply(&ev(AgentState::Running, StatusSource::Hook, 400)));
        assert_eq!(rec.state, AgentState::Finished);
    }

    #[test]
    fn 확인하면_완료가_해제되고_되살아나지_않는다() {
        let mut rec = StatusRecord::new(AgentState::Finished, StatusSource::Hook, 100);
        rec.acknowledge(200);
        assert_eq!(rec.state, AgentState::Idle);
        // 확인 직전에 발생해 늦게 도착한 약한 이벤트가 🔴을 되살리면 안 된다.
        assert!(!rec.apply(&ev(AgentState::Finished, StatusSource::Bell, 150)));
        assert_eq!(rec.state, AgentState::Idle);
    }

    /// `scripts/polycanv-hook.sh` 가 실제로 파이프로 보내는 바이트다.
    ///
    /// 셸 스크립트와 이 타입은 **서로를 모른 채 같은 형식에 합의**해야 한다. 어긋나면
    /// 신호등이 조용히 안 켜질 뿐 아무 오류도 안 난다 — 그래서 여기에 못 박는다.
    /// 스크립트 출력을 바꾸면 이 테스트부터 깨져야 한다.
    #[test]
    fn 훅_브리지가_보내는_와이어_포맷을_그대로_읽는다() {
        let wire = [
            (
                r#"{"pane":{"terminal":7},"state":"running","source":"hook","at_ms":1787101570000}"#,
                AgentState::Running,
            ),
            (
                r#"{"pane":{"terminal":7},"state":"waiting","source":"hook","at_ms":1787101570000}"#,
                AgentState::Waiting,
            ),
            (
                r#"{"pane":{"terminal":7},"state":"finished","source":"hook","at_ms":1787101570000}"#,
                AgentState::Finished,
            ),
            (
                r#"{"pane":{"terminal":7},"state":"idle","source":"hook","at_ms":1787101570000}"#,
                AgentState::Idle,
            ),
        ];
        for (line, expected) in wire {
            let ev: StatusEvent = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("훅 브리지 페이로드를 못 읽는다: {line}\n{e}"));
            assert_eq!(ev.state, expected);
            assert_eq!(ev.pane, PaneKey::Terminal(7));
            assert_eq!(ev.source, StatusSource::Hook);
        }
    }

    #[test]
    fn 확인해도_승인_대기는_남는다() {
        let mut rec = StatusRecord::new(AgentState::Waiting, StatusSource::Hook, 100);
        rec.acknowledge(200);
        assert_eq!(
            rec.state,
            AgentState::Waiting,
            "프롬프트는 쳐다본다고 사라지지 않는다"
        );
    }
}
