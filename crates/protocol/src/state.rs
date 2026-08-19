//! 신호등 상태와 그 전이 규칙.

use serde::{Deserialize, Serialize};

/// 터미널 하나의 신호등 상태.
///
/// `Waiting` 과 `Finished` 를 뭉개지 않는 것이 이 타입의 존재 이유다.
/// 둘 다 "멈춰 보이지만" 사용자가 할 일이 다르다 — 전자는 답을 해줘야 하고, 후자는 결과를 보면 된다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    /// ⚪ 아무것도 실행 중이지 않다.
    #[default]
    Idle,
    /// 🟢 에이전트가 작업 중이다.
    Running,
    /// 🟡 권한 승인·질문 등 **사용자 입력을 기다린다**.
    Waiting,
    /// 🔴 응답을 마쳤거나 알람이 났다. 깜빡이며, 사용자가 확인하면 해제된다.
    Finished,
}

impl AgentState {
    /// 사용자의 주의를 요구하는 상태인가. 캔버스 테두리·사이드바 아이템이 깜빡일 조건.
    pub fn needs_attention(self) -> bool {
        matches!(self, AgentState::Waiting | AgentState::Finished)
    }

    /// 신호등 문자.
    pub fn glyph(self) -> char {
        match self {
            AgentState::Idle => '⚪',
            AgentState::Running => '🟢',
            AgentState::Waiting => '🟡',
            AgentState::Finished => '🔴',
        }
    }

    /// 사용자가 그 패인을 실제로 포커스했을 때의 다음 상태.
    ///
    /// `Finished` 만 확인으로 해제된다. `Waiting` 은 **보는 것만으로 해제되지 않는다** —
    /// 승인 프롬프트는 쳐다본다고 사라지지 않고, 답을 해야 CLI가 다음 이벤트를 보낸다.
    /// 여기서 `Waiting` 을 같이 지우면 사용자가 프롬프트를 놓친다.
    pub fn on_focus(self) -> Self {
        match self {
            AgentState::Finished => AgentState::Idle,
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 포커스는_finished만_해제한다() {
        assert_eq!(AgentState::Finished.on_focus(), AgentState::Idle);
        assert_eq!(AgentState::Waiting.on_focus(), AgentState::Waiting);
        assert_eq!(AgentState::Running.on_focus(), AgentState::Running);
        assert_eq!(AgentState::Idle.on_focus(), AgentState::Idle);
    }

    #[test]
    fn 주의가_필요한_상태는_waiting과_finished뿐이다() {
        assert!(AgentState::Waiting.needs_attention());
        assert!(AgentState::Finished.needs_attention());
        assert!(!AgentState::Running.needs_attention());
        assert!(!AgentState::Idle.needs_attention());
    }

    #[test]
    fn 기본값은_idle이다() {
        assert_eq!(AgentState::default(), AgentState::Idle);
    }
}
