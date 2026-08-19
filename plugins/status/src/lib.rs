//! polycanv 상태 감지 플러그인.
//!
//! 하는 일은 하나다 — **각 터미널이 지금 🟢🟡🔴⚪ 중 무엇인지 판별해 사이드바에 알린다.**
//!
//! 판별 수단은 우선순위가 있다 (`CLAUDE.md`):
//! ① CLI 훅/이벤트 → ② 출력 패턴 정규식 → ③ 벨 문자 → ④ idle 휴리스틱.
//! **상위 수단이 있는 CLI 에 하위 수단을 섞지 않는다** — 오탐만 늘어난다.
//! 현재 구현된 것은 ① 뿐이고, opencode(SSE) 어댑터가 첫 번째다.
//!
//! 순수 로직([`adapters`], [`ingress`])은 zellij 에 의존하지 않아 호스트 타깃에서 `cargo test`
//! 로 검증된다. wasm 글루는 [`plugin`] 에 격리돼 있다.

pub mod adapters;
pub mod ingress;

#[cfg(target_arch = "wasm32")]
pub mod plugin;
