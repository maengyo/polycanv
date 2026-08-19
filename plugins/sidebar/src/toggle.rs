//! 뷰 전환 2단계 중 **2단계 — swap layout 몰기**.
//!
//! `next_swap_layout()` 에는 인덱스 지정이 없다. 대신 `TabInfo.active_swap_layout_name` 을 읽어
//! **목표에 닿을 때까지 반복 호출**하면 결정론이 된다. 이 모듈은 "한 번 더 부를까 / 도착했나 /
//! 포기할까" 만 판단한다. 실제 호출은 wasm 글루가 한다.
//!
//! ★ 순환의 길이를 가정하지 않는다. zellij 는 **기저 레이아웃을 `ExactPanes(선언된 패인 수)`
//!   제약으로 BASE 슬롯에 넣는다** (`zellij-server/src/tab/swap_layouts.rs:47`,
//!   주석 그대로 "the base layout is not intended to be progressive"). 즉 기저 레이아웃은
//!   **선언한 패인 수와 정확히 일치할 때만** 후보가 되고, 그 외에는 순환에서 통째로 빠진다.
//!   → 터미널이 늘면 주기가 2에서 3으로, 또 2로 바뀐다. 반복+상한이 그래서 필요하다.
//!
//! 상한을 넘기면 조용히 포기하고 로그를 남긴다 — 무한 루프보다는 낫다.

use polycanv_protocol::ViewMode;

/// `layouts/polycanv.kdl` 의 `swap_tiled_layout name="list"`.
pub const LIST_LAYOUT: &str = "list";

/// 이 횟수 안에 목표 레이아웃에 닿지 못하면 포기한다.
/// 주기 2 면 1회로 끝나고, 주기가 늘어나도 몇 번이면 한 바퀴를 돈다.
pub const MAX_SWAP_STEPS: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drive {
    /// 목표 레이아웃이다. 멈춘다.
    Arrived,
    /// `next_swap_layout()` 을 한 번 더 부른다.
    Step,
    /// 상한을 넘겼다. 무한 루프 대신 포기하고 로그를 남긴다.
    GaveUp,
}

/// 지금 활성 레이아웃이 목표인가.
///
/// 캔버스는 **이름이 아니라 "리스트가 아님"** 으로 판정한다. 캔버스 배치는 상황에 따라
/// `"BASE"` 로도 `"canvas"`(별도 swap layout) 로도 나타나기 때문이다 — 위 `ExactPanes` 때문에
/// 패인 수에 따라 둘 중 무엇이 잡힐지 달라진다. 이름을 하나로 못 박으면 그 순간 토글이 멈춘다.
pub fn reached(target: ViewMode, active: Option<&str>) -> bool {
    let is_list = active == Some(LIST_LAYOUT);
    match target {
        ViewMode::List => is_list,
        ViewMode::Canvas => !is_list,
    }
}

pub fn drive(target: ViewMode, active: Option<&str>, steps: u8) -> Drive {
    if reached(target, active) {
        Drive::Arrived
    } else if steps < MAX_SWAP_STEPS {
        Drive::Step
    } else {
        Drive::GaveUp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 기저_레이아웃은_이름이_뭐든_캔버스다() {
        assert!(reached(ViewMode::Canvas, None));
        assert!(reached(ViewMode::Canvas, Some("BASE")));
        assert!(reached(ViewMode::Canvas, Some("무엇이든")));
        assert!(!reached(ViewMode::Canvas, Some(LIST_LAYOUT)));
    }

    #[test]
    fn 리스트는_이름이_정확히_맞아야_한다() {
        assert!(reached(ViewMode::List, Some("list")));
        assert!(!reached(ViewMode::List, None));
        assert!(!reached(ViewMode::List, Some("List")));
    }

    #[test]
    fn 도착했으면_더_부르지_않는다() {
        assert_eq!(drive(ViewMode::List, Some("list"), 0), Drive::Arrived);
        assert_eq!(drive(ViewMode::Canvas, None, 3), Drive::Arrived);
    }

    #[test]
    fn 아직이면_한_번_더_부른다() {
        assert_eq!(drive(ViewMode::List, None, 0), Drive::Step);
        assert_eq!(drive(ViewMode::List, None, MAX_SWAP_STEPS - 1), Drive::Step);
    }

    #[test]
    fn 상한을_넘으면_무한루프_대신_포기한다() {
        assert_eq!(drive(ViewMode::List, None, MAX_SWAP_STEPS), Drive::GaveUp);
        assert_eq!(drive(ViewMode::List, Some("다른것"), 99), Drive::GaveUp);
    }
}
