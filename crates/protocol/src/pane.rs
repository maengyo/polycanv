//! 패인 식별자와 메타데이터.

use serde::{Deserialize, Serialize};

use crate::state::AgentState;

/// 패인 식별자.
///
/// zellij 의 `PaneId` 를 그대로 쓰지 않고 자체 타입을 둔다 — 이 크레이트를 호스트 타깃에서
/// 테스트할 수 있게 하려는 것이다. `zellij` 피처를 켜면 상호 변환이 딸려온다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaneKey {
    Terminal(u32),
    Plugin(u32),
}

impl PaneKey {
    /// 사용자의 터미널인가. 사이드바는 플러그인 패인(사이드바 자기 자신 등)을 목록에서 뺀다.
    pub fn is_terminal(self) -> bool {
        matches!(self, PaneKey::Terminal(_))
    }
}

/// 이 패인에서 돌고 있는 것.
///
/// 런처가 실행 시점에 채우고, 상태 감지가 어떤 판별 전략을 쓸지 고를 때 읽는다.
/// **목록은 `config/tools.kdl` 로 확장 가능해야 한다** — 아래 variant 는 내장 프리셋일 뿐,
/// 여기 없는 도구도 `Other` 로 1급 시민이어야 한다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    /// 훅으로 running/waiting/finished 완전 구분.
    ClaudeCode,
    /// 하이브리드 — finished 는 `notify`, waiting 은 출력 패턴.
    Codex,
    /// HTTP SSE 이벤트로 완전 구분.
    OpenCode,
    /// 문서상 claude 호환 훅 (미실측).
    QwenCode,
    /// 셸. AI 에이전트가 아니므로 running/idle 만 의미가 있다.
    Shell(String),
    /// 설정 파일로 추가된 사용자 정의 도구.
    Other(String),
}

impl ToolKind {
    /// 이 도구가 `Waiting`(승인 대기)이라는 개념을 갖는가.
    ///
    /// 셸은 갖지 않는다 — 셸 프롬프트가 떠 있는 건 유휴지 승인 대기가 아니다.
    /// 여기서 구분하지 않으면 사이드바가 노란불로 가득 차서 진짜 승인 요청이 묻힌다.
    pub fn has_approval_flow(&self) -> bool {
        !matches!(self, ToolKind::Shell(_))
    }

    /// 사이드바 아이템에 표시할 짧은 이름.
    pub fn label(&self) -> &str {
        match self {
            ToolKind::ClaudeCode => "claude",
            ToolKind::Codex => "codex",
            ToolKind::OpenCode => "opencode",
            ToolKind::QwenCode => "qwen",
            ToolKind::Shell(name) | ToolKind::Other(name) => name,
        }
    }
}

/// 사이드바 한 줄에 필요한 전부.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneMeta {
    pub key: PaneKey,
    /// 사용자가 붙인 이름. 없으면 패인 제목으로 대체한다.
    pub name: String,
    pub tool: ToolKind,
    /// 작업 디렉터리. 좁은 사이드바에서는 중간을 생략해 표시한다.
    pub cwd: String,
    pub state: AgentState,
    /// 현재 메인에 안 보이고 접혀 있는가. suppressed 여도 프로세스는 살아 있다.
    pub is_suppressed: bool,
}

#[cfg(feature = "zellij")]
mod zellij_interop {
    use super::PaneKey;
    use zellij_tile::prelude::PaneId;

    impl From<PaneId> for PaneKey {
        fn from(id: PaneId) -> Self {
            match id {
                PaneId::Terminal(n) => PaneKey::Terminal(n),
                PaneId::Plugin(n) => PaneKey::Plugin(n),
            }
        }
    }

    impl From<PaneKey> for PaneId {
        fn from(key: PaneKey) -> Self {
            match key {
                PaneKey::Terminal(n) => PaneId::Terminal(n),
                PaneKey::Plugin(n) => PaneId::Plugin(n),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 셸은_승인_대기_개념이_없다() {
        assert!(!ToolKind::Shell("pwsh".into()).has_approval_flow());
        assert!(ToolKind::ClaudeCode.has_approval_flow());
        assert!(ToolKind::Codex.has_approval_flow());
    }

    #[test]
    fn 플러그인_패인은_터미널이_아니다() {
        assert!(PaneKey::Terminal(3).is_terminal());
        assert!(!PaneKey::Plugin(3).is_terminal());
    }

    #[test]
    fn 같은_번호라도_터미널과_플러그인은_다른_패인이다() {
        assert_ne!(PaneKey::Terminal(1), PaneKey::Plugin(1));
    }
}
