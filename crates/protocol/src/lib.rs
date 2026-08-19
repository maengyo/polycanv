//! polycanv 공유 계약.
//!
//! 런처·상태감지·사이드바 플러그인이 주고받는 타입을 한곳에 모은다.
//! **이 크레이트는 리드만 수정한다.** 변경이 필요하면 "protocol 변경 요청"으로 보고할 것.
//!
//! 의존성을 일부러 얇게 유지한다 — zellij-tile 은 `zellij` 피처로만 끌어오므로
//! 호스트 타깃에서 `cargo test` 가 돈다. 상태 규칙은 wasm 없이 검증 가능해야 한다.

use serde::{Deserialize, Serialize};

pub mod event;
pub mod group;
pub mod pane;
pub mod state;

pub use event::{StatusEvent, StatusRecord, StatusSource};
pub use group::{color_for, GroupColor, GroupKey};
pub use pane::{PaneKey, PaneMeta, ToolKind};
pub use state::AgentState;

/// 사이드바가 관리하는 뷰 상태. 리스트 뷰에서 "지금 메인 슬롯에 있는 패인"이 핵심이다.
///
/// `replace_pane_with_existing_pane` 이 지오메트리를 자동 승계하므로 좌표를 들고 있을 필요가 없다.
/// (근거: `docs/research/zellij-pane-control-api.md`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ViewState {
    pub mode: ViewMode,
    /// 리스트 뷰에서 메인 슬롯을 점유 중인 패인. 캔버스 뷰에서는 "마지막 포커스"로 쓴다.
    ///
    /// 플러그인이 재시작돼도 `PaneUpdate` 에서 `is_suppressed == false && !is_plugin` 인
    /// 패인으로 복원할 수 있으므로, 이 값을 영속화할 필요는 없다.
    pub main_slot: Option<PaneKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode {
    /// 여러 터미널이 동시에 펼쳐진 기본 모드.
    #[default]
    Canvas,
    /// 좌측 사이드바 + 우측 메인 1개인 집중 모드.
    List,
}

impl ViewMode {
    pub fn toggled(self) -> Self {
        match self {
            ViewMode::Canvas => ViewMode::List,
            ViewMode::List => ViewMode::Canvas,
        }
    }
}
