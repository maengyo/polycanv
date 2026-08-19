//! polycanv 리스트 사이드바.
//!
//! 하는 일은 셋이다.
//! 1. 지금 살아 있는 터미널을 한 줄씩 그린다 (이름 / CLI 종류 / cwd / 신호등).
//! 2. 고른 항목을 메인 슬롯으로 올린다 — `replace_pane_with_existing_pane(.., .., true)` 한 번.
//! 3. 캔버스↔리스트 뷰를 전환한다 — **패인 집합을 먼저 맞추고 swap layout 을 몬다**(2단계).
//!
//! 이 크레이트에는 zellij 의존이 없다. wasm 글루는 `src/main.rs` + `src/plugin.rs` 에만 있고,
//! 그쪽은 `cfg(target_arch = "wasm32")` 뒤에 격리돼 있어 호스트에서 `cargo test` 가 돈다.
//!
//! **상태 감지는 이 플러그인의 일이 아니다.** `plugins/status/` 가 `polycanv:state` 파이프로
//! [`polycanv_protocol::StatusEvent`] 를 보내주면 그걸 받아 그릴 뿐이고,
//! 아무도 안 보내면 전부 ⚪ 로 남는다. 없는 상태를 지어내지 않는다.

pub mod model;
pub mod render;
pub mod toggle;

pub use model::{
    build_rows, fold_targets, pick_main, refresh_targets, restore_targets, tool_from_command,
    PaneFacts, PaneSnapshot, Row,
};
pub use render::{row_index_at_line, screen, visible_window, HEADER_ROWS};
pub use toggle::{drive, reached, Drive, LIST_LAYOUT, MAX_SWAP_STEPS};

/// 선택 인덱스를 목록 범위 안으로 되돌린다. 목록이 비면 0.
pub fn clamp_selection(selected: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        selected.min(len - 1)
    }
}

/// 위/아래 이동. 끝에서 감싼다 — 6개 넘는 목록에서 끝까지 내려간 뒤 한 번 더 누르는 건
/// "처음으로 가고 싶다"는 뜻이지 "아무 일도 일어나지 마라"가 아니다.
pub fn step_selection(selected: usize, len: usize, down: bool) -> usize {
    if len == 0 {
        return 0;
    }
    if down {
        (selected + 1) % len
    } else {
        (selected + len - 1) % len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 목록이_비면_선택은_0이다() {
        assert_eq!(clamp_selection(7, 0), 0);
        assert_eq!(step_selection(3, 0, true), 0);
    }

    #[test]
    fn 패인이_줄어들면_선택이_따라_줄어든다() {
        // 패인이 닫혀 목록이 짧아졌는데 선택 인덱스가 그대로면 엉뚱한 패인을 교체한다.
        assert_eq!(clamp_selection(5, 3), 2);
        assert_eq!(clamp_selection(1, 3), 1);
    }

    #[test]
    fn 선택은_끝에서_감싼다() {
        assert_eq!(step_selection(2, 3, true), 0);
        assert_eq!(step_selection(0, 3, false), 2);
    }
}
