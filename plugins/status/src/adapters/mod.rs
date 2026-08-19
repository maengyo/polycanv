//! CLI별 감지 로직.
//!
//! **새 CLI 지원은 여기에 모듈 하나를 추가하고 [`for_tool`] 에 한 줄 붙이는 것으로 끝나야 한다.**
//! 어댑터 밖으로 `match tool_name` 이 새어 나가면 설계가 틀린 것이다.
//!
//! 어댑터는 **원문 이벤트 한 줄**을 받는다. 브리지(셸 스크립트)는 파싱하지 않고 그대로 흘려보낸다 —
//! 매핑을 셸/jq 에 두면 단위 테스트가 불가능해지고 CLI 마다 스크립트가 불어난다.

use polycanv_protocol::{AgentState, StatusSource};

pub mod opencode;

/// 원문 이벤트 한 줄에서 읽어낸 상태 변화.
pub trait StatusAdapter {
    /// 이 이벤트가 어떤 상태를 뜻하는가. **관심 없는 이벤트면 `None`** — 상태를 유지한다.
    ///
    /// `&mut self` 인 이유: 상태 판별에 직전 맥락이 필요하다. opencode 의 `session.idle` 은
    /// "일하다 끝났다(🔴)"일 수도 "원래 놀고 있었다(⚪)"일 수도 있고, 그 차이는 이전 이벤트에만 있다.
    fn observe(&mut self, raw: &str) -> Option<AgentState>;

    /// 이 어댑터가 내놓는 근거의 등급. 병합 우선순위에 쓰인다.
    fn source(&self) -> StatusSource;
}

/// 파이프 args 의 `tool` 값 → 어댑터.
///
/// 모르는 도구면 `None`. 어댑터가 없다는 건 아직 ①계층 수단이 없다는 뜻이고,
/// 그 경우는 ②출력패턴 이하로 내려가야 한다 — 여기서 억지로 만들어내지 않는다.
pub fn for_tool(tool: &str) -> Option<Box<dyn StatusAdapter>> {
    match tool {
        "opencode" => Some(Box::new(opencode::OpenCodeAdapter::default())),
        _ => None,
    }
}
