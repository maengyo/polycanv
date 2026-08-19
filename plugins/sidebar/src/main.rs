//! zellij 가 로드하는 진입점.
//!
//! **바이너리 크레이트여야 한다.** `[lib] crate-type=["cdylib"]` 로 만든 wasm 에는 `_start` 가
//! 없어 로드에 실패한다 (docs/research/zellij-pane-control-api.md 「재현 절차 1)」).
//! `register_plugin!` 이 `fn main` 을 정의하므로 크레이트 루트에서 호출해야 한다.

#[cfg(target_arch = "wasm32")]
use zellij_tile::prelude::*;

#[cfg(target_arch = "wasm32")]
mod plugin;

#[cfg(target_arch = "wasm32")]
register_plugin!(plugin::Sidebar);

/// 호스트 타깃에서는 wasm 글루가 통째로 빠지므로 빈 main 이 필요하다.
/// 순수 로직은 `polycanv_sidebar` 라이브러리에 있고 `cargo test` 로 그쪽만 검증한다.
#[cfg(not(target_arch = "wasm32"))]
fn main() {}
